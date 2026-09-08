//! The network half of the Stripe adapter, against a local mock (wiremock).
//!
//! Every branch that only runs when bytes cross a socket — headers, idempotency keys,
//! retries, error decoding, oversized bodies — lives here. No database, no real Stripe.

use rust_decimal::Decimal;
use std::str::FromStr;
use underground_api::provider::{
    ChargeReq, ChargeStatus, PaymentProvider, PayoutReq, PayoutStatus, ProviderError, RefundReq, RefundStatus,
};
use underground_api::stripe::StripeProvider;
use uuid::Uuid;
use wiremock::matchers::{body_string_contains, header, header_exists, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn provider(server: &MockServer) -> StripeProvider {
    StripeProvider::new("sk_test_unit", Some("whsec_unit".into()), server.uri())
}

fn charge(order: Uuid, connected: Option<&str>) -> ChargeReq {
    ChargeReq {
        order_id: order,
        amount: Decimal::from_str("250.00").unwrap(),
        currency: "UAH".into(),
        return_url: Some("https://underground.example".into()),
        description: "Квиток на подію".into(),
        connected_account: connected.map(str::to_string),
        application_fee: Decimal::from_str("25.00").unwrap(),
    }
}

#[tokio::test]
async fn create_charge_sends_the_fields_stripe_needs_and_returns_the_session() {
    let server = MockServer::start().await;
    let order = Uuid::new_v4();
    Mock::given(method("POST"))
        .and(path("/v1/checkout/sessions"))
        .and(header("authorization", "Bearer sk_test_unit"))
        .and(header("stripe-account", "acct_org_1"))
        .and(header("idempotency-key", order.to_string().as_str()))
        .and(body_string_contains("mode=payment"))
        .and(body_string_contains(
            "line_items%5B0%5D%5Bprice_data%5D%5Bunit_amount%5D=25000",
        ))
        // Session metadata is not copied to the PaymentIntent; without this line every later
        // refund/dispute event is untraceable. Asserted here so it cannot silently vanish.
        .and(body_string_contains("payment_intent_data%5Bmetadata%5D%5Border_id%5D="))
        .and(body_string_contains(
            "payment_intent_data%5Bapplication_fee_amount%5D=2500",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "cs_test_abc", "url": "https://checkout.stripe.com/c/pay/cs_test_abc"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let res = provider(&server)
        .create_charge(charge(order, Some("acct_org_1")))
        .await
        .unwrap();
    assert_eq!(res.provider_ref, "cs_test_abc");
    assert_eq!(
        res.checkout_url.as_deref(),
        Some("https://checkout.stripe.com/c/pay/cs_test_abc")
    );
    assert_eq!(res.status, ChargeStatus::Pending);
}

#[tokio::test]
async fn create_charge_without_connected_account_sends_no_fee_and_no_account_header() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/checkout/sessions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id":"cs_1","url":"u"})))
        .expect(1)
        .mount(&server)
        .await;
    provider(&server)
        .create_charge(charge(Uuid::new_v4(), None))
        .await
        .unwrap();
    let reqs = server.received_requests().await.unwrap();
    let body = String::from_utf8(reqs[0].body.clone()).unwrap();
    assert!(
        !body.contains("application_fee_amount"),
        "no fee destination → no fee: {body}"
    );
    assert!(reqs[0].headers.get("stripe-account").is_none());
}

/// 429 and 5xx are retried with an idempotency key; the same key means Stripe returns the
/// same session, so a retry can never create a second payable checkout.
#[tokio::test]
async fn transient_failures_are_retried_and_then_succeed() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/checkout/sessions"))
        .respond_with(ResponseTemplate::new(429).set_body_json(serde_json::json!({"error":{"message":"slow down"}})))
        .up_to_n_times(2)
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/checkout/sessions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id":"cs_after_retry","url":"u"})))
        .expect(1)
        .mount(&server)
        .await;
    let res = provider(&server)
        .create_charge(charge(Uuid::new_v4(), None))
        .await
        .unwrap();
    assert_eq!(res.provider_ref, "cs_after_retry");
}

#[tokio::test]
async fn three_transient_failures_give_up_with_the_providers_message() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/checkout/sessions"))
        .respond_with(ResponseTemplate::new(503).set_body_json(serde_json::json!({"error":{"message":"maintenance"}})))
        .expect(3)
        .mount(&server)
        .await;
    let err = provider(&server)
        .create_charge(charge(Uuid::new_v4(), None))
        .await
        .unwrap_err();
    assert_eq!(err, ProviderError::Upstream("maintenance".into()));
}

/// A 4xx is Stripe saying we are wrong. Asking again cannot change that — one attempt.
#[tokio::test]
async fn a_client_error_is_not_retried() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/checkout/sessions"))
        .respond_with(
            ResponseTemplate::new(400).set_body_json(serde_json::json!({"error":{"message":"Invalid currency: xyz"}})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let err = provider(&server)
        .create_charge(charge(Uuid::new_v4(), None))
        .await
        .unwrap_err();
    assert_eq!(err, ProviderError::Upstream("Invalid currency: xyz".into()));
}

/// [SEC] A refund carries no idempotency key today only if the caller gave none; with none
/// it must NOT be retried — a repeated refund is money out twice.
#[tokio::test]
async fn a_request_without_idempotency_key_is_never_retried() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/refunds"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({"error":{"message":"boom"}})))
        .expect(1)
        .mount(&server)
        .await;
    let err = provider(&server)
        .refund(RefundReq {
            provider_ref: "pi_1".into(),
            amount: Decimal::ZERO,
            currency: "UAH".into(),
            reason: None,
            connected_account: None,
            refund_application_fee: true,
            idempotency_key: None,
        })
        .await
        .unwrap_err();
    assert_eq!(err, ProviderError::Upstream("boom".into()));
}

#[tokio::test]
async fn refund_sends_key_fee_flag_and_partial_amount() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/refunds"))
        .and(header("idempotency-key", "refund:o-1"))
        .and(body_string_contains("payment_intent=pi_1"))
        .and(body_string_contains("amount=5000"))
        .and(body_string_contains("refund_application_fee=true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id":"re_1","status":"succeeded"})))
        .expect(1)
        .mount(&server)
        .await;
    let res = provider(&server)
        .refund(RefundReq {
            provider_ref: "pi_1".into(),
            amount: Decimal::from_str("50.00").unwrap(),
            currency: "UAH".into(),
            reason: Some("подія скасована".into()),
            connected_account: None,
            refund_application_fee: true,
            idempotency_key: Some("refund:o-1".into()),
        })
        .await
        .unwrap();
    assert_eq!(res.provider_ref, "re_1");
    assert_eq!(res.status, RefundStatus::Succeeded);
}

#[tokio::test]
async fn payout_status_words_map_onto_our_states() {
    for (word, want) in [
        ("paid", PayoutStatus::Paid),
        ("pending", PayoutStatus::Processing),
        ("failed", PayoutStatus::Failed),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/payouts"))
            .and(header_exists("idempotency-key"))
            .and(header("stripe-account", "acct_x"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id":"po_1","status":word})))
            .mount(&server)
            .await;
        let res = provider(&server)
            .transfer_payout(PayoutReq {
                payout_id: Uuid::new_v4(),
                amount: Decimal::from_str("1000.00").unwrap(),
                currency: "UAH".into(),
                connected_account: Some("acct_x".into()),
            })
            .await
            .unwrap();
        assert_eq!(res.status, want, "status word {word}");
    }
}

#[tokio::test]
async fn a_success_without_an_id_is_an_upstream_error_not_a_panic() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/checkout/sessions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"object":"checkout.session"})))
        .mount(&server)
        .await;
    let err = provider(&server)
        .create_charge(charge(Uuid::new_v4(), None))
        .await
        .unwrap_err();
    assert!(matches!(err, ProviderError::Upstream(_)), "{err:?}");
}

#[tokio::test]
async fn a_non_json_body_is_a_transport_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/checkout/sessions"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>gateway</html>"))
        .mount(&server)
        .await;
    let err = provider(&server)
        .create_charge(charge(Uuid::new_v4(), None))
        .await
        .unwrap_err();
    assert!(matches!(err, ProviderError::Transport(_)), "{err:?}");
}

/// [SEC] A misrouted host must not be able to make us buffer arbitrary megabytes.
#[tokio::test]
async fn an_oversized_response_is_refused() {
    let server = MockServer::start().await;
    let huge = format!("{{\"id\":\"{}\"}}", "x".repeat(2 * 1024 * 1024));
    Mock::given(method("POST"))
        .and(path("/v1/checkout/sessions"))
        .respond_with(ResponseTemplate::new(200).set_body_string(huge))
        .mount(&server)
        .await;
    let err = provider(&server)
        .create_charge(charge(Uuid::new_v4(), None))
        .await
        .unwrap_err();
    assert_eq!(err, ProviderError::Transport("stripe response too large".into()));
}

#[tokio::test]
async fn check_connection_reports_the_account_or_the_refusal() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/account"))
        .and(header("authorization", "Bearer sk_test_unit"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id":"acct_platform"})))
        .mount(&server)
        .await;
    assert_eq!(provider(&server).check_connection().await.unwrap(), "acct_platform");

    let bad = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/account"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_body_json(serde_json::json!({"error":{"message":"Invalid API Key provided"}})),
        )
        .mount(&bad)
        .await;
    let err = provider(&bad).check_connection().await.unwrap_err();
    assert_eq!(err, ProviderError::Upstream("Invalid API Key provided".into()));
}

/// Below the currency minimum, or with a fee that swallows the charge, nothing is sent at all.
#[tokio::test]
async fn invalid_amounts_never_reach_the_network() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let mut tiny = charge(Uuid::new_v4(), None);
    tiny.amount = Decimal::from_str("1.00").unwrap();
    tiny.application_fee = Decimal::ZERO;
    assert!(matches!(
        provider(&server).create_charge(tiny).await,
        Err(ProviderError::Invalid(_))
    ));
    let mut greedy = charge(Uuid::new_v4(), Some("acct_1"));
    greedy.application_fee = greedy.amount;
    assert!(matches!(
        provider(&server).create_charge(greedy).await,
        Err(ProviderError::Invalid(_))
    ));
}

/// Voiding a superseded checkout: only a Checkout Session can be expired; a PaymentIntent id
/// means the payment already completed and there is nothing to void — and no request to make.
#[tokio::test]
async fn cancel_charge_expires_a_checkout_session_and_leaves_payment_intents_alone() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/checkout/sessions/cs_old/expire"))
        .and(header("authorization", "Bearer sk_test_unit"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "cs_old", "status": "expired"
        })))
        .expect(1)
        .mount(&server)
        .await;
    provider(&server).cancel_charge("cs_old").await.unwrap();
    provider(&server).cancel_charge("pi_done").await.unwrap();
}

/// A session that already completed cannot be expired: Stripe refuses, we report the refusal,
/// and the caller (checkout) logs and moves on — the order-side marker is the real control.
#[tokio::test]
async fn cancel_charge_reports_a_session_that_can_no_longer_be_expired() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/checkout/sessions/cs_paid/expire"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "error": { "message": "You cannot expire a Checkout Session that is not open." }
        })))
        .expect(1)
        .mount(&server)
        .await;
    let err = provider(&server).cancel_charge("cs_paid").await.unwrap_err();
    assert!(matches!(err, ProviderError::Upstream(_)), "{err:?}");
}

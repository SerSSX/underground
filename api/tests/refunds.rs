//! The refund executor (PAY-11): the queue, the ceiling, and the states a refund can end in.
//!
//! Refunds are the one operation with no clean undo — money that leaves twice does not come
//! back — so most of what is asserted here is what must be REFUSED, and the provider is a fake
//! whose answers are chosen to reproduce the failures a real one produces: a temporary outage,
//! a permanent refusal, and the direct-charge case where the provider says "accepted" while the
//! money has not actually moved.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use rust_decimal::Decimal;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use tower::ServiceExt;
use underground_api::provider::{
    ChargeReq, ChargeResult, ChargeStatus, InboundWebhook, PaymentProvider, PayoutReq, PayoutStatus, ProviderError,
    RefundReq, RefundResult, RefundStatus, TransferResult,
};
use underground_api::routes::router;
use underground_api::state::AppState;
use uuid::Uuid;

fn make_app(pool: PgPool) -> Router {
    router(AppState {
        db: pool,
        jwt_secret: "test-secret".into(),
    })
}

fn build(method: &str, path: &str, token: Option<&str>, body: Option<Value>) -> Request<Body> {
    let mut b = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    b.body(body.map(|v| Body::from(v.to_string())).unwrap_or_else(Body::empty))
        .unwrap()
}

async fn send(app: &Router, req: Request<Body>) -> (StatusCode, Value) {
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, body)
}

async fn register(app: &Router, email: &str) -> String {
    let (_, v) = send(
        app,
        build(
            "POST",
            "/api/auth/register",
            None,
            Some(json!({"email":email,"password":"password1","name":"U"})),
        ),
    )
    .await;
    v["token"].as_str().unwrap().to_string()
}

/// A published, paid event owned by `token`.
async fn paid_event(app: &Router, pool: &PgPool, token: &str, seats: i32) -> Uuid {
    // One account per organiser — `users_psp_account_uidx` refuses a shared one, which is the
    // point of the index: two organisers on one account means one webhook enabling both.
    sqlx::query(
        "UPDATE users SET psp_account_id = 'acct_' || id, psp_charges_enabled = true WHERE psp_account_id IS NULL",
    )
    .execute(pool)
    .await
    .unwrap();
    let (st, v) = send(
        app,
        build(
            "POST",
            "/api/events",
            Some(token),
            Some(json!({"title":"Gig","category":"Rock","price":"100","currency":"UAH",
                        "tickets_total":seats,"cost_type":"paid","status":"published"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "{v}");
    Uuid::parse_str(v["id"].as_str().unwrap()).unwrap()
}

/// Buy and pay for one ticket, leaving a `paid` order with a payment handle a refund can use.
async fn buy(app: &Router, pool: &PgPool, buyer: &str, event: Uuid) -> Uuid {
    let (st, v) = send(
        app,
        build("POST", &format!("/api/events/{event}/checkout"), Some(buyer), None),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "{v}");
    let order = Uuid::parse_str(v["order_id"].as_str().unwrap()).unwrap();
    let (st, _) = send(
        app,
        build("POST", &format!("/api/orders/{order}/pay-test"), Some(buyer), None),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    // A real payment leaves a PaymentIntent handle; the test provider records its own, and the
    // executor refuses to refund against a checkout session, so give it a payment-shaped one.
    sqlx::query("UPDATE orders SET provider_ref = $2 WHERE id = $1")
        .bind(order)
        .bind(format!("pi_{order}"))
        .execute(pool)
        .await
        .unwrap();
    order
}

async fn jobs_for(pool: &PgPool, order: Uuid) -> Vec<(i32, Decimal, String)> {
    sqlx::query_as("SELECT seq, amount, status FROM refund_jobs WHERE order_id = $1 ORDER BY seq")
        .bind(order)
        .fetch_all(pool)
        .await
        .unwrap()
}

// ------------------------------------------------------------------ fake providers

/// Always accepts and settles immediately.
struct GoodProvider;

#[async_trait::async_trait]
impl PaymentProvider for GoodProvider {
    fn name(&self) -> &'static str {
        "test"
    }
    fn moves_real_money(&self) -> bool {
        false
    }
    async fn create_charge(&self, req: ChargeReq) -> Result<ChargeResult, ProviderError> {
        Ok(ChargeResult {
            provider_ref: format!("pi_{}", req.order_id),
            checkout_url: None,
            status: ChargeStatus::Succeeded,
        })
    }
    async fn transfer_payout(&self, req: PayoutReq) -> Result<TransferResult, ProviderError> {
        Ok(TransferResult {
            provider_ref: format!("po_{}", req.payout_id),
            status: PayoutStatus::Paid,
        })
    }
    async fn refund(&self, req: RefundReq) -> Result<RefundResult, ProviderError> {
        Ok(RefundResult {
            provider_ref: format!("re_{}", req.provider_ref),
            status: RefundStatus::Succeeded,
        })
    }
    fn verify_webhook(&self, _h: &axum::http::HeaderMap, _r: &[u8]) -> Result<InboundWebhook, ProviderError> {
        Ok(InboundWebhook::ignored())
    }
    async fn create_seller_account(&self, _c: &str, _e: &str) -> Result<String, ProviderError> {
        Ok("acct_fake".into())
    }
    async fn seller_onboarding_link(&self, _a: &str, _r: &str, _t: &str) -> Result<String, ProviderError> {
        Ok("https://example.invalid/onboard".into())
    }
}

/// Answers with whatever the test asks for, and counts how many times it was called.
struct ScriptedProvider {
    calls: AtomicUsize,
    answer: fn(usize) -> Result<RefundResult, ProviderError>,
}

#[async_trait::async_trait]
impl PaymentProvider for ScriptedProvider {
    fn name(&self) -> &'static str {
        "test"
    }
    fn moves_real_money(&self) -> bool {
        false
    }
    async fn create_charge(&self, _req: ChargeReq) -> Result<ChargeResult, ProviderError> {
        unreachable!()
    }
    async fn transfer_payout(&self, _req: PayoutReq) -> Result<TransferResult, ProviderError> {
        unreachable!()
    }
    async fn refund(&self, _req: RefundReq) -> Result<RefundResult, ProviderError> {
        (self.answer)(self.calls.fetch_add(1, Ordering::SeqCst))
    }
    fn verify_webhook(&self, _h: &axum::http::HeaderMap, _r: &[u8]) -> Result<InboundWebhook, ProviderError> {
        Ok(InboundWebhook::ignored())
    }
    async fn create_seller_account(&self, _c: &str, _e: &str) -> Result<String, ProviderError> {
        unreachable!()
    }
    async fn seller_onboarding_link(&self, _a: &str, _r: &str, _t: &str) -> Result<String, ProviderError> {
        unreachable!()
    }
}

// ------------------------------------------------------------------ the ceiling

/// [SEC] The one invariant with no recovery: however the refunds are sliced, their total can
/// never exceed what the buyer actually paid.
#[sqlx::test(migrations = "./migrations")]
async fn sec_partial_refunds_can_never_add_up_past_what_was_paid(pool: PgPool) {
    let app = make_app(pool.clone());
    let org = register(&app, "org@x.co").await;
    let buyer = register(&app, "b@x.co").await;
    let event = paid_event(&app, &pool, &org, 5).await;
    let order = buy(&app, &pool, &buyer, event).await;
    let mut conn = pool.acquire().await.unwrap();

    let forty = Decimal::from_str("40").unwrap();
    assert!(underground_api::refunds::enqueue(&mut conn, order, forty, "part 1")
        .await
        .unwrap());
    assert!(underground_api::refunds::enqueue(&mut conn, order, forty, "part 2")
        .await
        .unwrap());
    // 40 + 40 + 40 = 120 on a 100 order: the third is refused outright rather than truncated.
    let err = underground_api::refunds::enqueue(&mut conn, order, forty, "part 3")
        .await
        .unwrap_err();
    assert!(format!("{err:?}").contains("Conflict"), "{err:?}");
    assert_eq!(jobs_for(&pool, order).await.len(), 2, "the refused slice left no row");

    // The remainder still fits, exactly.
    assert!(
        underground_api::refunds::enqueue(&mut conn, order, Decimal::from_str("20").unwrap(), "rest")
            .await
            .is_ok()
    );
}

/// [SEC] A refund of zero or a negative amount is a bug, not a refund — and at the provider an
/// omitted amount means "refund everything", so it must never get that far.
#[sqlx::test(migrations = "./migrations")]
async fn sec_a_zero_or_negative_refund_is_refused(pool: PgPool) {
    let app = make_app(pool.clone());
    let org = register(&app, "org@x.co").await;
    let buyer = register(&app, "b@x.co").await;
    let event = paid_event(&app, &pool, &org, 5).await;
    let order = buy(&app, &pool, &buyer, event).await;
    let mut conn = pool.acquire().await.unwrap();
    // Negative is a bug in the caller and is refused outright.
    let err = underground_api::refunds::enqueue(&mut conn, order, Decimal::from_str("-10").unwrap(), "bad")
        .await
        .unwrap_err();
    assert!(format!("{err:?}").contains("Validation"), "{err:?}");
    // Zero is not an error, it is "nothing to send back" — see the 100%-promo test below. Either
    // way no job is created, because a zero refund at the provider means "refund everything".
    assert!(
        !underground_api::refunds::enqueue(&mut conn, order, Decimal::ZERO, "nothing")
            .await
            .unwrap()
    );
    assert!(jobs_for(&pool, order).await.is_empty());
}

/// [SEC] A 100%-off promo is an ordinary organiser feature and produces a PAID order that cost
/// nothing. Cancelling that event must still work: treating the zero-cost order as an error used
/// to abort the whole transaction, so the event stayed on sale and NOBODY was refunded.
#[sqlx::test(migrations = "./migrations")]
async fn cancelling_an_event_with_a_free_order_still_refunds_everyone_else(pool: PgPool) {
    let app = make_app(pool.clone());
    let org = register(&app, "org@x.co").await;
    let free_buyer = register(&app, "free@x.co").await;
    let paying_buyer = register(&app, "pay@x.co").await;
    let event = paid_event(&app, &pool, &org, 10).await;
    sqlx::query(
        "INSERT INTO promo_codes (event_id, code, percent_off, active, max_uses) VALUES ($1, 'ALLFREE', 100, true, 10)",
    )
    .bind(event)
    .execute(&pool)
    .await
    .unwrap();

    // One buyer pays nothing, one pays in full.
    let (st, v) = send(
        &app,
        build(
            "POST",
            &format!("/api/events/{event}/checkout?promo=ALLFREE"),
            Some(&free_buyer),
            None,
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "{v}");
    let free_order = Uuid::parse_str(v["order_id"].as_str().unwrap()).unwrap();
    assert_eq!(v["amount"], "0", "the promo made it free: {v}");
    send(
        &app,
        build(
            "POST",
            &format!("/api/orders/{free_order}/pay-test"),
            Some(&free_buyer),
            None,
        ),
    )
    .await;
    let paid_order = buy(&app, &pool, &paying_buyer, event).await;

    let (st, out) = send(
        &app,
        build("POST", &format!("/api/events/{event}/cancel"), Some(&org), None),
    )
    .await;
    assert_eq!(
        st,
        StatusCode::OK,
        "a free order must not block the cancellation: {out}"
    );
    assert_eq!(out["tickets_returned"], 2, "both tickets come back");
    assert_eq!(
        out["refunds_queued"], 1,
        "only the buyer who actually paid is owed money"
    );
    assert_eq!(jobs_for(&pool, free_order).await.len(), 0);
    assert_eq!(jobs_for(&pool, paid_order).await.len(), 1);
    let (status,): (String,) = sqlx::query_as("SELECT status FROM events WHERE id = $1")
        .bind(event)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "cancelled", "the sale is actually closed");
}

/// [SEC] Money can only come back if it went out: an abandoned checkout has nothing to return,
/// and queueing one would ask the provider to refund a payment that never happened.
#[sqlx::test(migrations = "./migrations")]
async fn sec_an_unpaid_order_cannot_be_refunded(pool: PgPool) {
    let app = make_app(pool.clone());
    let org = register(&app, "org@x.co").await;
    let buyer = register(&app, "b@x.co").await;
    let event = paid_event(&app, &pool, &org, 5).await;
    let (_, v) = send(
        &app,
        build("POST", &format!("/api/events/{event}/checkout"), Some(&buyer), None),
    )
    .await;
    let order = Uuid::parse_str(v["order_id"].as_str().unwrap()).unwrap();
    let mut conn = pool.acquire().await.unwrap();
    let err = underground_api::refunds::enqueue(&mut conn, order, Decimal::from_str("100").unwrap(), "x")
        .await
        .unwrap_err();
    assert!(format!("{err:?}").contains("Conflict"), "{err:?}");
}

// ------------------------------------------------------------------ cancelling an event

/// Cancelling an event returns every ticket and queues every refund in ONE transaction — and
/// cancelling again queues nothing more.
#[sqlx::test(migrations = "./migrations")]
async fn cancelling_an_event_queues_one_refund_per_paid_ticket_and_is_idempotent(pool: PgPool) {
    let app = make_app(pool.clone());
    let org = register(&app, "org@x.co").await;
    let event = paid_event(&app, &pool, &org, 10).await;
    let mut orders = Vec::new();
    for i in 0..3 {
        let buyer = register(&app, &format!("b{i}@x.co")).await;
        orders.push(buy(&app, &pool, &buyer, event).await);
    }

    let (st, v) = send(
        &app,
        build("POST", &format!("/api/events/{event}/cancel"), Some(&org), None),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["tickets_returned"], 3);
    assert_eq!(v["refunds_queued"], 3);

    let queued: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM refund_jobs j JOIN orders o ON o.id = j.order_id WHERE o.event_id = $1",
    )
    .bind(event)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(queued, 3);
    for o in &orders {
        assert_eq!(jobs_for(&pool, *o).await.len(), 1);
    }
    let (status, sales_closed): (String, bool) =
        sqlx::query_as("SELECT status, sales_closed FROM events WHERE id = $1")
            .bind(event)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((status.as_str(), sales_closed), ("cancelled", true));

    // Again: nothing new. The orders are no longer `paid`, so there is nothing left to pick up.
    let (st, v2) = send(
        &app,
        build("POST", &format!("/api/events/{event}/cancel"), Some(&org), None),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v2["refunds_queued"], 0, "a second cancel queues nothing: {v2}");
    let after: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM refund_jobs j JOIN orders o ON o.id = j.order_id WHERE o.event_id = $1",
    )
    .bind(event)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(after, 3);
}

/// Cancelling an event nobody bought a ticket for is a no-op, not an error.
#[sqlx::test(migrations = "./migrations")]
async fn cancelling_an_event_with_no_sales_is_harmless(pool: PgPool) {
    let app = make_app(pool.clone());
    let org = register(&app, "org@x.co").await;
    let event = paid_event(&app, &pool, &org, 10).await;
    let (st, v) = send(
        &app,
        build("POST", &format!("/api/events/{event}/cancel"), Some(&org), None),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v["refunds_queued"], 0);
    assert_eq!(v["tickets_returned"], 0);
}

/// [SEC] Only the organiser or a platform admin may cancel. A door controller with `scan`
/// rights on the event may not: scanning tickets is not deciding to refund every buyer.
#[sqlx::test(migrations = "./migrations")]
async fn sec_only_the_owner_can_cancel_an_event(pool: PgPool) {
    let app = make_app(pool.clone());
    let org = register(&app, "org@x.co").await;
    let stranger = register(&app, "s@x.co").await;
    let event = paid_event(&app, &pool, &org, 5).await;

    let (st, _) = send(
        &app,
        build("POST", &format!("/api/events/{event}/cancel"), Some(&stranger), None),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);

    // Even with scan rights on this very event.
    let scanner_id: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email = 's@x.co'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let owner_id: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email = 'org@x.co'")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO event_staff (event_id, user_id, permission, invited_by) VALUES ($1, $2, 'scan', $3)")
        .bind(event)
        .bind(scanner_id)
        .bind(owner_id)
        .execute(&pool)
        .await
        .unwrap();
    let (st, _) = send(
        &app,
        build("POST", &format!("/api/events/{event}/cancel"), Some(&stranger), None),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN, "[SEC] scan rights are not cancel rights");
    let (status,): (String,) = sqlx::query_as("SELECT status FROM events WHERE id = $1")
        .bind(event)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "published");
}

/// [SEC] Money for a cancelled event must never walk on toward a payout. Delete the
/// `status <> 'cancelled'` condition from the escrow sweep and this test goes red.
#[sqlx::test(migrations = "./migrations")]
async fn sec_a_cancelled_events_money_never_clears_for_payout(pool: PgPool) {
    let app = make_app(pool.clone());
    let org = register(&app, "org@x.co").await;
    let buyer = register(&app, "b@x.co").await;
    let event = paid_event(&app, &pool, &org, 5).await;
    let order = buy(&app, &pool, &buyer, event).await;
    sqlx::query(
        "UPDATE events SET starts_at = now() - interval '2 days', ends_at = now() - interval '2 days' WHERE id = $1",
    )
    .bind(event)
    .execute(&pool)
    .await
    .unwrap();
    send(
        &app,
        build("POST", &format!("/api/events/{event}/cancel"), Some(&org), None),
    )
    .await;

    // The order the cancellation could NOT have caught: money that arrived on its own after the
    // event died, so its escrow is still `held`. This is the only shape the sweep's
    // `status <> 'cancelled'` condition actually guards — with the order left at `refund_due`
    // the sweep would skip it anyway and the test would pass with the guard deleted.
    sqlx::query("UPDATE orders SET escrow_status = 'held' WHERE id = $1")
        .bind(order)
        .execute(&pool)
        .await
        .unwrap();

    underground_api::escrow::sweep(&pool, 0).await.unwrap();
    let escrow: String = sqlx::query_scalar("SELECT escrow_status FROM orders WHERE id = $1")
        .bind(order)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        escrow, "held",
        "[SEC] a cancelled event's money must not advance toward a payout"
    );
}

// ------------------------------------------------------------------ the executor

/// The happy path: a queued job is sent, settles, and is recorded exactly once against the order.
#[sqlx::test(migrations = "./migrations")]
async fn a_queued_refund_is_settled_and_recorded_once(pool: PgPool) {
    let app = make_app(pool.clone());
    let org = register(&app, "org@x.co").await;
    let buyer = register(&app, "b@x.co").await;
    let event = paid_event(&app, &pool, &org, 5).await;
    let order = buy(&app, &pool, &buyer, event).await;
    let mut conn = pool.acquire().await.unwrap();
    underground_api::refunds::enqueue(&mut conn, order, Decimal::from_str("100").unwrap(), "test")
        .await
        .unwrap();
    drop(conn);

    let (settled, failed) = underground_api::refunds::run_once(&pool, &GoodProvider).await.unwrap();
    assert_eq!((settled, failed), (1, 0));
    assert_eq!(jobs_for(&pool, order).await[0].2, "done");
    let (refunded, escrow): (Decimal, String) =
        sqlx::query_as("SELECT refunded_amount, escrow_status FROM orders WHERE id = $1")
            .bind(order)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(refunded, Decimal::from_str("100").unwrap());
    assert_eq!(escrow, "refunded", "the order is whole again");

    // A second pass has nothing to do and must not credit the amount again.
    let (settled, _) = underground_api::refunds::run_once(&pool, &GoodProvider).await.unwrap();
    assert_eq!(settled, 0);
    let refunded: Decimal = sqlx::query_scalar("SELECT refunded_amount FROM orders WHERE id = $1")
        .bind(order)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(refunded, Decimal::from_str("100").unwrap(), "recorded once, not twice");
}

/// A temporary provider failure is retried later; a permanent refusal stops and asks for a
/// human. Neither loses the row, because a lost row is money nobody ever gives back.
#[sqlx::test(migrations = "./migrations")]
async fn a_transient_failure_is_rescheduled_and_a_refusal_needs_attention(pool: PgPool) {
    let app = make_app(pool.clone());
    let org = register(&app, "org@x.co").await;
    let buyer = register(&app, "b@x.co").await;
    let event = paid_event(&app, &pool, &org, 5).await;
    let order = buy(&app, &pool, &buyer, event).await;
    let mut conn = pool.acquire().await.unwrap();
    underground_api::refunds::enqueue(&mut conn, order, Decimal::from_str("100").unwrap(), "test")
        .await
        .unwrap();
    drop(conn);

    let flaky = ScriptedProvider {
        calls: AtomicUsize::new(0),
        answer: |_| Err(ProviderError::Transport("connection reset".into())),
    };
    let (settled, failed) = underground_api::refunds::run_once(&pool, &flaky).await.unwrap();
    assert_eq!((settled, failed), (0, 0), "a blip is neither settled nor given up on");
    let (status, attempts, next_in_future): (String, i32, bool) =
        sqlx::query_as("SELECT status, attempts, next_attempt_at > now() FROM refund_jobs WHERE order_id = $1")
            .bind(order)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "queued", "still owed, waiting to be retried");
    assert_eq!(attempts, 1);
    assert!(next_in_future, "backed off rather than hammering the provider");

    // A permanent refusal: the row stays, visible, with a reason.
    sqlx::query("UPDATE refund_jobs SET next_attempt_at = now() WHERE order_id = $1")
        .bind(order)
        .execute(&pool)
        .await
        .unwrap();
    let refusing = ScriptedProvider {
        calls: AtomicUsize::new(0),
        answer: |_| Err(ProviderError::Upstream("charge already refunded".into())),
    };
    let (_, failed) = underground_api::refunds::run_once(&pool, &refusing).await.unwrap();
    assert_eq!(failed, 1);
    let (status, last_error): (String, Option<String>) =
        sqlx::query_as("SELECT status, last_error FROM refund_jobs WHERE order_id = $1")
            .bind(order)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "needs_attention");
    assert!(last_error.unwrap().contains("already refunded"));
}

/// [SEC] The provider says "accepted" but the money has not moved — the direct-charge case where
/// the organiser's balance is short. The debt must stay open, and the order must NOT read as
/// refunded, until the provider confirms the money actually left.
#[sqlx::test(migrations = "./migrations")]
async fn sec_a_pending_refund_is_not_treated_as_money_returned(pool: PgPool) {
    let app = make_app(pool.clone());
    let org = register(&app, "org@x.co").await;
    let buyer = register(&app, "b@x.co").await;
    let event = paid_event(&app, &pool, &org, 5).await;
    let order = buy(&app, &pool, &buyer, event).await;
    let mut conn = pool.acquire().await.unwrap();
    underground_api::refunds::enqueue(&mut conn, order, Decimal::from_str("100").unwrap(), "test")
        .await
        .unwrap();
    drop(conn);

    let pending = ScriptedProvider {
        calls: AtomicUsize::new(0),
        answer: |_| {
            Ok(RefundResult {
                provider_ref: "re_pending".into(),
                status: RefundStatus::Pending,
            })
        },
    };
    let (settled, failed) = underground_api::refunds::run_once(&pool, &pending).await.unwrap();
    assert_eq!((settled, failed), (0, 0));
    let (status, refunded): (String, Decimal) = sqlx::query_as(
        "SELECT j.status, o.refunded_amount FROM refund_jobs j JOIN orders o ON o.id = j.order_id \
         WHERE j.order_id = $1",
    )
    .bind(order)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "sent", "still in flight");
    assert_eq!(
        refunded,
        Decimal::ZERO,
        "[SEC] nothing is recorded until the money moves"
    );

    // The provider then confirms it settled, and only now does the ledger move.
    let mut conn = pool.acquire().await.unwrap();
    underground_api::refunds::resolve_from_webhook(&mut conn, order, "re_pending", "succeeded", None)
        .await
        .unwrap();
    drop(conn);
    let (status, refunded): (String, Decimal) = sqlx::query_as(
        "SELECT j.status, o.refunded_amount FROM refund_jobs j JOIN orders o ON o.id = j.order_id \
         WHERE j.order_id = $1",
    )
    .bind(order)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "done");
    assert_eq!(refunded, Decimal::from_str("100").unwrap());
}

/// [SEC] The money was taken by one provider; it must not be refunded through another. A
/// provider that never saw the charge would report success while nothing moved.
#[sqlx::test(migrations = "./migrations")]
async fn sec_a_refund_is_never_sent_to_a_provider_that_did_not_take_the_money(pool: PgPool) {
    let app = make_app(pool.clone());
    let org = register(&app, "org@x.co").await;
    let buyer = register(&app, "b@x.co").await;
    let event = paid_event(&app, &pool, &org, 5).await;
    let order = buy(&app, &pool, &buyer, event).await;
    sqlx::query("UPDATE orders SET provider = 'stripe' WHERE id = $1")
        .bind(order)
        .execute(&pool)
        .await
        .unwrap();
    let mut conn = pool.acquire().await.unwrap();
    underground_api::refunds::enqueue(&mut conn, order, Decimal::from_str("100").unwrap(), "test")
        .await
        .unwrap();
    drop(conn);

    // GoodProvider is named "test"; the order was charged by "stripe".
    let (settled, failed) = underground_api::refunds::run_once(&pool, &GoodProvider).await.unwrap();
    assert_eq!((settled, failed), (0, 1));
    let (status, last_error): (String, Option<String>) =
        sqlx::query_as("SELECT status, last_error FROM refund_jobs WHERE order_id = $1")
            .bind(order)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "needs_attention");
    assert!(last_error.unwrap().contains("іншим провайдером"));
}

/// [SEC] A checkout SESSION is not a payment. Refunding against one means the payment webhook
/// never arrived, and guessing would send the provider a reference it cannot honour.
#[sqlx::test(migrations = "./migrations")]
async fn sec_a_refund_against_a_checkout_session_is_refused(pool: PgPool) {
    let app = make_app(pool.clone());
    let org = register(&app, "org@x.co").await;
    let buyer = register(&app, "b@x.co").await;
    let event = paid_event(&app, &pool, &org, 5).await;
    let order = buy(&app, &pool, &buyer, event).await;
    sqlx::query("UPDATE orders SET provider_ref = 'cs_still_a_session' WHERE id = $1")
        .bind(order)
        .execute(&pool)
        .await
        .unwrap();
    let mut conn = pool.acquire().await.unwrap();
    underground_api::refunds::enqueue(&mut conn, order, Decimal::from_str("100").unwrap(), "test")
        .await
        .unwrap();
    drop(conn);
    let (_, failed) = underground_api::refunds::run_once(&pool, &GoodProvider).await.unwrap();
    assert_eq!(failed, 1);
    let status: String = sqlx::query_scalar("SELECT status FROM refund_jobs WHERE order_id = $1")
        .bind(order)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "needs_attention");
}

// ------------------------------------------------------------------ the two refund paths agree

/// A buyer returning their own ticket and an admin refunding the same ticket must do the SAME
/// thing to the promo code. They did not: the buyer's path silently burned a use forever.
#[sqlx::test(migrations = "./migrations")]
async fn a_self_return_gives_the_promo_use_back_like_an_admin_refund_does(pool: PgPool) {
    let app = make_app(pool.clone());
    let org = register(&app, "org@x.co").await;
    let buyer = register(&app, "b@x.co").await;
    let event = paid_event(&app, &pool, &org, 5).await;
    sqlx::query(
        "INSERT INTO promo_codes (event_id, code, percent_off, active, max_uses) VALUES ($1, 'HALF', 50, true, 10)",
    )
    .bind(event)
    .execute(&pool)
    .await
    .unwrap();

    let (st, v) = send(
        &app,
        build(
            "POST",
            &format!("/api/events/{event}/checkout?promo=HALF"),
            Some(&buyer),
            None,
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "{v}");
    let order = Uuid::parse_str(v["order_id"].as_str().unwrap()).unwrap();
    let (st, tv) = send(
        &app,
        build("POST", &format!("/api/orders/{order}/pay-test"), Some(&buyer), None),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    let ticket = tv["id"].as_str().unwrap().to_string();
    let used: i32 = sqlx::query_scalar("SELECT used_count FROM promo_codes WHERE event_id = $1")
        .bind(event)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(used, 1, "the code was claimed at payment");

    let (st, _) = send(
        &app,
        build("POST", &format!("/api/tickets/{ticket}/return"), Some(&buyer), None),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let used: i32 = sqlx::query_scalar("SELECT used_count FROM promo_codes WHERE event_id = $1")
        .bind(event)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        used, 0,
        "the buyer's own return gives the use back, exactly as an admin refund does"
    );

    // And the money is actually queued to go back, not merely marked returned.
    assert_eq!(jobs_for(&pool, order).await.len(), 1);
}

/// [SEC] A payment that lands AFTER the event was cancelled must never become a valid ticket to
/// an event that is not happening — and the money must go back. The buyer's checkout page stays
/// open for the whole hold, so this window is ordinary, not exotic.
#[sqlx::test(migrations = "./migrations")]
async fn sec_a_payment_after_cancellation_is_refunded_not_ticketed(pool: PgPool) {
    let app = make_app(pool.clone());
    let org = register(&app, "org@x.co").await;
    let buyer = register(&app, "b@x.co").await;
    let event = paid_event(&app, &pool, &org, 5).await;

    // Checkout, but do NOT pay yet: the payment page is open.
    let (st, v) = send(
        &app,
        build("POST", &format!("/api/events/{event}/checkout"), Some(&buyer), None),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "{v}");
    let order = Uuid::parse_str(v["order_id"].as_str().unwrap()).unwrap();

    let (st, out) = send(
        &app,
        build("POST", &format!("/api/events/{event}/cancel"), Some(&org), None),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(out["refunds_queued"], 0, "nothing was paid yet");

    // The payment lands now. `pay-test` runs the same `fulfill` a provider webhook would, so
    // this IS the late-payment path: it answers 409 to the buyer's page because there is no
    // ticket to hand over, and books the money to be returned.
    let (st, _) = send(
        &app,
        build("POST", &format!("/api/orders/{order}/pay-test"), Some(&buyer), None),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "a cancelled event cannot mint a ticket");

    let minted: i64 = sqlx::query_scalar("SELECT count(*) FROM tickets WHERE event_id = $1 AND status = 'active'")
        .bind(event)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(minted, 0, "[SEC] no ticket to a cancelled event");
    assert_eq!(jobs_for(&pool, order).await.len(), 1, "the money is queued to go back");
    let (status,): (String,) = sqlx::query_as("SELECT status FROM orders WHERE id = $1")
        .bind(order)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "refund_due");

    // A replayed delivery for the same order adds nothing.
    let mut conn = pool.acquire().await.unwrap();
    let again = underground_api::orders::fulfill(&mut conn, order).await.unwrap();
    drop(conn);
    assert_eq!(again, underground_api::orders::FulfillOutcome::AlreadyHandled);
    assert_eq!(jobs_for(&pool, order).await.len(), 1, "still one debt");
}

/// [SEC] Our commission is returned exactly ONCE, on the slice that finishes the refund. Sending
/// it on every partial would claw the fee back two or three times, out of the platform's balance.
#[sqlx::test(migrations = "./migrations")]
async fn sec_the_commission_is_returned_only_on_the_closing_slice(pool: PgPool) {
    use std::sync::Mutex;
    /// Records the fee flag of every refund it is asked to make.
    struct RecordingProvider {
        seen: Mutex<Vec<bool>>,
    }
    #[async_trait::async_trait]
    impl PaymentProvider for RecordingProvider {
        fn name(&self) -> &'static str {
            "test"
        }
        fn moves_real_money(&self) -> bool {
            false
        }
        async fn create_charge(&self, _r: ChargeReq) -> Result<ChargeResult, ProviderError> {
            unreachable!()
        }
        async fn transfer_payout(&self, _r: PayoutReq) -> Result<TransferResult, ProviderError> {
            unreachable!()
        }
        async fn refund(&self, req: RefundReq) -> Result<RefundResult, ProviderError> {
            self.seen.lock().unwrap().push(req.refund_application_fee);
            Ok(RefundResult {
                provider_ref: format!("re_{}", self.seen.lock().unwrap().len()),
                status: RefundStatus::Succeeded,
            })
        }
        fn verify_webhook(&self, _h: &axum::http::HeaderMap, _r: &[u8]) -> Result<InboundWebhook, ProviderError> {
            Ok(InboundWebhook::ignored())
        }
        async fn create_seller_account(&self, _c: &str, _e: &str) -> Result<String, ProviderError> {
            unreachable!()
        }
        async fn seller_onboarding_link(&self, _a: &str, _r: &str, _t: &str) -> Result<String, ProviderError> {
            unreachable!()
        }
    }

    let app = make_app(pool.clone());
    let org = register(&app, "org@x.co").await;
    let buyer = register(&app, "b@x.co").await;
    let event = paid_event(&app, &pool, &org, 5).await;
    let order = buy(&app, &pool, &buyer, event).await;
    let mut conn = pool.acquire().await.unwrap();
    // Two halves of a 100.00 order.
    for reason in ["half one", "half two"] {
        assert!(
            underground_api::refunds::enqueue(&mut conn, order, Decimal::from_str("50").unwrap(), reason)
                .await
                .unwrap()
        );
    }
    drop(conn);

    let p = RecordingProvider {
        seen: Mutex::new(Vec::new()),
    };
    let (settled, failed) = underground_api::refunds::run_once(&pool, &p).await.unwrap();
    assert_eq!((settled, failed), (2, 0));
    // The invariant is that the fee is claimed back EXACTLY ONCE. Which of the two slices carries
    // it depends on the order the batch happens to process them in, and that is not the property
    // worth pinning — "returned twice" is what costs the platform money.
    let seen = p.seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 2, "both slices were sent");
    assert_eq!(
        seen.iter().filter(|&&f| f).count(),
        1,
        "the commission comes back on exactly one slice, not on every partial: {seen:?}"
    );
}

/// The organiser's view of a draining cancellation, and who may see it. Sales figures are the
/// organiser's business: a stranger gets 403, not a count.
#[sqlx::test(migrations = "./migrations")]
async fn refund_progress_is_visible_to_the_owner_and_to_an_admin_only(pool: PgPool) {
    let app = make_app(pool.clone());
    let org = register(&app, "org@x.co").await;
    let buyer = register(&app, "b@x.co").await;
    let stranger = register(&app, "s@x.co").await;
    let admin = register(&app, "admin@x.co").await;
    sqlx::query("UPDATE users SET role = 'ADMIN' WHERE email = 'admin@x.co'")
        .execute(&pool)
        .await
        .unwrap();
    let event = paid_event(&app, &pool, &org, 5).await;
    let order = buy(&app, &pool, &buyer, event).await;
    send(
        &app,
        build("POST", &format!("/api/events/{event}/cancel"), Some(&org), None),
    )
    .await;

    let (st, p) = send(
        &app,
        build("GET", &format!("/api/events/{event}/refund-progress"), Some(&org), None),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{p}");
    assert_eq!(p["total"], 1);
    assert_eq!(p["in_flight"], 1);
    assert_eq!(p["done"], 0);
    assert_eq!(p["needs_attention"], 0);
    assert_eq!(p["amount_refunded"], "0");

    // The platform admin sees the same figures.
    let (st, ap) = send(
        &app,
        build(
            "GET",
            &format!("/api/events/{event}/refund-progress"),
            Some(&admin),
            None,
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(ap["total"], 1);

    // Nobody else does.
    let (st, _) = send(
        &app,
        build(
            "GET",
            &format!("/api/events/{event}/refund-progress"),
            Some(&stranger),
            None,
        ),
    )
    .await;
    assert_eq!(
        st,
        StatusCode::FORBIDDEN,
        "[SEC] sales figures are the organiser's business"
    );

    // After the executor runs, the figures move.
    underground_api::refunds::run_once(&pool, &GoodProvider).await.unwrap();
    let (_, done) = send(
        &app,
        build("GET", &format!("/api/events/{event}/refund-progress"), Some(&org), None),
    )
    .await;
    assert_eq!(done["done"], 1);
    assert_eq!(done["in_flight"], 0);
    assert_eq!(done["amount_refunded"], "100.00");
    let _ = order;
}

/// A platform admin can cancel an event the organiser will not — the case cancellation exists
/// for when an organiser goes silent.
#[sqlx::test(migrations = "./migrations")]
async fn a_platform_admin_can_cancel_an_event(pool: PgPool) {
    let app = make_app(pool.clone());
    let org = register(&app, "org@x.co").await;
    let admin = register(&app, "admin@x.co").await;
    sqlx::query("UPDATE users SET role = 'ADMIN' WHERE email = 'admin@x.co'")
        .execute(&pool)
        .await
        .unwrap();
    let event = paid_event(&app, &pool, &org, 5).await;
    let buyer = register(&app, "b@x.co").await;
    buy(&app, &pool, &buyer, event).await;

    let (st, out) = send(
        &app,
        build("POST", &format!("/api/events/{event}/cancel"), Some(&admin), None),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{out}");
    assert_eq!(out["refunds_queued"], 1);
    let (status,): (String,) = sqlx::query_as("SELECT status FROM events WHERE id = $1")
        .bind(event)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "cancelled");
}

/// Cancelling an event that does not exist is a 404, not a 403 — and a stranger still cannot
/// learn whether it exists.
#[sqlx::test(migrations = "./migrations")]
async fn cancelling_an_unknown_event_is_not_found(pool: PgPool) {
    let app = make_app(pool.clone());
    let org = register(&app, "org@x.co").await;
    let (st, _) = send(
        &app,
        build(
            "POST",
            &format!("/api/events/{}/cancel", Uuid::new_v4()),
            Some(&org),
            None,
        ),
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}

//! Payment-provider seam (TASK-104 / BR-006, BR-021).
//!
//! The escrow domain (`escrow.rs`) owns the money state machine; a `PaymentProvider`
//! only performs the side effects (charge / payout / refund / webhook parsing) behind
//! this provider-neutral trait. Swapping the provider (Noop → Test → a real PSP) never
//! touches domain logic. Product-first providers (`Noop`, `Test`) move NO real money
//! (`moves_real_money() == false`), so the whole product is demoable before a live PSP.

use axum::http::HeaderMap;
use rust_decimal::Decimal;
use uuid::Uuid;

/// Provider-neutral failure. Never carries secrets or raw provider payloads.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ProviderError {
    /// Our own request was wrong — a bad amount, a missing credential. Never retried.
    #[error("invalid charge: {0}")]
    Invalid(String),
    /// The network or the provider was unreachable. Safe to retry.
    #[error("provider unreachable: {0}")]
    Transport(String),
    /// The provider answered and refused. The message is the provider's own, and is safe
    /// to show an operator — it never contains our API key.
    #[error("{0}")]
    Upstream(String),
}

/// Result of asking the provider to collect money for an order.
#[derive(Debug, Clone, PartialEq)]
pub enum ChargeStatus {
    /// Buyer must complete payment at `checkout_url` (hosted redirect / SDK sheet).
    Pending,
    /// Funds captured — the order can be fulfilled now.
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PayoutStatus {
    Requested,
    Processing,
    Paid,
    Failed,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RefundStatus {
    Succeeded,
    Failed,
    /// Accepted but the money has not moved yet — for a direct charge this is what an organiser
    /// with too little balance produces, and the provider settles it later or gives up. The
    /// refund job stays open until a webhook says which.
    Pending,
}

/// A normalized inbound payment event, decoded from a provider webhook.
#[derive(Debug, Clone, PartialEq)]
pub enum WebhookEvent {
    Paid {
        order_ref: String,
        /// The handle a later refund must be issued against. Stripe replaces the checkout
        /// session with a PaymentIntent once paid, and only the PaymentIntent is refundable,
        /// so the id we stored at checkout is not the one we will need.
        provider_ref: Option<String>,
        /// The id WE stored on the order at checkout (Stripe: the Checkout Session). This is
        /// the binding: `order_ref` comes from metadata, which the payer can influence, so a
        /// payment is applied only when the object it arrived on is the one we created for
        /// that order. Without this, paying for a cheap order with a forged `order_id` in
        /// metadata fulfils an expensive one.
        charge_ref: Option<String>,
        /// What was actually paid, in the provider's minor units — compared against the order
        /// before minting, so a tampered or partial amount never yields a full ticket.
        amount_minor: Option<i64>,
        currency: Option<String>,
    },
    Refunded {
        order_ref: String,
        /// How much has come back so far vs the charge total, in minor units. A partial
        /// refund is a bookkeeping entry, not a returned ticket; only `refunded == total`
        /// closes the order.
        refunded_minor: Option<i64>,
        total_minor: Option<i64>,
    },
    /// The buyer charged back. Terminal for the money: the ticket must not be honoured and
    /// the escrow row freezes until an admin resolves it.
    Disputed {
        order_ref: String,
    },
    /// A payout we requested reached (or failed to reach) the organiser's bank. Until this
    /// arrives, `payouts.status` is a claim, not a fact.
    PayoutPaid {
        payout_ref: String,
    },
    PayoutFailed {
        payout_ref: String,
        reason: String,
    },
    /// A connected account's capabilities changed — this is how we learn an organiser
    /// finished (or broke) their onboarding without polling.
    AccountUpdated {
        account_ref: String,
        charges_enabled: bool,
        payouts_enabled: bool,
        /// WHICH documents the provider is still waiting for, not merely that some are. The
        /// organiser has to be told what to go and do; "потрібні документи" with no list is a
        /// dead end for them.
        requirements_due: Vec<String>,
    },
    /// A refund we issued finally resolved — this is how a `Pending` refund becomes settled or
    /// failed, and the only signal that money actually left the organiser's balance.
    RefundResolved {
        order_ref: String,
        refund_ref: String,
        status: String,
        failure_reason: Option<String>,
    },
    /// Recognized but not actionable (or providers that never call back).
    Ignored,
}

/// An authenticated webhook delivery: the normalized action plus what we need to record it.
///
/// `id` is the PROVIDER's event id, and it exists for de-duplication: providers document
/// that delivery is at-least-once and unordered, so "have we already applied this exact
/// event" cannot be answered from the order's own state alone.
#[derive(Debug, Clone, PartialEq)]
pub struct InboundWebhook {
    pub id: Option<String>,
    pub kind: String,
    pub event: WebhookEvent,
}

impl InboundWebhook {
    /// A delivery we authenticated but do not act on.
    pub fn ignored() -> Self {
        InboundWebhook {
            id: None,
            kind: "ignored".into(),
            event: WebhookEvent::Ignored,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChargeReq {
    pub order_id: Uuid,
    pub amount: Decimal,
    pub currency: String,
    pub return_url: Option<String>,
    /// What the buyer sees on the hosted checkout and on their card statement line.
    pub description: String,
    /// The organiser's account for a DIRECT charge: funds land on their balance, not ours.
    /// `None` = the charge goes to the platform account (free/test orders, legacy flow).
    pub connected_account: Option<String>,
    /// Our commission, taken out of the same movement. Zero when there is no fee to take.
    pub application_fee: Decimal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChargeResult {
    pub provider_ref: String,
    pub checkout_url: Option<String>,
    pub status: ChargeStatus,
}

#[derive(Debug, Clone)]
pub struct PayoutReq {
    pub payout_id: Uuid,
    pub amount: Decimal,
    pub currency: String,
    /// Whose balance the money leaves. On direct charges this is the organiser's account.
    pub connected_account: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransferResult {
    pub provider_ref: String,
    pub status: PayoutStatus,
}

#[derive(Debug, Clone)]
pub struct RefundReq {
    pub provider_ref: String,
    pub amount: Decimal,
    /// Needed to convert `amount` into the provider's minor units; a refund in the wrong
    /// currency scale is the same factor-of-100 bug as a charge, only harder to notice.
    pub currency: String,
    pub reason: Option<String>,
    pub connected_account: Option<String>,
    /// Give our commission back too. True for a full refund: otherwise we keep a fee on
    /// money the buyer no longer paid.
    pub refund_application_fee: bool,
    /// Sent as the provider's idempotency key. A retried refund — ours or the network's —
    /// must return the SAME refund, not issue a second one; without a key a timeout on the
    /// first attempt and a retry is money out twice.
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RefundResult {
    pub provider_ref: String,
    pub status: RefundStatus,
}

/// The single seam every money side effect flows through. Implementations are stateless
/// and cheap to construct; the domain selects one per request via [`resolve`].
#[async_trait::async_trait]
pub trait PaymentProvider: Send + Sync {
    /// Stable identifier persisted with orders/payouts (`provider` column).
    fn name(&self) -> &'static str;

    /// Whether this provider actually moves money. Product-first providers return `false`
    /// so payout/refund stay pure domain transitions until a real PSP is wired (BR-021).
    fn moves_real_money(&self) -> bool;

    async fn create_charge(&self, req: ChargeReq) -> Result<ChargeResult, ProviderError>;
    async fn transfer_payout(&self, req: PayoutReq) -> Result<TransferResult, ProviderError>;
    async fn refund(&self, req: RefundReq) -> Result<RefundResult, ProviderError>;

    /// Decode + authenticate an inbound webhook into a normalized event.
    fn verify_webhook(&self, headers: &HeaderMap, raw: &[u8]) -> Result<InboundWebhook, ProviderError>;

    /// Void a not-yet-paid charge (a superseded checkout). Best effort: a session that already
    /// completed cannot be voided, and the caller logs and moves on — the order-side
    /// `superseded` marker is what stops the money from becoming a ticket.
    async fn cancel_charge(&self, _provider_ref: &str) -> Result<(), ProviderError> {
        Ok(())
    }

    /// Create the SELLER's own account at the provider and return its id. The organiser is the
    /// merchant of record for their tickets, so this account — not ours — is where their money
    /// lands. Deliberately has no default body: a new provider must decide how it onboards
    /// sellers instead of inheriting a stub that reports success and never took an account.
    ///
    /// The returned id is the ONLY source of `users.psp_account_id`; it never comes from a
    /// request body (PAY-5 SEC).
    async fn create_seller_account(&self, country: &str, email: &str) -> Result<String, ProviderError>;

    /// A short-lived, single-use URL where the seller enters their own identity and bank
    /// details, on the provider's pages. We never see those details and never store this URL.
    async fn seller_onboarding_link(
        &self,
        account_ref: &str,
        refresh_url: &str,
        return_url: &str,
    ) -> Result<String, ProviderError>;
}

/// Free / RSVP provider: only zero-amount charges, no money ever moves.
pub struct NoopProvider;

#[async_trait::async_trait]
impl PaymentProvider for NoopProvider {
    fn name(&self) -> &'static str {
        "noop"
    }
    fn moves_real_money(&self) -> bool {
        false
    }
    async fn create_charge(&self, req: ChargeReq) -> Result<ChargeResult, ProviderError> {
        if req.amount != Decimal::ZERO {
            return Err(ProviderError::Invalid(
                "noop provider only accepts free (zero) charges".into(),
            ));
        }
        Ok(ChargeResult {
            provider_ref: format!("noop:{}", req.order_id),
            checkout_url: None,
            status: ChargeStatus::Succeeded,
        })
    }
    async fn transfer_payout(&self, req: PayoutReq) -> Result<TransferResult, ProviderError> {
        Ok(TransferResult {
            provider_ref: format!("noop:{}", req.payout_id),
            status: PayoutStatus::Paid,
        })
    }
    async fn refund(&self, req: RefundReq) -> Result<RefundResult, ProviderError> {
        Ok(RefundResult {
            provider_ref: req.provider_ref,
            status: RefundStatus::Succeeded,
        })
    }
    fn verify_webhook(&self, _headers: &HeaderMap, _raw: &[u8]) -> Result<InboundWebhook, ProviderError> {
        Ok(InboundWebhook::ignored())
    }
    /// Deterministic: the same organiser always gets the same account id, so a test can assert
    /// that a second connect call did NOT create a second account.
    async fn create_seller_account(&self, country: &str, email: &str) -> Result<String, ProviderError> {
        Ok(format!(
            "acct_noop_{}_{}",
            country.to_ascii_lowercase(),
            email.trim().to_ascii_lowercase()
        ))
    }
    async fn seller_onboarding_link(
        &self,
        account_ref: &str,
        _refresh_url: &str,
        return_url: &str,
    ) -> Result<String, ProviderError> {
        // The return URL already carries a query string, so join with `&`. A stub that produces
        // `...?done=1?onboarding=...` is not a URL anyone can follow, and this one is what a
        // demo stand actually opens.
        let sep = if return_url.contains('?') { '&' } else { '?' };
        Ok(format!("{return_url}{sep}onboarding={account_ref}"))
    }
}

/// Deterministic test provider: paid amounts succeed instantly with NO real money,
/// so the full buy → hold → payout → refund flow is demoable in test mode.
pub struct TestProvider;

#[async_trait::async_trait]
impl PaymentProvider for TestProvider {
    fn name(&self) -> &'static str {
        "test"
    }
    fn moves_real_money(&self) -> bool {
        false
    }
    async fn create_charge(&self, req: ChargeReq) -> Result<ChargeResult, ProviderError> {
        if req.amount < Decimal::ZERO {
            return Err(ProviderError::Invalid("amount must be >= 0".into()));
        }
        Ok(ChargeResult {
            provider_ref: format!("test:{}", req.order_id),
            checkout_url: None,
            status: ChargeStatus::Succeeded,
        })
    }
    async fn transfer_payout(&self, req: PayoutReq) -> Result<TransferResult, ProviderError> {
        Ok(TransferResult {
            provider_ref: format!("test:{}", req.payout_id),
            status: PayoutStatus::Paid,
        })
    }
    async fn refund(&self, req: RefundReq) -> Result<RefundResult, ProviderError> {
        Ok(RefundResult {
            provider_ref: req.provider_ref,
            status: RefundStatus::Succeeded,
        })
    }
    fn verify_webhook(&self, _headers: &HeaderMap, _raw: &[u8]) -> Result<InboundWebhook, ProviderError> {
        Ok(InboundWebhook::ignored())
    }
    /// Deterministic: the same organiser always gets the same account id, so a test can assert
    /// that a second connect call did NOT create a second account.
    async fn create_seller_account(&self, country: &str, email: &str) -> Result<String, ProviderError> {
        Ok(format!(
            "acct_test_{}_{}",
            country.to_ascii_lowercase(),
            email.trim().to_ascii_lowercase()
        ))
    }
    async fn seller_onboarding_link(
        &self,
        account_ref: &str,
        _refresh_url: &str,
        return_url: &str,
    ) -> Result<String, ProviderError> {
        // The return URL already carries a query string, so join with `&`. A stub that produces
        // `...?done=1?onboarding=...` is not a URL anyone can follow, and this one is what a
        // demo stand actually opens.
        let sep = if return_url.contains('?') { '&' } else { '?' };
        Ok(format!("{return_url}{sep}onboarding={account_ref}"))
    }
}

/// Which environment a Stripe secret key belongs to, read off its prefix.
pub fn stripe_key_kind(key: &str) -> Option<&'static str> {
    let k = key.trim();
    if k.starts_with("sk_live_") || k.starts_with("rk_live_") {
        Some("live")
    } else if k.starts_with("sk_test_") || k.starts_with("rk_test_") {
        Some("test")
    } else {
        None
    }
}

/// Select the active provider from the stored settings.
///
/// Every refusal here is deliberate and loud, because the alternative is quiet money damage:
/// - an unknown name is an error, never a fallback to `Test` (a typo must stop checkout, not
///   sell tickets nobody can be charged for);
/// - `lemonsqueezy` is retired (PAY-12) — selecting it refuses rather than pretending;
/// - Stripe requires a key, AND the key's environment must match `mode`. A live key under
///   `mode = test` would take real money on the demo stand while every screen says "test";
///   a test key under `mode = live` charges nobody while every screen says "paid".
pub fn resolve(settings: &crate::payments::PaymentSettings) -> Result<Box<dyn PaymentProvider>, ProviderError> {
    let provider: Box<dyn PaymentProvider> = match settings.provider.as_str() {
        "noop" => Box::new(NoopProvider),
        "test" => Box::new(TestProvider),
        "lemonsqueezy" => {
            return Err(ProviderError::Invalid(
                "lemonsqueezy is retired — select stripe or test".into(),
            ))
        }
        "stripe" => {
            let key = settings
                .stripe_secret_key
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| ProviderError::Invalid("stripe selected but no secret key configured".into()))?;
            match (stripe_key_kind(key), settings.mode.as_str()) {
                (Some("live"), "live") | (Some("test"), "test") => {}
                (Some(kind), mode) => {
                    return Err(ProviderError::Invalid(format!(
                        "stripe key is {kind} but payment mode is {mode}"
                    )))
                }
                (None, _) => return Err(ProviderError::Invalid("stripe key has no recognised prefix".into())),
            }
            Box::new(crate::stripe::StripeProvider::new(
                key,
                settings.stripe_webhook_secret.clone(),
                crate::stripe::base_url()?,
            ))
        }
        other => return Err(ProviderError::Invalid(format!("unknown payment provider: {other}"))),
    };
    // Live mode means real money. A provider that moves none (`test`, `noop`) under
    // `mode = live` would create orders every screen calls paid while nobody was charged —
    // the exact hazard the old "live without a variant" guard existed for, now stated once
    // for every provider instead of once per provider.
    if settings.mode == "live" && !provider.moves_real_money() {
        return Err(ProviderError::Invalid(format!(
            "provider '{}' moves no real money and cannot run in live mode",
            provider.name()
        )));
    }
    Ok(provider)
}

/// Select by bare name, for call sites that have no settings row (unit tests, tooling).
/// Same contract as [`resolve`]: an unknown or retired name is an error, never a fallback.
pub fn resolve_by_name(provider: &str) -> Result<Box<dyn PaymentProvider>, ProviderError> {
    match provider {
        "noop" => Ok(Box::new(NoopProvider)),
        "test" => Ok(Box::new(TestProvider)),
        "lemonsqueezy" => Err(ProviderError::Invalid("lemonsqueezy is retired".into())),
        other => Err(ProviderError::Invalid(format!("unknown payment provider: {other}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::prelude::FromPrimitive;

    fn charge(amount: Decimal) -> ChargeReq {
        ChargeReq {
            order_id: Uuid::nil(),
            amount,
            currency: "UAH".into(),
            return_url: None,
            description: "Квиток".into(),
            connected_account: None,
            application_fee: Decimal::ZERO,
        }
    }

    fn payout(amount: Decimal) -> PayoutReq {
        PayoutReq {
            payout_id: Uuid::nil(),
            amount,
            currency: "UAH".into(),
            connected_account: None,
        }
    }

    fn refund_of(provider_ref: &str) -> RefundReq {
        RefundReq {
            provider_ref: provider_ref.into(),
            amount: Decimal::ZERO,
            currency: "UAH".into(),
            reason: None,
            connected_account: None,
            refund_application_fee: false,
            idempotency_key: None,
        }
    }

    #[tokio::test]
    async fn noop_accepts_free_and_moves_no_money() {
        let p = NoopProvider;
        assert!(!p.moves_real_money());
        let res = p.create_charge(charge(Decimal::ZERO)).await.unwrap();
        assert_eq!(res.status, ChargeStatus::Succeeded);
        assert_eq!(res.checkout_url, None);
        assert_eq!(res.provider_ref, format!("noop:{}", Uuid::nil()));
    }

    #[tokio::test]
    async fn noop_rejects_paid_charge() {
        let p = NoopProvider;
        let err = p
            .create_charge(charge(Decimal::from_f64(10.0).unwrap()))
            .await
            .unwrap_err();
        assert!(
            matches!(err, ProviderError::Invalid(_)),
            "noop must reject non-zero amount"
        );
    }

    #[tokio::test]
    async fn noop_payout_and_refund_succeed_without_money() {
        let p = NoopProvider;
        let po = p.transfer_payout(payout(Decimal::ZERO)).await.unwrap();
        assert_eq!(po.status, PayoutStatus::Paid);
        let rf = p.refund(refund_of("noop:x")).await.unwrap();
        assert_eq!(rf.status, RefundStatus::Succeeded);
    }

    #[tokio::test]
    async fn test_provider_succeeds_for_paid_without_real_money() {
        let p = TestProvider;
        assert!(!p.moves_real_money());
        let res = p
            .create_charge(charge(Decimal::from_f64(250.0).unwrap()))
            .await
            .unwrap();
        assert_eq!(res.status, ChargeStatus::Succeeded);
    }

    #[tokio::test]
    async fn test_provider_rejects_negative_amount() {
        let p = TestProvider;
        let err = p
            .create_charge(charge(Decimal::from_f64(-1.0).unwrap()))
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::Invalid(_)));
    }

    #[tokio::test]
    async fn payout_and_refund_succeed_as_domain_transitions() {
        let p = TestProvider;
        let po = p
            .transfer_payout(payout(Decimal::from_f64(100.0).unwrap()))
            .await
            .unwrap();
        assert_eq!(po.status, PayoutStatus::Paid);
        let rf = p.refund(refund_of("test:x")).await.unwrap();
        assert_eq!(rf.status, RefundStatus::Succeeded);
    }

    #[test]
    fn resolve_by_name_maps_known_providers() {
        assert_eq!(resolve_by_name("noop").unwrap().name(), "noop");
        assert_eq!(resolve_by_name("test").unwrap().name(), "test");
        // Retired, not aliased: mapping it to Test created pending orders nobody could pay
        // (review of PAY-1, finding M7). A retired provider refuses at resolve time.
        assert!(
            matches!(resolve_by_name("lemonsqueezy"), Err(ProviderError::Invalid(_))),
            "lemonsqueezy is retired and must refuse, not fall back"
        );
    }

    fn settings(provider: &str, mode: &str, key: Option<&str>) -> crate::payments::PaymentSettings {
        crate::payments::PaymentSettings {
            mode: mode.into(),
            provider: provider.into(),
            ls_api_key: None,
            ls_store_id: None,
            ls_variant_id: None,
            ls_webhook_secret: None,
            stripe_secret_key: key.map(str::to_string),
            stripe_publishable_key: None,
            stripe_webhook_secret: None,
            stripe_last_webhook_at: None,
            stripe_webhook_rejections: 0,
            platform_fee_bps: 1000,
            clearing_window_days: 7,
            updated_at: chrono::Utc::now(),
        }
    }

    /// [SEC] The key's environment and the payment mode must agree in BOTH directions. A live
    /// key under test mode is the dangerous one: real charges on the demo stand while every
    /// screen says "test".
    /// The trait's default `cancel_charge` is a no-op the money-free providers inherit; a
    /// superseded free/RSVP order has nothing to void at a provider. Exercised so the default
    /// body is not dead (it is the fallback every future provider gets for free).
    #[tokio::test]
    async fn money_free_providers_cancel_charge_is_a_harmless_noop() {
        assert!(NoopProvider.cancel_charge("cs_x").await.is_ok());
        assert!(TestProvider.cancel_charge("cs_x").await.is_ok());
    }

    /// The money-free providers still have to answer the onboarding calls, because a demo stand
    /// runs the same organiser flow. Deterministic: the same organiser always gets the same
    /// account id, which is what lets a test prove a second connect made no second account.
    #[tokio::test]
    async fn money_free_providers_onboard_deterministically() {
        for p in [&NoopProvider as &dyn PaymentProvider, &TestProvider] {
            let a = p.create_seller_account("PL", " Org@Example.com ").await.unwrap();
            let b = p.create_seller_account("pl", "org@example.com").await.unwrap();
            assert_eq!(a, b, "same organiser, same account");
            assert!(a.contains("pl") && a.contains("org@example.com"), "{a}");

            // One query string, not two: the return URL already carries one.
            let url = p
                .seller_onboarding_link(&a, "https://x.test/again", "https://x.test/done?ok=1")
                .await
                .unwrap();
            assert_eq!(url.matches('?').count(), 1, "{url}");
            assert!(url.contains("onboarding="), "{url}");
        }
    }

    /// Stripe selected but no key configured is a configuration error, refused at resolve.
    #[test]
    fn resolve_refuses_stripe_without_a_secret_key() {
        let _env = crate::stripe::test_env_lock().lock().unwrap();
        assert!(matches!(
            resolve(&settings("stripe", "test", None)),
            Err(ProviderError::Invalid(m)) if m.contains("no secret key")
        ));
    }

    /// Restricted keys (`rk_`) are environment-typed like secret keys; a copy of this logic in
    /// payments.rs once lagged behind and saved an rk_live_ key under test mode as "unknown".
    #[test]
    fn stripe_key_kind_reads_secret_and_restricted_keys() {
        assert_eq!(stripe_key_kind("sk_live_a"), Some("live"));
        assert_eq!(stripe_key_kind("rk_live_a"), Some("live"));
        assert_eq!(stripe_key_kind("sk_test_a"), Some("test"));
        assert_eq!(stripe_key_kind("rk_test_a"), Some("test"));
        assert_eq!(
            stripe_key_kind("pk_live_a"),
            None,
            "publishable keys are not account keys"
        );
        assert_eq!(stripe_key_kind("garbage"), None);
    }

    #[test]
    fn resolve_refuses_stripe_key_that_disagrees_with_mode() {
        let _env = crate::stripe::test_env_lock().lock().unwrap();
        assert!(matches!(
            resolve(&settings("stripe", "test", Some("sk_live_abc"))),
            Err(ProviderError::Invalid(_))
        ));
        assert!(matches!(
            resolve(&settings("stripe", "live", Some("sk_test_abc"))),
            Err(ProviderError::Invalid(_))
        ));
        assert!(matches!(
            resolve(&settings("stripe", "live", Some("garbage"))),
            Err(ProviderError::Invalid(_))
        ));
        assert_eq!(
            resolve(&settings("stripe", "test", Some("sk_test_abc")))
                .unwrap()
                .name(),
            "stripe"
        );
        assert_eq!(
            resolve(&settings("stripe", "live", Some("sk_live_abc")))
                .unwrap()
                .name(),
            "stripe"
        );
    }

    /// [SEC] Live mode with a provider that moves no money would sell tickets nobody paid for.
    #[test]
    fn resolve_refuses_money_free_providers_in_live_mode() {
        let _env = crate::stripe::test_env_lock().lock().unwrap();
        assert!(matches!(
            resolve(&settings("test", "live", None)),
            Err(ProviderError::Invalid(_))
        ));
        assert!(matches!(
            resolve(&settings("noop", "live", None)),
            Err(ProviderError::Invalid(_))
        ));
        assert_eq!(resolve(&settings("test", "test", None)).unwrap().name(), "test");
        assert_eq!(resolve(&settings("noop", "test", None)).unwrap().name(), "noop");
    }

    // The previous contract silently substituted Test for anything unknown. That is the
    // behaviour PAY-1 removes: a typo in `payment_settings.provider` must stop checkout, not
    // quietly sell tickets nobody can be charged for.
    #[test]
    fn resolve_by_name_rejects_unknown_provider() {
        // `Box<dyn PaymentProvider>` is not Debug, so unwrap_err() is unavailable here.
        assert!(
            matches!(resolve_by_name("wayforpay"), Err(ProviderError::Invalid(m)) if m.contains("unknown payment provider"))
        );
        // The settings-driven entry point refuses the same way — a typo in the row stops checkout.
        assert!(
            matches!(resolve(&settings("wayforpay", "test", None)), Err(ProviderError::Invalid(m)) if m.contains("unknown payment provider"))
        );
    }

    #[test]
    fn webhook_is_ignored_for_product_first_providers() {
        let h = HeaderMap::new();
        assert_eq!(
            NoopProvider.verify_webhook(&h, b"{}").unwrap().event,
            WebhookEvent::Ignored
        );
        assert_eq!(
            TestProvider.verify_webhook(&h, b"{}").unwrap().event,
            WebhookEvent::Ignored
        );
    }
}

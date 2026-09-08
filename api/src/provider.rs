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
    },
    Refunded {
        order_ref: String,
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
        requirements_due: bool,
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
}

/// Select the active provider from the stored settings.
///
/// Stripe is chosen only when it is BOTH selected and has a secret key: a half-configured
/// Stripe would otherwise fail at the network boundary on every checkout, which reads to a
/// buyer as the product being broken rather than as the operator having unfinished setup.
/// Everything else falls back to `Test`, so the domain never accidentally moves real money.
pub fn resolve(settings: &crate::payments::PaymentSettings) -> Result<Box<dyn PaymentProvider>, ProviderError> {
    match settings.provider.as_str() {
        "noop" => Ok(Box::new(NoopProvider)),
        "test" | "lemonsqueezy" => Ok(Box::new(TestProvider)),
        "stripe" => {
            let key = settings
                .stripe_secret_key
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| ProviderError::Invalid("stripe selected but no secret key configured".into()))?;
            Ok(Box::new(crate::stripe::StripeProvider::new(
                key,
                settings.stripe_webhook_secret.clone(),
                crate::stripe::base_url(),
            )))
        }
        other => Err(ProviderError::Invalid(format!("unknown payment provider: {other}"))),
    }
}

/// Select by bare name, for call sites that have no settings row (unit tests, tooling).
/// Same contract as [`resolve`]: an unknown name is an error, never a silent fallback.
pub fn resolve_by_name(provider: &str) -> Result<Box<dyn PaymentProvider>, ProviderError> {
    match provider {
        "noop" => Ok(Box::new(NoopProvider)),
        "test" | "lemonsqueezy" => Ok(Box::new(TestProvider)),
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
        assert_eq!(
            resolve_by_name("lemonsqueezy").unwrap().name(),
            "test",
            "the legacy provider stays demoable on Test until PAY-12 removes it"
        );
    }

    // The previous contract silently substituted Test for anything unknown. That is the
    // behaviour PAY-1 removes: a typo in `payment_settings.provider` must stop checkout, not
    // quietly sell tickets nobody can be charged for.
    #[test]
    fn resolve_by_name_rejects_unknown_provider() {
        // `Box<dyn PaymentProvider>` is not Debug, so unwrap_err() is unavailable here.
        match resolve_by_name("wayforpay") {
            Ok(p) => panic!("unknown provider must not resolve, got {}", p.name()),
            Err(e) => assert!(matches!(e, ProviderError::Invalid(m) if m.contains("unknown payment provider"))),
        }
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

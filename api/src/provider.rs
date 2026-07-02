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
    #[error("invalid charge: {0}")]
    Invalid(String),
}

/// Result of asking the provider to collect money for an order.
#[derive(Debug, Clone, PartialEq)]
pub enum ChargeStatus {
    /// Buyer must complete payment at `checkout_url` (hosted redirect / SDK sheet).
    Pending,
    /// Funds captured to the platform balance — the order can be fulfilled now.
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
    },
    Refunded {
        order_ref: String,
    },
    /// Recognized but not actionable (or providers that never call back).
    Ignored,
}

#[derive(Debug, Clone)]
pub struct ChargeReq {
    pub order_id: Uuid,
    pub amount: Decimal,
    pub currency: String,
    pub return_url: Option<String>,
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
    pub reason: Option<String>,
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
    fn verify_webhook(&self, headers: &HeaderMap, raw: &[u8]) -> Result<WebhookEvent, ProviderError>;
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
    fn verify_webhook(&self, _headers: &HeaderMap, _raw: &[u8]) -> Result<WebhookEvent, ProviderError> {
        Ok(WebhookEvent::Ignored)
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
    fn verify_webhook(&self, _headers: &HeaderMap, _raw: &[u8]) -> Result<WebhookEvent, ProviderError> {
        Ok(WebhookEvent::Ignored)
    }
}

/// Select the active provider by its `payment_settings.provider` name. Product-first:
/// anything but an explicit `noop` resolves to the `Test` provider until a real PSP
/// adapter is wired (Phase 3–4), so the domain never accidentally moves real money.
pub fn resolve(provider: &str) -> Box<dyn PaymentProvider> {
    match provider {
        "noop" => Box::new(NoopProvider),
        _ => Box::new(TestProvider),
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
        let po = p
            .transfer_payout(PayoutReq {
                payout_id: Uuid::nil(),
                amount: Decimal::ZERO,
                currency: "UAH".into(),
            })
            .await
            .unwrap();
        assert_eq!(po.status, PayoutStatus::Paid);
        let rf = p
            .refund(RefundReq {
                provider_ref: "noop:x".into(),
                amount: Decimal::ZERO,
                reason: None,
            })
            .await
            .unwrap();
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
            .transfer_payout(PayoutReq {
                payout_id: Uuid::nil(),
                amount: Decimal::from_f64(100.0).unwrap(),
                currency: "UAH".into(),
            })
            .await
            .unwrap();
        assert_eq!(po.status, PayoutStatus::Paid);
        let rf = p
            .refund(RefundReq {
                provider_ref: "test:x".into(),
                amount: Decimal::ZERO,
                reason: None,
            })
            .await
            .unwrap();
        assert_eq!(rf.status, RefundStatus::Succeeded);
    }

    #[test]
    fn resolve_maps_names_and_defaults_to_test() {
        assert_eq!(resolve("noop").name(), "noop");
        assert_eq!(resolve("test").name(), "test");
        assert_eq!(
            resolve("lemonsqueezy").name(),
            "test",
            "product-first: unknown/real PSP → Test until wired"
        );
    }

    #[test]
    fn webhook_is_ignored_for_product_first_providers() {
        let h = HeaderMap::new();
        assert_eq!(NoopProvider.verify_webhook(&h, b"{}").unwrap(), WebhookEvent::Ignored);
        assert_eq!(TestProvider.verify_webhook(&h, b"{}").unwrap(), WebhookEvent::Ignored);
    }
}

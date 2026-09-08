//! Inbound payment webhooks (PAY-1).
//!
//! Two endpoints, one pipeline. Whatever the provider, an authenticated delivery becomes an
//! [`InboundWebhook`] and goes through [`apply`], which owns the three things that make a
//! webhook consumer correct and that no provider gives you for free:
//!
//! 1. **De-duplication by the provider's event id.** Delivery is at-least-once. The order's
//!    own `pending` status stops a double-mint of the *same* event but says nothing about a
//!    replay of a different one.
//! 2. **Ordering.** Delivery is unordered. A `refunded` that overtakes its `paid` must not
//!    leave the order mintable — hence `orders.terminal_event_seen`.
//! 3. **Answering 200 anyway.** A provider retries on any non-2xx. Our own business refusals
//!    ("no seats left") are not the provider's problem: retrying cannot fix them, so we
//!    acknowledge and resolve the money on our side.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

use crate::error::AppError;
use crate::orders;
use crate::payments;
use crate::provider::{InboundWebhook, WebhookEvent};
use crate::state::AppState;

type HmacSha256 = Hmac<Sha256>;

/// Hex HMAC-SHA256 of `body` under `secret` — the signature Lemon Squeezy sends (and
/// what a client/test produces to authenticate a webhook).
pub fn sign(secret: &str, body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(body);
    hex::encode(mac.finalize().into_bytes())
}

/// Constant-time verify the hex `signature` over `body` under `secret`.
fn verify_signature(secret: &str, body: &[u8], signature: &str) -> bool {
    let Ok(sig) = hex::decode(signature) else {
        return false;
    };
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(body);
    mac.verify_slice(&sig).is_ok()
}

/// Record the delivery. `false` means we have already seen this exact `(provider, event_id)`
/// and the caller must not run the side effect again.
///
/// The INSERT is the lock: two concurrent duplicates race on the primary key and exactly one
/// wins. An event with no id (providers that do not supply one) is always applied — the
/// order-level guards downstream still make that safe.
async fn record_delivery(db: &sqlx::PgPool, provider: &str, hook: &InboundWebhook) -> Result<bool, AppError> {
    let Some(event_id) = hook.id.as_deref() else {
        return Ok(true);
    };
    let order_ref = match &hook.event {
        WebhookEvent::Paid { order_ref, .. }
        | WebhookEvent::Refunded { order_ref }
        | WebhookEvent::Disputed { order_ref } => Some(order_ref.clone()),
        _ => None,
    };
    let inserted: Option<i32> = sqlx::query_scalar(
        "INSERT INTO webhook_events (provider, event_id, kind, order_ref) VALUES ($1, $2, $3, $4) \
         ON CONFLICT (provider, event_id) DO NOTHING RETURNING 1",
    )
    .bind(provider)
    .bind(event_id)
    .bind(&hook.kind)
    .bind(order_ref)
    .fetch_optional(db)
    .await?;
    Ok(inserted.is_some())
}

/// Mark an order terminal for the money and stop the escrow sweep from advancing it.
///
/// `escrow_status` moves to `refunded` (money returned) or `frozen` (charged back, pending an
/// admin decision). `terminal_event_seen` is what a late `Paid` checks before minting.
async fn mark_terminal(db: &sqlx::PgPool, order_id: Uuid, marker: &str, escrow: &str) -> Result<(), AppError> {
    sqlx::query("UPDATE orders SET terminal_event_seen = $2, escrow_status = $3 WHERE id = $1")
        .bind(order_id)
        .bind(marker)
        .bind(escrow)
        .execute(db)
        .await?;
    Ok(())
}

/// Apply one authenticated delivery. Always returns 200 unless the database itself failed:
/// a provider cannot fix our business state by retrying.
pub async fn apply(st: &AppState, provider: &str, hook: InboundWebhook) -> Result<StatusCode, AppError> {
    if !record_delivery(&st.db, provider, &hook).await? {
        tracing::info!(provider, event = ?hook.id, "webhook: duplicate delivery ignored");
        return Ok(StatusCode::OK);
    }
    match hook.event {
        WebhookEvent::Paid {
            order_ref,
            provider_ref,
        } => {
            let Ok(order_id) = Uuid::parse_str(&order_ref) else {
                return Ok(StatusCode::OK);
            };
            // A terminal event already arrived out of order — the money is gone, so the
            // ticket must not be issued. This is the ordering guard, and it is checked
            // before any minting rather than after.
            let terminal: Option<Option<String>> =
                sqlx::query_scalar("SELECT terminal_event_seen FROM orders WHERE id = $1")
                    .bind(order_id)
                    .fetch_optional(&st.db)
                    .await?;
            if let Some(Some(marker)) = terminal {
                tracing::warn!(%order_id, marker, "webhook: paid arrived after a terminal event, not minting");
                return Ok(StatusCode::OK);
            }
            // The refundable handle only exists once the payment completed: at checkout we
            // stored the session, and a refund can only be issued against what replaced it.
            if let Some(pr) = provider_ref.filter(|p| !p.is_empty()) {
                sqlx::query("UPDATE orders SET provider_ref = $2 WHERE id = $1")
                    .bind(order_id)
                    .bind(pr)
                    .execute(&st.db)
                    .await?;
            }
            let mut tx = st.db.begin().await?;
            // Serializable so two concurrent identical deliveries can't both slip past the
            // FOR UPDATE + pending-status check and double-mint (idempotency belt-and-braces).
            sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
                .execute(&mut *tx)
                .await?;
            // Idempotent: a duplicate delivery for an already-fulfilled order is a safe no-op,
            // and a payment that can no longer be honoured is a queued refund — never a 4xx
            // to the provider, which would only make it retry a state we own.
            let outcome = orders::fulfill(&mut tx, order_id).await?;
            tx.commit().await?;
            tracing::info!(%order_id, ?outcome, "webhook: payment applied");
            Ok(StatusCode::OK)
        }
        WebhookEvent::Refunded { order_ref } => {
            if let Ok(order_id) = Uuid::parse_str(&order_ref) {
                mark_terminal(&st.db, order_id, "refunded", "refunded").await?;
            }
            Ok(StatusCode::OK)
        }
        WebhookEvent::Disputed { order_ref } => {
            if let Ok(order_id) = Uuid::parse_str(&order_ref) {
                mark_terminal(&st.db, order_id, "disputed", "frozen").await?;
            }
            Ok(StatusCode::OK)
        }
        // The payout and account branches are recorded and acknowledged here; acting on them
        // is PAY-5 (organiser onboarding) and PAY-9 (payout confirmation). Recording them now
        // means the delivery is de-duplicated from day one rather than replayed later.
        WebhookEvent::PayoutPaid { payout_ref } => {
            tracing::info!(payout_ref, "webhook: payout paid");
            Ok(StatusCode::OK)
        }
        WebhookEvent::PayoutFailed { payout_ref, reason } => {
            tracing::warn!(payout_ref, reason, "webhook: payout failed");
            Ok(StatusCode::OK)
        }
        WebhookEvent::AccountUpdated {
            account_ref,
            charges_enabled,
            payouts_enabled,
            requirements_due,
        } => {
            tracing::info!(
                account_ref,
                charges_enabled,
                payouts_enabled,
                requirements_due,
                "webhook: connected account updated"
            );
            Ok(StatusCode::OK)
        }
        WebhookEvent::Ignored => Ok(StatusCode::OK),
    }
}

/// `POST /api/webhooks/lemonsqueezy` — public; authenticity = the signature.
pub async fn lemonsqueezy(State(st): State<AppState>, headers: HeaderMap, body: Bytes) -> Result<StatusCode, AppError> {
    let settings = payments::load_settings(&st.db).await?;
    let secret = settings
        .ls_webhook_secret
        .ok_or_else(|| AppError::Conflict("вебхук не налаштовано".into()))?;
    let signature = headers
        .get("x-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::Unauthorized)?;
    if !verify_signature(&secret, &body, signature) {
        return Err(AppError::Unauthorized);
    }
    let payload: serde_json::Value =
        serde_json::from_slice(&body).map_err(|_| AppError::Validation("invalid json".into()))?;
    let kind = payload["meta"]["event_name"].as_str().unwrap_or_default().to_string();
    // Act only on a completed purchase; acknowledge everything else so LS stops retrying.
    let event = if kind == "order_created" || kind == "order_paid" {
        match payload["meta"]["custom_data"]["order_id"].as_str() {
            Some(order_ref) => WebhookEvent::Paid {
                order_ref: order_ref.to_string(),
                provider_ref: None,
            },
            None => WebhookEvent::Ignored,
        }
    } else {
        WebhookEvent::Ignored
    };
    // Lemon Squeezy's own delivery id, when present, so replays de-duplicate like Stripe's.
    let id = payload["data"]["id"].as_str().map(str::to_string);
    apply(&st, "lemonsqueezy", InboundWebhook { id, kind, event }).await
}

/// `POST /api/webhooks/stripe` — public; authenticity = the `Stripe-Signature` header.
///
/// The provider is resolved from settings rather than hardcoded, so the signature is checked
/// with the secret the operator actually configured, and a stripe delivery to an instance
/// running another provider is rejected instead of half-processed.
pub async fn stripe(State(st): State<AppState>, headers: HeaderMap, body: Bytes) -> Result<StatusCode, AppError> {
    let settings = payments::load_settings(&st.db).await?;
    let provider = crate::provider::resolve(&settings)
        .map_err(|_| AppError::Conflict("платіжний провайдер не налаштований".into()))?;
    if provider.name() != "stripe" {
        return Err(AppError::Conflict("активний провайдер — не stripe".into()));
    }
    // A bad signature is indistinguishable from an attacker: never leak which check failed.
    let hook = provider.verify_webhook(&headers, &body).map_err(|e| {
        tracing::warn!(error = %e, "stripe webhook rejected");
        AppError::Unauthorized
    })?;
    apply(&st, "stripe", hook).await
}

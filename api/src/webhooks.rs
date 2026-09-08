//! Inbound payment webhooks (PAY-1, hardened after independent review).
//!
//! Two endpoints, one pipeline. Whatever the provider, an authenticated delivery becomes an
//! [`InboundWebhook`] and goes through [`apply`], which owns the four things that make a
//! webhook consumer correct and that no provider gives you for free:
//!
//! 1. **Binding.** A payment is applied to an order only if it arrived on the object WE
//!    created for that order (`orders.provider_ref`), from the provider the order was placed
//!    with, for the amount the order asked. `metadata.order_id` is a hint the payer can
//!    influence; it is never the authority.
//! 2. **De-duplication by the provider's event id**, recorded in the SAME transaction as the
//!    side effect. Recorded first and separately, a database failure mid-fulfilment would
//!    burn the event id forever while the order stayed unpaid.
//! 3. **Ordering.** Delivery is unordered. A `refunded` that overtakes its `paid` must not
//!    leave the order mintable — hence `orders.terminal_event_seen`.
//! 4. **Answering 200 anyway.** A provider retries on any non-2xx. Our own business refusals
//!    are not the provider's problem: retrying cannot fix them, so we acknowledge and resolve
//!    the money on our side (`fulfill` never fails on business grounds — PAY-3).

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use sqlx::PgConnection;
use uuid::Uuid;

use crate::error::AppError;
use crate::orders;
use crate::payments;
use crate::provider::{InboundWebhook, WebhookEvent};
use crate::state::AppState;

type HmacSha256 = Hmac<Sha256>;

/// REJECTED deliveries tolerated per minute per route before further unverified requests get
/// 429. Verified deliveries are never limited — Stripe treats 429 as a failed delivery and
/// stops retrying after ~3 days, so limiting the success path would let an anonymous flood
/// silently turn paid orders into unpaid ones.
const WEBHOOK_RATE_MAX: i64 = 600;
const WEBHOOK_RATE_WINDOW_SECS: i32 = 60;

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

/// Record the delivery inside the caller's transaction. `false` means this exact
/// `(provider, event_id)` was already applied and the caller must not run the side effect.
///
/// The INSERT is the lock: two concurrent duplicates race on the primary key and exactly one
/// wins. Because it shares the transaction with the side effect, a rollback also forgets the
/// delivery — the next retry gets a clean attempt instead of a "duplicate" that did nothing.
/// An event with no id (providers that do not supply one) is always applied — the order-level
/// guards downstream still make that safe.
async fn record_delivery(conn: &mut PgConnection, provider: &str, hook: &InboundWebhook) -> Result<bool, AppError> {
    let Some(event_id) = hook.id.as_deref() else {
        return Ok(true);
    };
    let order_ref = match &hook.event {
        WebhookEvent::Paid { order_ref, .. }
        | WebhookEvent::Refunded { order_ref, .. }
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
    .fetch_optional(&mut *conn)
    .await?;
    Ok(inserted.is_some())
}

/// The order an event refers to, if we can prove the reference is ours.
///
/// `order_ref` is either our own order id (from metadata we set) or the provider's object id
/// (a PaymentIntent, when metadata was not carried — disputes). Either way the order must
/// have been placed with THIS provider; a Stripe event can never touch a test order.
async fn resolve_order(conn: &mut PgConnection, provider: &str, order_ref: &str) -> Result<Option<Uuid>, AppError> {
    if let Ok(id) = Uuid::parse_str(order_ref) {
        let owned: Option<Uuid> = sqlx::query_scalar("SELECT id FROM orders WHERE id = $1 AND provider = $2")
            .bind(id)
            .bind(provider)
            .fetch_optional(&mut *conn)
            .await?;
        return Ok(owned);
    }
    let by_ref: Option<Uuid> = sqlx::query_scalar("SELECT id FROM orders WHERE provider = $1 AND provider_ref = $2")
        .bind(provider)
        .bind(order_ref)
        .fetch_optional(&mut *conn)
        .await?;
    Ok(by_ref)
}

/// A refused delivery leaves a trail even though its dedup row is rolled back on purpose:
/// what arrived, which order it named, why it was not applied. Written on its own connection
/// AFTER the rollback so the trail survives the refusal (REVIEW-3 #7). Best effort at the call
/// sites: a forensic table must never change the answer Stripe gets — a 500 here would make it
/// retry, for days, a delivery we refused on purpose (REVIEW-4 #4).
async fn record_refusal(
    db: &sqlx::PgPool,
    provider: &str,
    event_id: Option<&str>,
    order_ref: &str,
    reason: &str,
) -> Result<(), AppError> {
    sqlx::query("INSERT INTO webhook_refusals (provider, event_id, order_ref, reason) VALUES ($1, $2, $3, $4)")
        .bind(provider)
        .bind(event_id)
        .bind(order_ref)
        .bind(reason)
        .execute(db)
        .await?;
    Ok(())
}

/// Mark an order terminal for the money and stop the escrow sweep from advancing it.
/// `terminal_event_seen` is what a late `Paid` checks before minting.
async fn mark_terminal(conn: &mut PgConnection, order_id: Uuid, marker: &str, escrow: &str) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE orders SET terminal_event_seen = COALESCE(terminal_event_seen, $2), escrow_status = $3 WHERE id = $1",
    )
    .bind(order_id)
    .bind(marker)
    .bind(escrow)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Apply one authenticated delivery. Always returns 200 unless the database itself failed:
/// a provider cannot fix our business state by retrying.
pub async fn apply(st: &AppState, provider: &str, hook: InboundWebhook) -> Result<StatusCode, AppError> {
    let mut tx = st.db.begin().await?;
    // Serializable so two concurrent identical deliveries cannot both slip past the FOR UPDATE
    // + status checks and double-mint. The dedup INSERT below is the first line of defence;
    // this is the belt to its braces.
    sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
        .execute(&mut *tx)
        .await?;
    if !record_delivery(&mut tx, provider, &hook).await? {
        tx.commit().await?;
        tracing::info!(provider, event = ?hook.id, "webhook: duplicate delivery ignored");
        return Ok(StatusCode::OK);
    }
    match hook.event {
        WebhookEvent::Paid {
            order_ref,
            provider_ref,
            charge_ref,
            amount_minor,
            currency,
        } => {
            let Some(order_id) = resolve_order(&mut tx, provider, &order_ref).await? else {
                tracing::warn!(
                    provider,
                    order_ref,
                    "webhook: paid event names no order of this provider"
                );
                // Rolled back, not committed: the delivery is NOT recorded, so a corrected resend
                // of the same event id is applied instead of dropped as a duplicate.
                tx.rollback().await?;
                if let Err(e) = record_refusal(
                    &st.db,
                    provider,
                    hook.id.as_deref(),
                    &order_ref,
                    "no order of this provider",
                )
                .await
                {
                    tracing::error!(error = %e, "webhook: refusal not recorded");
                }
                return Ok(StatusCode::OK);
            };
            // (provider_ref, amount, currency, terminal_event_seen) — everything the binding needs.
            type Row = (Option<String>, rust_decimal::Decimal, String, Option<String>);
            let (stored_ref, amount, order_currency, terminal): Row = sqlx::query_as(
                "SELECT provider_ref, amount, currency, terminal_event_seen FROM orders WHERE id = $1 FOR UPDATE",
            )
            .bind(order_id)
            .fetch_one(&mut *tx)
            .await?;
            // THE binding. The event arrived on a specific object; at checkout we stored the id
            // of the object we created for this order. They must be the same object. Without
            // this, a payer who pays for a cheap order with an expensive order's id in the
            // metadata gets the expensive ticket.
            // An event without a charge reference cannot be bound and, with Lemon Squeezy retired,
            // no longer exists; one flat condition keeps the refusal on a single branch.
            if charge_ref.is_some() && charge_ref.as_deref() != stored_ref.as_deref() {
                tracing::warn!(%order_id, ?charge_ref, ?stored_ref, "webhook: paid event is not for the object we created — ignored");
                tx.rollback().await?;
                if let Err(e) = record_refusal(
                    &st.db,
                    provider,
                    hook.id.as_deref(),
                    &order_ref,
                    "charge is not the object created for this order",
                )
                .await
                {
                    tracing::error!(error = %e, "webhook: refusal not recorded");
                }
                return Ok(StatusCode::OK);
            }
            // And the money must match what the order asked for. A partial or tampered amount
            // is recorded and not honoured.
            let expected = crate::stripe::to_minor(amount, &order_currency).ok();
            let mismatch = amount_minor
                .zip(currency.as_deref())
                .filter(|(paid, cur)| expected != Some(*paid) || !cur.eq_ignore_ascii_case(&order_currency));
            if let Some((paid, cur)) = mismatch {
                tracing::warn!(%order_id, paid, ?expected, cur, %order_currency, "webhook: paid amount does not match the order — ignored");
                tx.rollback().await?;
                if let Err(e) = record_refusal(
                    &st.db,
                    provider,
                    hook.id.as_deref(),
                    &order_ref,
                    "amount or currency does not match the order",
                )
                .await
                {
                    tracing::error!(error = %e, "webhook: refusal not recorded");
                }
                return Ok(StatusCode::OK);
            }
            // A terminal event already arrived out of order — the money is gone, so the ticket
            // must not be issued. Checked before any minting rather than after.
            if let Some(marker) = terminal.as_deref() {
                // Closed before this money arrived. `fulfill` owns the decision (refund it, or
                // nothing — the provider already returned it); we only make it visible here,
                // and fall through so the PaymentIntent below is stored for the refund.
                tracing::warn!(%order_id, marker, "webhook: paid arrived after a terminal event");
            }
            // The refundable handle only exists once the payment completed: at checkout we
            // stored the session, and a refund can only be issued against what replaced it.
            if let Some(pr) = provider_ref.filter(|p| !p.is_empty()) {
                sqlx::query("UPDATE orders SET provider_ref = $2 WHERE id = $1")
                    .bind(order_id)
                    .bind(pr)
                    .execute(&mut *tx)
                    .await?;
            }
            // Idempotent, and never a business failure: a replay is `AlreadyHandled`, a payment
            // that can no longer be honoured is a queued refund (PAY-3).
            let outcome = orders::fulfill(&mut tx, order_id).await?;
            tx.commit().await?;
            tracing::info!(%order_id, ?outcome, "webhook: payment applied");
            Ok(StatusCode::OK)
        }
        WebhookEvent::Refunded {
            order_ref,
            refunded_minor,
            total_minor,
        } => {
            if let Some(order_id) = resolve_order(&mut tx, provider, &order_ref).await? {
                // Full only when proven full: the charge total is known and the cumulative
                // refunded amount reaches it. A partial refund — or a Refund object that does
                // not know the total — is bookkeeping, not a returned ticket.
                let full = matches!((refunded_minor, total_minor), (Some(r), Some(t)) if r >= t)
                    || (refunded_minor.is_none() && total_minor.is_none());
                // A partial amount is bookkeeping only when the refund is NOT proven full.
                let partial = if full { None } else { refunded_minor };
                if full {
                    orders::apply_full_refund(&mut tx, order_id).await?;
                }
                if let Some(r) = partial {
                    let (currency,): (String,) = sqlx::query_as("SELECT currency FROM orders WHERE id = $1")
                        .bind(order_id)
                        .fetch_one(&mut *tx)
                        .await?;
                    sqlx::query("UPDATE orders SET refunded_amount = $2 WHERE id = $1")
                        .bind(order_id)
                        .bind(crate::stripe::from_minor(r, &currency))
                        .execute(&mut *tx)
                        .await?;
                    tracing::info!(%order_id, refunded = r, "webhook: partial refund recorded");
                }
            }
            tx.commit().await?;
            Ok(StatusCode::OK)
        }
        WebhookEvent::Disputed { order_ref } => {
            if let Some(order_id) = resolve_order(&mut tx, provider, &order_ref).await? {
                mark_terminal(&mut tx, order_id, "disputed", "frozen").await?;
            }
            tx.commit().await?;
            Ok(StatusCode::OK)
        }
        // The payout and account branches are recorded and acknowledged here; acting on them
        // is PAY-5 (organiser onboarding) and PAY-9 (payout confirmation). Recording them now
        // means the delivery is de-duplicated from day one rather than replayed later.
        WebhookEvent::PayoutPaid { payout_ref } => {
            tx.commit().await?;
            tracing::info!(payout_ref, "webhook: payout paid");
            Ok(StatusCode::OK)
        }
        WebhookEvent::PayoutFailed { payout_ref, reason } => {
            tx.commit().await?;
            tracing::warn!(payout_ref, reason, "webhook: payout failed");
            Ok(StatusCode::OK)
        }
        WebhookEvent::AccountUpdated {
            account_ref,
            charges_enabled,
            payouts_enabled,
            requirements_due,
        } => {
            tx.commit().await?;
            tracing::info!(
                account_ref,
                charges_enabled,
                payouts_enabled,
                requirements_due,
                "webhook: connected account updated"
            );
            Ok(StatusCode::OK)
        }
        WebhookEvent::Ignored => {
            tx.commit().await?;
            Ok(StatusCode::OK)
        }
    }
}

/// `POST /api/webhooks/lemonsqueezy` — public; authenticity = the signature.
///
/// Lemon Squeezy is retired (PAY-12). The route stays so a stale endpoint configured at LS
/// does not 404 into an infinite retry, but nothing it says can move money: no order is
/// placed with provider `lemonsqueezy` any more, so the binding in [`apply`] makes every
/// delivery a recorded no-op. The `custom_data.order_id` it carries was always a URL parameter
/// the buyer could edit — which is exactly why the binding exists.
pub async fn lemonsqueezy(State(st): State<AppState>, headers: HeaderMap, body: Bytes) -> Result<StatusCode, AppError> {
    let settings = payments::load_settings(&st.db).await?;
    let secret = settings
        .ls_webhook_secret
        .ok_or_else(|| AppError::Conflict("вебхук не налаштовано".into()))?;
    let signature = headers.get("x-signature").and_then(|v| v.to_str().ok());
    // A missing header is a rejected delivery like any other and is metered the same way
    // (REVIEW-3 #8); before, it returned 401 above the limiter and cost a SELECT each time.
    if !signature.is_some_and(|s| verify_signature(&secret, &body, s)) {
        // Only REJECTED deliveries are rate-limited: a verified one is never told to wait.
        crate::ratelimit::hit(
            &st.db,
            "webhook:lemonsqueezy:rejected",
            WEBHOOK_RATE_MAX,
            WEBHOOK_RATE_WINDOW_SECS,
        )
        .await?;
        return Err(AppError::Unauthorized);
    }
    let payload: serde_json::Value =
        serde_json::from_slice(&body).map_err(|_| AppError::Validation("invalid json".into()))?;
    let kind = payload["meta"]["event_name"].as_str().unwrap_or_default().to_string();
    // Never `Paid`: an LS event carries no charge_ref and no amount, so the binding in `apply`
    // would have nothing to bind on. Recorded and acknowledged, never applied.
    let event = WebhookEvent::Ignored;
    // The DELIVERY id, not the order id: `data.id` is the same for every event about one order,
    // so using it made the first event permanently block all later ones.
    let id = payload["meta"]["webhook_id"].as_str().map(str::to_string);
    apply(&st, "lemonsqueezy", InboundWebhook { id, kind, event }).await
}

/// `POST /api/webhooks/stripe` — public; authenticity = the `Stripe-Signature` header.
///
/// The provider is resolved from settings rather than hardcoded, so the signature is checked
/// with the secret the operator actually configured, and a stripe delivery to an instance
/// running another provider is rejected instead of half-processed. Every outcome of the
/// signature check is counted, so an operator can see a wrong secret from the checklist
/// instead of from a buyer's complaint.
pub async fn stripe(State(st): State<AppState>, headers: HeaderMap, body: Bytes) -> Result<StatusCode, AppError> {
    let settings = payments::load_settings(&st.db).await?;
    let provider = crate::provider::resolve(&settings)
        .map_err(|_| AppError::Conflict("платіжний провайдер не налаштований".into()))?;
    if provider.name() != "stripe" {
        return Err(AppError::Conflict("активний провайдер — не stripe".into()));
    }
    let hook = match provider.verify_webhook(&headers, &body) {
        Ok(h) => h,
        Err(e) => {
            // A bad signature is indistinguishable from an attacker: never leak which check
            // failed. Count it, so a misconfigured secret is visible.
            tracing::warn!(error = %e, "stripe webhook rejected");
            // Only REJECTED deliveries are rate-limited. Counting before verification let 600
            // anonymous POSTs make a real, signed Stripe delivery 429 — and Stripe gives up
            // retrying after ~3 days, which is money taken and no ticket (REVIEW-2 N1).
            // And the throttle comes BEFORE the rejection counter: that counter is a write to
            // the same settings row a verified delivery updates, so an unmetered flood of it
            // was lock contention on the money path (REVIEW-3 #3).
            crate::ratelimit::hit(
                &st.db,
                "webhook:stripe:rejected",
                WEBHOOK_RATE_MAX,
                WEBHOOK_RATE_WINDOW_SECS,
            )
            .await?;
            payments::note_webhook_rejected(&st.db).await?;
            return Err(AppError::Unauthorized);
        }
    };
    payments::note_webhook_ok(&st.db).await?;
    apply(&st, "stripe", hook).await
}

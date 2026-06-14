//! Lemon Squeezy webhook receiver (TASK-017 M6). Authenticity comes from the
//! HMAC-SHA256 `X-Signature` over the raw body (secret configured via admin); on a
//! paid order we fulfill the matching pending order (mint ticket + escrow held).

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

use crate::error::AppError;
use crate::orders;
use crate::payments;
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
    let event = payload["meta"]["event_name"].as_str().unwrap_or_default();
    // Act only on a completed purchase; acknowledge everything else so LS stops retrying.
    if event != "order_created" && event != "order_paid" {
        return Ok(StatusCode::OK);
    }
    let order_id = payload["meta"]["custom_data"]["order_id"]
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok());
    let Some(order_id) = order_id else {
        return Ok(StatusCode::OK);
    };
    let mut tx = st.db.begin().await?;
    // Serializable so two concurrent identical deliveries can't both slip past the
    // FOR UPDATE + pending-status check and double-mint (idempotency belt-and-braces).
    sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
        .execute(&mut *tx)
        .await?;
    // Idempotent: a duplicate delivery for an already-fulfilled order is a safe no-op.
    orders::fulfill(&mut tx, order_id).await?;
    tx.commit().await?;
    Ok(StatusCode::OK)
}

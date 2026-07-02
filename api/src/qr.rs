//! Signed, short-TTL rotating door-QR tokens (TASK-118-B, BR-023, UC-007/UC-014).
//!
//! The QR shown on an e-ticket is a signed HS256 token over the stable `ticket_id` (+ `event_id`)
//! with a short expiry, minted on the fly and auto-refreshed by clients. The raw `qr_token`
//! column stays in the DB as a stable key but is never exposed in the UI, so a screenshot of a
//! stale QR (or the old plaintext token) can't be replayed at the door. Validation accepts a
//! signed token (preferred) or, during migration, the raw `qr_token` (backward compat).

use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;

/// Distinguishes a door-QR token from any other HS256 token minted with the same secret
/// (auth JWTs), so one can never be substituted for the other.
const QR_KIND: &str = "qr";

/// Door-QR lifetime in seconds, overridable via env; default 120s. Non-positive → default.
fn ttl_secs() -> i64 {
    std::env::var("QR_TOKEN_TTL_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|n| *n > 0)
        .unwrap_or(120)
}

#[derive(Serialize, Deserialize)]
pub struct QrClaims {
    pub tid: Uuid,
    pub eid: Uuid,
    pub exp: usize,
    pub kind: String,
}

/// Mint a signed, short-TTL QR token for a ticket. Returns the token and its absolute expiry.
pub fn sign_qr(ticket_id: Uuid, event_id: Uuid, secret: &str) -> Result<(String, DateTime<Utc>), AppError> {
    let expires_at = Utc::now() + Duration::seconds(ttl_secs());
    let claims = QrClaims {
        tid: ticket_id,
        eid: event_id,
        exp: expires_at.timestamp() as usize,
        kind: QR_KIND.to_string(),
    };
    let token = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|_| AppError::Internal)?;
    Ok((token, expires_at))
}

/// Verify a scanned string as a signed QR token. Returns `None` when it is not a valid,
/// unexpired, correctly-typed QR token — callers then fall back to treating the string as a
/// raw `qr_token` (migration). HS256 pinned (no algorithm substitution).
pub fn verify_qr(token: &str, secret: &str) -> Option<QrClaims> {
    let data = decode::<QrClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .ok()?;
    if data.claims.kind != QR_KIND {
        return None;
    }
    Some(data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_then_verify_roundtrips() {
        let tid = Uuid::new_v4();
        let eid = Uuid::new_v4();
        let (token, exp) = sign_qr(tid, eid, "secret").unwrap();
        assert!(exp > Utc::now());
        let claims = verify_qr(&token, "secret").expect("valid token verifies");
        assert_eq!(claims.tid, tid);
        assert_eq!(claims.eid, eid);
    }

    #[test]
    fn verify_rejects_wrong_secret() {
        let (token, _) = sign_qr(Uuid::new_v4(), Uuid::new_v4(), "secret").unwrap();
        assert!(verify_qr(&token, "other").is_none(), "[SEC] wrong secret is rejected");
    }

    #[test]
    fn verify_rejects_non_qr_kind() {
        // A token with the same shape but a different `kind` must not pass as a QR.
        let claims = QrClaims {
            tid: Uuid::new_v4(),
            eid: Uuid::new_v4(),
            exp: (Utc::now().timestamp() + 60) as usize,
            kind: "auth".to_string(),
        };
        let token = encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(b"secret"),
        )
        .unwrap();
        assert!(verify_qr(&token, "secret").is_none(), "[SEC] non-qr kind is rejected");
    }

    #[test]
    fn verify_rejects_expired() {
        let claims = QrClaims {
            tid: Uuid::new_v4(),
            eid: Uuid::new_v4(),
            exp: (Utc::now().timestamp() - 3600) as usize,
            kind: QR_KIND.to_string(),
        };
        let token = encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(b"secret"),
        )
        .unwrap();
        assert!(verify_qr(&token, "secret").is_none(), "[SEC] expired QR is rejected");
    }

    #[test]
    fn verify_rejects_garbage() {
        assert!(verify_qr("not-a-jwt", "secret").is_none());
    }

    #[test]
    fn ttl_honors_env_then_defaults() {
        std::env::set_var("QR_TOKEN_TTL_SECS", "30");
        assert_eq!(ttl_secs(), 30);
        std::env::set_var("QR_TOKEN_TTL_SECS", "0");
        assert_eq!(ttl_secs(), 120, "non-positive TTL falls back to the default");
        std::env::remove_var("QR_TOKEN_TTL_SECS");
        assert_eq!(ttl_secs(), 120);
    }
}

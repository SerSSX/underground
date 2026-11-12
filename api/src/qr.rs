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
    /// The ticket owner at mint time (TASK-302 CT10). The door accepts the QR only while the
    /// ticket still belongs to this user, so a QR kept by a previous owner (transfer, gift)
    /// is dead the moment the ticket changes hands. Required: a token without it is invalid.
    pub oid: Uuid,
    pub exp: usize,
    pub kind: String,
}

/// Mint a signed, short-TTL QR token for a ticket owned by `owner_id`. Returns the token and
/// its absolute expiry.
pub fn sign_qr(
    ticket_id: Uuid,
    event_id: Uuid,
    owner_id: Uuid,
    secret: &str,
) -> Result<(String, DateTime<Utc>), AppError> {
    let expires_at = Utc::now() + Duration::seconds(ttl_secs());
    let claims = QrClaims {
        tid: ticket_id,
        eid: event_id,
        oid: owner_id,
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
/// raw `qr_token` (migration). HS256 pinned (no algorithm substitution). No clock leeway
/// (TASK-302 CT10, P4): jsonwebtoken defaults to 60 s, which would stretch a 120 s QR to 180 s.
pub fn verify_qr(token: &str, secret: &str) -> Option<QrClaims> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.leeway = 0;
    let data = decode::<QrClaims>(token, &DecodingKey::from_secret(secret.as_bytes()), &validation).ok()?;
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
        let oid = Uuid::new_v4();
        let (token, exp) = sign_qr(tid, eid, oid, "secret").unwrap();
        assert!(exp > Utc::now());
        let claims = verify_qr(&token, "secret").expect("valid token verifies");
        assert_eq!(claims.tid, tid);
        assert_eq!(claims.eid, eid);
        assert_eq!(claims.oid, oid);
    }

    #[test]
    fn verify_rejects_wrong_secret() {
        let (token, _) = sign_qr(Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4(), "secret").unwrap();
        assert!(verify_qr(&token, "other").is_none(), "[SEC] wrong secret is rejected");
    }

    #[test]
    fn verify_rejects_non_qr_kind() {
        // A token with the same shape but a different `kind` must not pass as a QR.
        let claims = QrClaims {
            tid: Uuid::new_v4(),
            eid: Uuid::new_v4(),
            oid: Uuid::new_v4(),
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
            oid: Uuid::new_v4(),
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

    fn token_expiring_at(exp: i64) -> String {
        let claims = QrClaims {
            tid: Uuid::new_v4(),
            eid: Uuid::new_v4(),
            oid: Uuid::new_v4(),
            exp: exp as usize,
            kind: QR_KIND.to_string(),
        };
        encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(b"secret"),
        )
        .unwrap()
    }

    /// TASK-302 S02 / CT10: no clock leeway. jsonwebtoken's default 60 s leeway would accept
    /// both of these.
    #[test]
    fn verify_rejects_just_expired_without_leeway() {
        let now = Utc::now().timestamp();
        assert!(
            verify_qr(&token_expiring_at(now - 1), "secret").is_none(),
            "[SEC] exp = now - 1 s"
        );
        assert!(
            verify_qr(&token_expiring_at(now - 30), "secret").is_none(),
            "[SEC] exp = now - 30 s"
        );
        assert!(
            verify_qr(&token_expiring_at(now + 30), "secret").is_some(),
            "unexpired still verifies"
        );
    }

    /// TASK-302 CT10: a token minted before the owner claim existed does not verify.
    #[test]
    fn verify_rejects_token_without_owner() {
        #[derive(Serialize)]
        struct Legacy {
            tid: Uuid,
            eid: Uuid,
            exp: usize,
            kind: String,
        }
        let legacy = Legacy {
            tid: Uuid::new_v4(),
            eid: Uuid::new_v4(),
            exp: (Utc::now().timestamp() + 60) as usize,
            kind: QR_KIND.to_string(),
        };
        let token = encode(
            &Header::new(Algorithm::HS256),
            &legacy,
            &EncodingKey::from_secret(b"secret"),
        )
        .unwrap();
        assert!(
            verify_qr(&token, "secret").is_none(),
            "[SEC] QR without oid is rejected"
        );
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

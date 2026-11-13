//! Passwordless email one-time code login (TASK-115, UC-001, WF-Auth).
//!
//! The frictionless auth for the gift/claim loop: a user enters an email, gets a short-lived
//! 6-digit code, and verifying it signs them in — creating the account on first use (no
//! password). Only the SHA-256 hash of a code is stored; codes are single-use and expire.
//! `request_code` always returns 204 (even for unknown emails) so it can't enumerate accounts.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use chrono::{Duration, Utc};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::auth::{normalize_email, AuthResp};
use crate::error::AppError;
use crate::state::AppState;
use crate::users;

/// How long a login code stays valid.
const CODE_TTL_MINS: i64 = 10;
/// Soft cap on codes issued per email per 10 min window (full rate-limit is TASK-113).
const MAX_CODES_PER_WINDOW: i64 = 5;

/// SHA-256 hex of a raw code — only the hash is persisted. Public so tests can seed a row.
pub fn code_hash(raw: &str) -> String {
    hex::encode(Sha256::digest(raw.as_bytes()))
}

fn valid_email(email: &str) -> bool {
    email.len() >= 3 && email.contains('@') && !email.contains(' ')
}

/// Generate a zero-padded 6-digit numeric code from the OS CSPRNG.
fn generate_code() -> String {
    use argon2::password_hash::rand_core::{OsRng, RngCore};
    format!("{:06}", OsRng.next_u32() % 1_000_000)
}

#[derive(Deserialize)]
pub struct RequestCodeReq {
    pub email: String,
    /// The UI language; the mail uses the account's stored locale when there is one, else this.
    pub locale: Option<String>,
}

/// `POST /api/auth/email/request-code` — email a login code. Always 204 (anti-enumeration).
pub async fn request_code(
    State(st): State<AppState>,
    ip: crate::ratelimit::ClientIp,
    Json(req): Json<RequestCodeReq>,
) -> Result<StatusCode, AppError> {
    // Own per-IP bucket (N4): this is also the LOGIN path for password-less accounts, so it must
    // not share register's tight sign-up budget. Existing emails are not exempt — a 429 that
    // only unknown emails could get would tell whether an account exists.
    crate::ratelimit::hit(
        &st.db,
        &format!("code-ip:{}", ip.0),
        crate::ratelimit::env_limit("CODE_LIMIT_PER_IP", 60),
        3600,
    )
    .await?;
    let email = normalize_email(&req.email);
    if !valid_email(&email) {
        return Err(AppError::Validation("invalid email".into()));
    }
    let recent: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM login_codes WHERE email = $1 AND created_at > now() - interval '10 minutes'",
    )
    .bind(&email)
    .fetch_one(&st.db)
    .await?;
    if recent < MAX_CODES_PER_WINDOW {
        let code = generate_code();
        let expires = Utc::now() + Duration::minutes(CODE_TTL_MINS);
        sqlx::query("INSERT INTO login_codes (email, code_hash, expires_at) VALUES ($1, $2, $3)")
            .bind(&email)
            .bind(code_hash(&code))
            .bind(expires)
            .execute(&st.db)
            .await?;
        // The code itself is only in the mail (sent from a background task, so this answer does
        // not wait on it); logs never carry it nor the address.
        let stored: Option<String> = sqlx::query_scalar(
            "SELECT p.locale FROM users u JOIN user_prefs p ON p.user_id = u.id WHERE u.email = $1 AND u.blocked = false",
        )
        .bind(&email)
        .fetch_optional(&st.db)
        .await?;
        let locale = stored.or(req.locale);
        crate::mail::enqueue(crate::mail::code_mail(locale.as_deref(), &email, &code, CODE_TTL_MINS));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct VerifyCodeReq {
    pub email: String,
    pub code: String,
    /// The UI language, applied only when this call creates the account (TASK-303).
    pub locale: Option<String>,
}

/// `POST /api/auth/email/verify-code` — sign in with a valid code, creating the account on
/// first use. Merges with an existing (Google/email) account by email (BR-007).
pub async fn verify_code(
    State(st): State<AppState>,
    Json(req): Json<VerifyCodeReq>,
) -> Result<Json<AuthResp>, AppError> {
    let email = normalize_email(&req.email);
    // Throttle code-guessing (BR-017): 10 verify attempts / 10 min per email.
    crate::ratelimit::hit(&st.db, &format!("verify:{email}"), 10, 600).await?;
    // Atomically claim the code: only an unused, unexpired match for this email succeeds.
    let claimed: Option<Uuid> = sqlx::query_scalar(
        "UPDATE login_codes SET used_at = now() \
         WHERE email = $1 AND code_hash = $2 AND used_at IS NULL AND expires_at > now() RETURNING id",
    )
    .bind(&email)
    .bind(code_hash(req.code.trim()))
    .fetch_optional(&st.db)
    .await?;
    if claimed.is_none() {
        return Err(AppError::Unauthorized);
    }
    let user = match users::find_by_email(&st.db, &email).await? {
        Some(u) => u,
        // Lost a create race, or the email belongs to a blocked account: re-fetch active only.
        None => match users::create_passwordless_user(&st.db, &email).await {
            Ok(u) => {
                crate::prefs::seed_locale(&st.db, u.id, req.locale.as_deref()).await?;
                u
            }
            Err(_) => users::find_by_email(&st.db, &email)
                .await?
                .ok_or(AppError::Unauthorized)?,
        },
    };
    let token = crate::auth::issue_token(user.id, &st.jwt_secret)?;
    crate::telemetry::log(&st.db, Some(user.id), "login_code", serde_json::json!({})).await;
    Ok(Json(AuthResp { token, user }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_hash_is_stable_sha256_hex() {
        let h = code_hash("123456");
        assert_eq!(h.len(), 64);
        assert_eq!(h, code_hash("123456"));
        assert_ne!(h, code_hash("123457"));
    }

    #[test]
    fn generated_code_is_six_digits() {
        let c = generate_code();
        assert_eq!(c.len(), 6);
        assert!(c.chars().all(|ch| ch.is_ascii_digit()));
    }

    #[test]
    fn email_validation() {
        assert!(valid_email("a@b.co"));
        assert!(!valid_email("nope"));
        assert!(!valid_email("a b@c.co"));
    }
}

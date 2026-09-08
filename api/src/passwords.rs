//! Password reset (structural bet): time-boxed, single-use reset tokens delivered by email.
//!
//! Only the SHA-256 hash of a token is ever stored, so a DB read cannot reset anyone's
//! password. `forgot_password` always returns 204 — including for unknown addresses — so the
//! endpoint can't be used to enumerate which emails are registered. Email transport is the same
//! documented SendGrid follow-up as order confirmations (env-gated in `main.rs`); for now the
//! reset link is logged so the flow is observable end-to-end in dev.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use chrono::{Duration, Utc};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::auth::{hash_password, normalize_email};
use crate::error::AppError;
use crate::state::AppState;

/// How long a reset link stays valid.
const RESET_TTL_MINS: i64 = 60;

#[derive(Deserialize)]
pub struct ForgotReq {
    pub email: String,
}

#[derive(Deserialize)]
pub struct ResetReq {
    pub token: String,
    pub password: String,
}

/// Public web app base URL: reset links, and the return URL a hosted checkout sends the
/// buyer back to. Trailing slash trimmed so callers can append a path unconditionally.
pub fn web_base_url() -> String {
    std::env::var("WEB_APP_URL")
        .ok()
        .map(|s| s.trim().trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "http://localhost:3001".into())
}

/// SHA-256 hex of a raw token — what we persist and look up by. Public so tests can seed a
/// reset row with a known hash (the raw token never crosses the API surface otherwise).
pub fn token_hash(raw: &str) -> String {
    hex::encode(Sha256::digest(raw.as_bytes()))
}

/// Build the (subject, body) of a reset email (pure → unit-testable). The link carries the raw token.
pub fn build_reset_email(base_url: &str, raw_token: &str) -> (String, String) {
    let subject = "Скидання паролю underground".to_string();
    let link = format!("{base_url}/reset-password?token={raw_token}");
    let body = format!(
        "Ви запросили скидання паролю.\n\nВідкрийте посилання (дійсне 60 хв):\n{link}\n\n\
         Якщо це були не ви — просто проігноруйте цей лист.",
    );
    (subject, body)
}

/// `POST /api/auth/forgot-password` — start a reset. Always 204, even for unknown/blocked emails.
pub async fn forgot_password(State(st): State<AppState>, Json(req): Json<ForgotReq>) -> Result<StatusCode, AppError> {
    let email = normalize_email(&req.email);
    let user_id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE email = $1 AND blocked = false")
        .bind(&email)
        .fetch_optional(&st.db)
        .await?;
    if let Some(uid) = user_id {
        // Two v4 UUIDs → ~244 bits of CSPRNG entropy in the raw token; only its hash is stored.
        let raw = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let expires = Utc::now() + Duration::minutes(RESET_TTL_MINS);
        sqlx::query("INSERT INTO password_resets (user_id, token_hash, expires_at) VALUES ($1, $2, $3)")
            .bind(uid)
            .bind(token_hash(&raw))
            .bind(expires)
            .execute(&st.db)
            .await?;
        let (subject, body) = build_reset_email(&web_base_url(), &raw);
        // info: observable that a reset was sent, WITHOUT the token. The link (which contains the
        // raw token) is debug-only so it never lands in production logs — a prod log reader must
        // not be able to reset accounts. The real delivery is the env-gated SendGrid follow-up.
        tracing::info!(target: "email", to = %email, subject = %subject, "password reset email queued");
        tracing::debug!(target: "email", %body, "password reset link (dev)");
    }
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /api/auth/reset-password` — complete a reset with a valid token + new password.
pub async fn reset_password(State(st): State<AppState>, Json(req): Json<ResetReq>) -> Result<StatusCode, AppError> {
    if req.password.len() < 8 {
        return Err(AppError::Validation("password must be at least 8 characters".into()));
    }
    let pw_hash = hash_password(&req.password)?;
    // Whole reset is one transaction: claim token → rotate password → invalidate other tokens.
    let mut tx = st.db.begin().await?;
    // Atomically claim the token: only an unexpired, unused row matches, and stamping used_at in
    // the same UPDATE makes it single-use even under concurrent requests.
    let user_id: Option<Uuid> = sqlx::query_scalar(
        "UPDATE password_resets SET used_at = now() \
         WHERE token_hash = $1 AND used_at IS NULL AND expires_at > now() \
         RETURNING user_id",
    )
    .bind(token_hash(&req.token))
    .fetch_optional(&mut *tx)
    .await?;
    let user_id = user_id.ok_or_else(|| AppError::Conflict("посилання недійсне або протерміноване".into()))?;
    // Only an active account can be reset (a blocked/erased user must not regain a password), and
    // bumping tokens_valid_after instantly invalidates any token issued before this reset.
    let updated = sqlx::query(
        "UPDATE users SET password_hash = $1, tokens_valid_after = now() WHERE id = $2 AND blocked = false",
    )
    .bind(&pw_hash)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(AppError::Conflict("акаунт недоступний".into()));
    }
    // Invalidate any other outstanding reset tokens for this user.
    sqlx::query("UPDATE password_resets SET used_at = now() WHERE user_id = $1 AND used_at IS NULL")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_hash_is_stable_sha256_hex() {
        // Deterministic, 64 hex chars, and different inputs differ.
        let h = token_hash("abc");
        assert_eq!(h.len(), 64);
        assert_eq!(h, token_hash("abc"));
        assert_ne!(h, token_hash("abd"));
    }

    #[test]
    fn reset_email_carries_link_and_token() {
        let (subject, body) = build_reset_email("https://app.example", "tok123");
        assert!(subject.contains("Скидання"));
        assert!(body.contains("https://app.example/reset-password?token=tok123"));
    }

    #[test]
    fn web_base_url_defaults_then_honors_env() {
        // WEB_APP_URL is not part of the test environment; remove_var fully restores the unset
        // state afterwards so this can't leak into other tests (matches uploads.rs's pattern).
        std::env::remove_var("WEB_APP_URL");
        assert_eq!(web_base_url(), "http://localhost:3001");
        std::env::set_var("WEB_APP_URL", "https://underground.app");
        assert_eq!(web_base_url(), "https://underground.app");
        std::env::remove_var("WEB_APP_URL");
    }
}

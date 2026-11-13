//! Passwords (TASK-304): reset by emailed link, and change while signed in.
//!
//! Reset tokens are single-use, valid for [`RESET_TTL_MINS`], and only their SHA-256 hash is
//! stored, so a DB read cannot reset anyone's password. `forgot_password` answers 204 with an
//! empty body for every address — registered, unknown or blocked — after the same rate-limit and
//! mail-budget checks, so the ANSWER does not tell whether an account exists. Its TIMING does, a
//! little: only a registered address gets the token writes (~0.3 ms more, measured by review);
//! the mail itself goes out from a background task (`mail::enqueue`), so Resend's latency is not
//! visible. Existence is not secret here anyway: sign-up answers 409 for a registered address
//! (rate-limited per IP and per address). Asking again replaces the previous link.
//!
//! A reset and a change both sign out every session: `tokens_valid_after` moves to the next whole
//! second, because JWT `iat` has one-second resolution and the check is `iat >= watermark`; with
//! plain `now()` a token minted in the same second would survive. The change answers a fresh token
//! minted at exactly that second.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use chrono::{Duration, Utc};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::auth::{hash_password_async, normalize_email, verify_password_async, CurrentUser};
use crate::error::AppError;
use crate::ratelimit::{env_limit, hit, ClientIp};
use crate::state::AppState;

/// How long a reset link stays valid.
pub const RESET_TTL_MINS: i64 = 30;
/// Password length bounds (characters). The upper one keeps argon2 from hashing megabytes.
const MIN_PASSWORD: usize = 8;
const MAX_PASSWORD: usize = 256;
/// How recent a password-less user's sign-in must be to SET a first password (as for erasure).
const RECENT_LOGIN_SECS: i64 = 10 * 60;

#[derive(Deserialize)]
pub struct ForgotReq {
    pub email: String,
    /// The UI language; used for the mail only when the account has no stored locale.
    pub locale: Option<String>,
}

#[derive(Deserialize)]
pub struct ResetReq {
    pub token: String,
    pub password: String,
}

/// Public web app base URL for the reset link (the page lives in the public client, not the API).
fn web_base_url() -> String {
    std::env::var("WEB_APP_URL").unwrap_or_else(|_| "http://localhost:3001".into())
}

/// SHA-256 hex of a raw token — what we persist and look up by. Public so tests can seed a
/// reset row with a known hash (the raw token never crosses the API surface otherwise).
pub fn token_hash(raw: &str) -> String {
    hex::encode(Sha256::digest(raw.as_bytes()))
}

/// The reset page link; it carries the raw token (hex only, so no escaping is needed).
pub fn reset_link(base_url: &str, raw_token: &str) -> String {
    format!("{}/reset-password?token={raw_token}", base_url.trim_end_matches('/'))
}

/// The (subject, plain-text body) of the Ukrainian reset mail (pure → unit-testable).
pub fn build_reset_email(base_url: &str, raw_token: &str) -> (String, String) {
    let m = crate::mail::reset_mail(Some("uk"), "", &reset_link(base_url, raw_token), RESET_TTL_MINS);
    (m.subject, m.text)
}

/// Bounds in characters (as the messages and the web's `minLength`/`maxLength` say), not bytes:
/// "жжжж" is 4 characters, 8 bytes.
fn check_new_password(password: &str) -> Result<(), AppError> {
    let chars = password.chars().count();
    if chars < MIN_PASSWORD {
        return Err(AppError::Validation("password must be at least 8 characters".into()));
    }
    if chars > MAX_PASSWORD {
        return Err(AppError::Validation("пароль задовгий (до 256 символів)".into()));
    }
    Ok(())
}

/// `POST /api/auth/forgot-password` — start a reset. Always 204 with no body (unknown, blocked
/// and registered addresses alike); 429/503 only from limits and the mail budget, which count
/// every address the same way.
pub async fn forgot_password(
    State(st): State<AppState>,
    ip: ClientIp,
    Json(req): Json<ForgotReq>,
) -> Result<StatusCode, AppError> {
    let email = normalize_email(&req.email);
    // Both limits run before the lookup, for every address: a 429 says nothing about accounts.
    hit(
        &st.db,
        &format!("forgot-ip:{}", ip.0),
        env_limit("FORGOT_LIMIT_PER_IP", 20),
        3600,
    )
    .await?;
    hit(
        &st.db,
        &format!("forgot:{email}"),
        env_limit("FORGOT_LIMIT_PER_EMAIL", 5),
        3600,
    )
    .await?;
    // A mail slot before the lookup, for every address alike (TASK-304 review): a spent daily
    // budget is 503 "mail temporarily unavailable" for any address, never a pretended "sent".
    if crate::mail::enabled() {
        crate::mail::reserve(&st.db, &email, &ip.0).await?;
    }
    let found: Option<(Uuid, Option<String>)> = sqlx::query_as(
        "SELECT u.id, p.locale FROM users u LEFT JOIN user_prefs p ON p.user_id = u.id \
         WHERE u.email = $1 AND u.blocked = false",
    )
    .bind(&email)
    .fetch_optional(&st.db)
    .await?;
    if let Some((uid, locale)) = found {
        // Two v4 UUIDs → ~244 bits of CSPRNG entropy in the raw token; only its hash is stored.
        let raw = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let expires = Utc::now() + Duration::minutes(RESET_TTL_MINS);
        let mut tx = st.db.begin().await?;
        // Only the newest link works: earlier unused ones are retired.
        sqlx::query("UPDATE password_resets SET used_at = now() WHERE user_id = $1 AND used_at IS NULL")
            .bind(uid)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO password_resets (user_id, token_hash, expires_at) VALUES ($1, $2, $3)")
            .bind(uid)
            .bind(token_hash(&raw))
            .bind(expires)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        let locale = locale.or(req.locale);
        let link = reset_link(&web_base_url(), &raw);
        crate::mail::enqueue(crate::mail::reset_mail(
            locale.as_deref(),
            &email,
            &link,
            RESET_TTL_MINS,
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /api/auth/reset-password` — complete a reset with a valid token + new password.
pub async fn reset_password(
    State(st): State<AppState>,
    ip: ClientIp,
    Json(req): Json<ResetReq>,
) -> Result<StatusCode, AppError> {
    hit(
        &st.db,
        &format!("reset-ip:{}", ip.0),
        env_limit("RESET_LIMIT_PER_IP", 30),
        3600,
    )
    .await?;
    check_new_password(&req.password)?;
    let pw_hash = hash_password_async(&req.password).await?;
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
    // Only an active account can be reset (a blocked/erased user must not regain a password).
    // The watermark goes to the NEXT whole second, so every token issued up to and including this
    // second dies (`iat` is whole seconds and the check is `iat >= tokens_valid_after`).
    let updated = sqlx::query(
        "UPDATE users SET password_hash = $1, \
         tokens_valid_after = date_trunc('second', now()) + interval '1 second' \
         WHERE id = $2 AND blocked = false",
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

#[derive(Deserialize)]
pub struct ChangeReq {
    /// The current password; required when the account has one.
    pub current: Option<String>,
    pub new: String,
}

#[derive(serde::Serialize)]
pub struct ChangeResp {
    /// A fresh token for this device: every token issued before the change no longer works.
    pub token: String,
}

/// `POST /api/me/password` — change (or, for a password-less account, set) the password.
///
/// Password accounts must send the right `current` (wrong → 422, not 401, so a client never
/// mistakes it for "signed out"). Password-less accounts (email code, Google) may set one only within
/// 10 minutes of signing in (401 "sign in again" otherwise), as for account erasure. Every other
/// session is signed out; the answer carries the new token for this one.
pub async fn change_password(
    user: CurrentUser,
    State(st): State<AppState>,
    Json(req): Json<ChangeReq>,
) -> Result<Json<ChangeResp>, AppError> {
    // Guessing the current password with a stolen token: 10 tries per hour per account.
    hit(
        &st.db,
        &format!("pwchange:{}", user.id),
        env_limit("PWCHANGE_LIMIT_PER_USER", 10),
        3600,
    )
    .await?;
    check_new_password(&req.new)?;
    let hash: Option<String> = sqlx::query_scalar("SELECT password_hash FROM users WHERE id = $1 AND blocked = false")
        .bind(user.id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::Unauthorized)?;
    match hash {
        Some(hash) => {
            let current = req.current.as_deref().unwrap_or("");
            if current.is_empty() {
                return Err(AppError::Validation("введіть поточний пароль".into()));
            }
            if !verify_password_async(current, Some(&hash)).await? {
                return Err(AppError::Validation("неправильний поточний пароль".into()));
            }
        }
        None => {
            if Utc::now().timestamp() - user.iat > RECENT_LOGIN_SECS {
                return Err(AppError::ReauthRequired("увійдіть ще раз, щоб задати пароль".into()));
            }
        }
    }
    let pw_hash = hash_password_async(&req.new).await?;
    // One transaction: the new password, the watermark and the retired reset links land together.
    let mut tx = st.db.begin().await?;
    // The watermark goes one whole second past now; the new token is minted AT that second.
    let valid_after: chrono::DateTime<Utc> = sqlx::query_scalar(
        "UPDATE users SET password_hash = $1, tokens_valid_after = date_trunc('second', now()) + interval '1 second' \
         WHERE id = $2 AND blocked = false RETURNING tokens_valid_after",
    )
    .bind(&pw_hash)
    .bind(user.id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::Unauthorized)?;
    // A reset link asked for before the change must not undo it.
    sqlx::query("UPDATE password_resets SET used_at = now() WHERE user_id = $1 AND used_at IS NULL")
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    let token = crate::auth::issue_token_at(user.id, &st.jwt_secret, valid_after.timestamp())?;
    crate::telemetry::log(&st.db, Some(user.id), "password_changed", serde_json::json!({})).await;
    Ok(Json(ChangeResp { token }))
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
    fn password_bounds() {
        assert!(check_new_password("1234567").is_err());
        assert!(check_new_password("12345678").is_ok());
        assert!(check_new_password(&"x".repeat(256)).is_ok());
        assert!(check_new_password(&"x".repeat(257)).is_err());
        // Characters, not bytes (Cyrillic is 2 bytes each).
        assert!(
            check_new_password(&"ж".repeat(200)).is_ok(),
            "200 chars = 400 bytes: accepted"
        );
        assert!(check_new_password(&"ж".repeat(256)).is_ok());
        assert!(check_new_password(&"ж".repeat(257)).is_err());
        assert!(
            check_new_password(&"ж".repeat(4)).is_err(),
            "4 chars = 8 bytes: too short"
        );
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

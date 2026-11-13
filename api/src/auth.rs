//! Email/password authentication: argon2id hashing, HS256 JWT, handlers, extractor.

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use async_trait::async_trait;
use axum::extract::{FromRequestParts, State};
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::Json;
use chrono::{NaiveDate, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::state::AppState;
use crate::users::{self, User};

const TOKEN_TTL_SECS: i64 = 7 * 24 * 3600;

pub fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|_| AppError::Internal)
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .and_then(|parsed| Argon2::default().verify_password(password.as_bytes(), &parsed))
        .is_ok()
}

#[derive(Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    /// Issued-at (unix seconds). Compared against `users.tokens_valid_after` so a password reset
    /// can invalidate every token minted before it. `default` keeps pre-`iat` tokens decodable.
    #[serde(default)]
    pub iat: i64,
}

pub fn issue_token(user_id: Uuid, secret: &str) -> Result<String, AppError> {
    issue_token_at(user_id, secret, Utc::now().timestamp())
}

/// A token whose `iat` is `iat` (unix seconds). Used after a password change, where `iat` must
/// equal the new `tokens_valid_after` watermark (one second ahead of now).
pub fn issue_token_at(user_id: Uuid, secret: &str, iat: i64) -> Result<String, AppError> {
    let claims = Claims {
        sub: user_id.to_string(),
        exp: (iat + TOKEN_TTL_SECS) as usize,
        iat,
    };
    // Pin HS256 explicitly on both sides (encode + decode) to prevent algorithm-substitution.
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|_| AppError::Internal)
}

/// Decode + verify a token, returning its claims (sub + iat). HS256 pinned.
pub fn decode_claims(token: &str, secret: &str) -> Result<Claims, AppError> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .map(|d| d.claims)
    .map_err(|_| AppError::Unauthorized)
}

pub fn decode_user_id(token: &str, secret: &str) -> Result<Uuid, AppError> {
    let claims = decode_claims(token, secret)?;
    Uuid::parse_str(&claims.sub).map_err(|_| AppError::Unauthorized)
}

/// Canonical email form for storage and lookup — trimmed + lowercased, so login and
/// uniqueness are case-insensitive.
pub(crate) fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

fn validate_email(email: &str) -> bool {
    let e = email.trim();
    e.len() >= 3 && e.contains('@') && !e.contains(' ')
}

#[derive(Deserialize)]
pub struct RegisterReq {
    pub email: String,
    pub password: String,
    pub name: String,
    /// The UI language at sign-up (TASK-303): one of `prefs::SUPPORTED_LOCALES`; others ignored.
    pub locale: Option<String>,
}

#[derive(Deserialize)]
pub struct LoginReq {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct AuthResp {
    pub token: String,
    pub user: User,
}

pub async fn register(
    State(st): State<AppState>,
    ip: crate::ratelimit::ClientIp,
    Json(req): Json<RegisterReq>,
) -> Result<(StatusCode, Json<AuthResp>), AppError> {
    let email = normalize_email(&req.email);
    // TASK-302 fix batch 1, item 3: the 409 below says whether an email has an account, and each
    // attempt used to cost an argon2 hash. Throttle per client IP and per email first.
    crate::ratelimit::hit_signup_ip(&st.db, &ip).await?;
    crate::ratelimit::hit(
        &st.db,
        &format!("register:{email}"),
        crate::ratelimit::env_limit("REGISTER_LIMIT_PER_EMAIL", 5),
        3600,
    )
    .await?;
    if !validate_email(&email) {
        return Err(AppError::Validation("invalid email".into()));
    }
    if req.password.len() < 8 {
        return Err(AppError::Validation("password must be at least 8 characters".into()));
    }
    if req.name.trim().is_empty() {
        return Err(AppError::Validation("name is required".into()));
    }
    // Same cap as `PATCH /api/me` (TASK-302 P12): the name fills guest lists and scanner screens.
    if req.name.trim().len() > 80 {
        return Err(AppError::Validation("name is too long".into()));
    }
    // Cheap uniqueness check before the expensive hash; the unique index stays the race backstop.
    let taken: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)")
        .bind(&email)
        .fetch_one(&st.db)
        .await?;
    if taken {
        return Err(AppError::Conflict("email already registered".into()));
    }
    let hash = hash_password(&req.password)?;
    let user = match users::create_user(&st.db, &email, req.name.trim(), &hash).await {
        Ok(u) => u,
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            return Err(AppError::Conflict("email already registered".into()));
        }
        Err(e) => return Err(AppError::Db(e)),
    };
    crate::prefs::seed_locale(&st.db, user.id, req.locale.as_deref()).await?;
    let token = issue_token(user.id, &st.jwt_secret)?;
    crate::telemetry::log(&st.db, Some(user.id), "signup", serde_json::json!({})).await;
    Ok((StatusCode::CREATED, Json(AuthResp { token, user })))
}

pub async fn login(State(st): State<AppState>, Json(req): Json<LoginReq>) -> Result<Json<AuthResp>, AppError> {
    let email = normalize_email(&req.email);
    // Throttle brute-force against a single account (BR-017): 10 attempts / 5 min per email.
    crate::ratelimit::hit(&st.db, &format!("login:{email}"), 10, 300).await?;
    let creds = users::find_credentials(&st.db, &email)
        .await?
        .ok_or(AppError::Unauthorized)?;
    let hash = creds.password_hash.ok_or(AppError::Unauthorized)?;
    if !verify_password(&req.password, &hash) {
        return Err(AppError::Unauthorized);
    }
    let user = users::find_by_id(&st.db, creds.id)
        .await?
        .ok_or(AppError::Unauthorized)?;
    let token = issue_token(user.id, &st.jwt_secret)?;
    Ok(Json(AuthResp { token, user }))
}

pub struct CurrentUser {
    pub id: Uuid,
    /// Issued-at of the bearer token (unix seconds): how recently the user signed in. Used for
    /// step-up checks on destructive actions (TASK-302 fix batch 1, item 2).
    pub iat: i64,
}

#[async_trait]
impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or(AppError::Unauthorized)?;
        let token = header.strip_prefix("Bearer ").ok_or(AppError::Unauthorized)?;
        let claims = decode_claims(token, &state.jwt_secret)?;
        let id = Uuid::parse_str(&claims.sub).map_err(|_| AppError::Unauthorized)?;
        // A still-valid token must not outlive an account block/deletion, nor a password reset.
        // One indexed PK probe gates every authenticated handler centrally: reject blocked accounts
        // and any token issued before the user's `tokens_valid_after` watermark (reset cuts off
        // older, possibly-stolen tokens). Per-handler `blocked = false` filters stay as defense-in-depth.
        let row: Option<(bool, chrono::DateTime<Utc>)> =
            sqlx::query_as("SELECT blocked, tokens_valid_after FROM users WHERE id = $1")
                .bind(id)
                .fetch_optional(&state.db)
                .await?;
        match row {
            Some((false, valid_after)) if claims.iat >= valid_after.timestamp() => {
                Ok(CurrentUser { id, iat: claims.iat })
            }
            _ => Err(AppError::Unauthorized),
        }
    }
}

pub async fn me(user: CurrentUser, State(st): State<AppState>) -> Result<Json<User>, AppError> {
    users::find_by_id(&st.db, user.id)
        .await?
        .map(Json)
        .ok_or(AppError::NotFound)
}

#[derive(Deserialize)]
pub struct PatchMeReq {
    pub name: Option<String>,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub is_organizer: Option<bool>,
    /// Date of birth (YYYY-MM-DD) — used for age-gating restricted events (P1 #77).
    pub dob: Option<NaiveDate>,
}

/// `PATCH /api/me` — update the caller's own profile (name, bio, avatar, organizer label).
pub async fn patch_me(
    user: CurrentUser,
    State(st): State<AppState>,
    Json(req): Json<PatchMeReq>,
) -> Result<Json<User>, AppError> {
    let name = req.name.as_deref().map(str::trim);
    if name.is_some_and(str::is_empty) {
        return Err(AppError::Validation("name must not be blank".into()));
    }
    if name.is_some_and(|n| n.len() > 80) {
        return Err(AppError::Validation("name is too long".into()));
    }
    // Bio: trim; an explicit empty string clears it.
    let bio = req.bio.as_deref().map(str::trim);
    if bio.is_some_and(|b| b.len() > 500) {
        return Err(AppError::Validation("bio must be at most 500 chars".into()));
    }
    let avatar_url = req.avatar_url.as_deref().map(str::trim).filter(|a| !a.is_empty());
    if req.dob.is_some_and(|d| d > Utc::now().date_naive()) {
        return Err(AppError::Validation(
            "дата народження не може бути в майбутньому".into(),
        ));
    }
    users::update_profile(&st.db, user.id, name, bio, avatar_url, req.is_organizer, req.dob)
        .await?
        .map(Json)
        .ok_or(AppError::NotFound)
}

/// Extractor that requires an authenticated user with the `ADMIN` role (else 403).
pub struct CurrentAdmin {
    pub id: Uuid,
}

#[async_trait]
impl FromRequestParts<AppState> for CurrentAdmin {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let user = CurrentUser::from_request_parts(parts, state).await?;
        let role: Option<String> = sqlx::query_scalar("SELECT role FROM users WHERE id = $1 AND blocked = false")
            .bind(user.id)
            .fetch_optional(&state.db)
            .await?;
        match role.as_deref() {
            Some("ADMIN") => Ok(CurrentAdmin { id: user.id }),
            _ => Err(AppError::Forbidden),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_then_verify_roundtrip() {
        let hash = hash_password("correct horse").unwrap();
        assert!(!hash.contains("correct horse"), "hash must not contain the plaintext");
        assert!(verify_password("correct horse", &hash));
        assert!(!verify_password("wrong", &hash));
    }

    #[test]
    fn verify_rejects_malformed_hash() {
        assert!(!verify_password("x", "not-a-phc-hash"));
    }

    #[test]
    fn token_roundtrip_and_rejections() {
        let id = Uuid::new_v4();
        let token = issue_token(id, "secret").unwrap();
        assert_eq!(decode_user_id(&token, "secret").unwrap(), id);
        assert!(decode_user_id(&token, "other-secret").is_err());
        assert!(decode_user_id("garbage", "secret").is_err());
    }

    #[test]
    fn email_validation() {
        assert!(validate_email("a@b.co"));
        assert!(!validate_email("nope"));
        assert!(!validate_email("a b@c.co"));
    }

    #[test]
    fn normalize_email_trims_and_lowercases() {
        assert_eq!(normalize_email("  Mixed@Case.Co "), "mixed@case.co");
    }

    #[test]
    fn rejects_token_signed_with_other_algorithm() {
        // A token forged with a different (still-symmetric) algorithm must not validate —
        // decode pins HS256, so an HS384 token under the same secret is rejected.
        let claims = Claims {
            sub: Uuid::new_v4().to_string(),
            exp: (Utc::now().timestamp() + 3600) as usize,
            iat: Utc::now().timestamp(),
        };
        let token = encode(
            &Header::new(Algorithm::HS384),
            &claims,
            &EncodingKey::from_secret(b"secret"),
        )
        .unwrap();
        assert!(decode_user_id(&token, "secret").is_err());
    }
}

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

/// The answer for an avatar that is not one of our own uploads (TASK-305 R4b).
pub const AVATAR_REFUSED: &str = "фото профілю має бути зображенням, завантаженим на сайт";

/// Display-name and bio caps, in characters (TASK-305 F10).
pub const NAME_MAX: usize = 80;
pub const BIO_MAX: usize = 500;

pub fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|_| AppError::Internal)
}

/// A hash of a password nobody knows, made with the same parameters as real ones. Login verifies
/// against it when the account does not exist or has no password, so that answer takes as long
/// as a wrong password (TASK-305 F4: no account-existence oracle by timing).
static DUMMY_HASH: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| hash_password(&Uuid::new_v4().to_string()).unwrap_or_default());

pub fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .and_then(|parsed| Argon2::default().verify_password(password.as_bytes(), &parsed))
        .is_ok()
}

/// Build the dummy hash now (startup), so the first login for an unknown account does not pay
/// for it and answer slower than the ones after (TASK-305 R8).
pub fn warm_dummy_hash() {
    std::sync::LazyLock::force(&DUMMY_HASH);
}

/// Most argon2 jobs running at once (TASK-305 R8). Each takes ~19 MiB and a core for tens of
/// milliseconds; more in parallel would only queue on the CPU and grow memory.
pub const HASH_JOBS: usize = 2;

static HASH_PERMITS: std::sync::LazyLock<std::sync::Arc<tokio::sync::Semaphore>> =
    std::sync::LazyLock::new(|| std::sync::Arc::new(tokio::sync::Semaphore::new(HASH_JOBS)));

/// Run CPU-heavy hashing work off the async workers (`spawn_blocking`), at most [`HASH_JOBS`] at
/// a time. The permit moves into the blocking job, so a caller that goes away (client
/// disconnect) does not free a slot while its hash is still running.
pub async fn run_hash_job<T, F>(job: F) -> Result<T, AppError>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let permit = HASH_PERMITS
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| AppError::Internal)?;
    tokio::task::spawn_blocking(move || {
        let out = job();
        drop(permit);
        out
    })
    .await
    .map_err(|_| AppError::Internal)
}

/// [`hash_password`] through [`run_hash_job`].
pub async fn hash_password_async(password: &str) -> Result<String, AppError> {
    let password = password.to_owned();
    run_hash_job(move || hash_password(&password)).await?
}

/// [`verify_password`] through [`run_hash_job`]; `None` verifies against the dummy hash (the
/// answer is always false, the time is a real verify's).
pub async fn verify_password_async(password: &str, hash: Option<&str>) -> Result<bool, AppError> {
    let password = password.to_owned();
    let hash = hash.map(str::to_owned);
    run_hash_job(move || verify_password(&password, hash.as_deref().unwrap_or(DUMMY_HASH.as_str()))).await
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

/// One plain address (TASK-304 review): an account email later receives reset mail, so it must
/// never name several recipients (`a@x.co,b@y.co`) or carry a display name.
fn validate_email(email: &str) -> bool {
    crate::mail::is_single_address(email.trim())
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
    // Controls and bidi controls out (TASK-305 F8); same cap as `PATCH /api/me`, in characters
    // (TASK-302 P12, TASK-305 F10): the name fills guest lists and scanner screens.
    let name = crate::text::clean_line(&req.name);
    if crate::text::is_blank(&name) {
        return Err(AppError::Validation("name is required".into()));
    }
    if name.chars().count() > NAME_MAX {
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
    let hash = hash_password_async(&req.password).await?;
    let user = match users::create_user(&st.db, &email, &name, &hash).await {
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

pub async fn login(
    State(st): State<AppState>,
    ip: crate::ratelimit::ClientIp,
    Json(req): Json<LoginReq>,
) -> Result<Json<AuthResp>, AppError> {
    let email = normalize_email(&req.email);
    // Password spraying across many emails from one client (TASK-305 F4): 30 per 10 min per IP.
    crate::ratelimit::hit(
        &st.db,
        &format!("login-ip:{}", ip.0),
        crate::ratelimit::env_limit("LOGIN_LIMIT_PER_IP", 30),
        600,
    )
    .await?;
    // Throttle brute-force against a single account (BR-017): 10 attempts / 5 min per email.
    crate::ratelimit::hit(&st.db, &format!("login:{email}"), 10, 300).await?;
    let creds = users::find_credentials(&st.db, &email).await?;
    let hash = creds.as_ref().and_then(|c| c.password_hash.as_deref());
    // Always one argon2 verify, against the dummy hash when there is no real one (F4).
    let matched = verify_password_async(&req.password, hash).await?;
    let creds = creds
        .filter(|c| matched && c.password_hash.is_some())
        .ok_or(AppError::Unauthorized)?;
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
    // Controls and bidi controls out (TASK-305 F8); caps in characters (F10).
    let name = req.name.as_deref().map(crate::text::clean_line);
    if name.as_deref().is_some_and(crate::text::is_blank) {
        return Err(AppError::Validation("name must not be blank".into()));
    }
    if name.as_deref().is_some_and(|n| n.chars().count() > NAME_MAX) {
        return Err(AppError::Validation("name is too long".into()));
    }
    // Bio: cleaned and trimmed, newlines kept; an explicit empty string clears it.
    let bio = req.bio.as_deref().map(crate::text::clean_multiline);
    if bio.as_deref().is_some_and(|b| b.chars().count() > BIO_MAX) {
        return Err(AppError::Validation("bio must be at most 500 chars".into()));
    }
    let avatar_url = req.avatar_url.as_deref().map(str::trim).filter(|a| !a.is_empty());
    // Only a picture uploaded here (TASK-305 R4b): our own `/uploads/` file, relative or under
    // `PUBLIC_BASE_URL`. A third-party URL would let a profile load anything from anywhere.
    if avatar_url.is_some_and(|a| !crate::uploads::is_own_upload_url(a, &crate::uploads::public_base_url())) {
        return Err(AppError::Validation(AVATAR_REFUSED.into()));
    }
    if req.dob.is_some_and(|d| d > Utc::now().date_naive()) {
        return Err(AppError::Validation(
            "дата народження не може бути в майбутньому".into(),
        ));
    }
    users::update_profile(
        &st.db,
        user.id,
        name.as_deref(),
        bio.as_deref(),
        avatar_url,
        req.is_organizer,
        req.dob,
    )
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

    /// R8: at most HASH_JOBS hashing jobs run at once, and on blocking threads: with ONE async
    /// worker, two jobs still overlap (inline on the worker they could not), and never more than
    /// two. The async verify gives the same verdicts as the sync one.
    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn hash_jobs_run_off_the_runtime_two_at_a_time() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        let running = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let jobs = (0..8).map(|_| {
            let (running, peak) = (running.clone(), peak.clone());
            tokio::spawn(run_hash_job(move || {
                let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                std::thread::sleep(std::time::Duration::from_millis(60));
                running.fetch_sub(1, Ordering::SeqCst);
            }))
        });
        for job in jobs.collect::<Vec<_>>() {
            job.await.unwrap().unwrap();
        }
        assert_eq!(
            peak.load(Ordering::SeqCst),
            HASH_JOBS,
            "[SEC] {HASH_JOBS} at once, off the single async worker"
        );

        let hash = hash_password_async("correct horse").await.unwrap();
        assert!(verify_password_async("correct horse", Some(&hash)).await.unwrap());
        assert!(!verify_password_async("wrong", Some(&hash)).await.unwrap());
        assert!(
            !verify_password_async("x", None).await.unwrap(),
            "dummy hash never matches"
        );
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

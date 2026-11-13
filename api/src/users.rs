//! User model + repository.

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

/// Public user shape — never includes `password_hash`.
#[derive(Serialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub role: String,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
    /// Self-serve organizer label (cosmetic — anyone can create events).
    pub is_organizer: bool,
    /// Date of birth (self-view only) — drives client-side age-gating (P1 #77).
    pub dob: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
    /// Whether the account has a password (TASK-304): /account asks for the current one to change
    /// it, or offers to set a first one. Never the hash itself.
    pub has_password: bool,
}

const USER_COLS: &str =
    "id, email, name, role, avatar_url, bio, is_organizer, dob, created_at, (password_hash IS NOT NULL) AS has_password";

#[derive(sqlx::FromRow)]
pub struct Credentials {
    pub id: Uuid,
    pub password_hash: Option<String>,
}

pub async fn create_user(db: &PgPool, email: &str, name: &str, password_hash: &str) -> Result<User, sqlx::Error> {
    let sql = format!("INSERT INTO users (email, name, password_hash) VALUES ($1, $2, $3) RETURNING {USER_COLS}");
    sqlx::query_as::<_, User>(&sql)
        .bind(email)
        .bind(name)
        .bind(password_hash)
        .fetch_one(db)
        .await
}

pub async fn find_by_id(db: &PgPool, id: Uuid) -> Result<Option<User>, sqlx::Error> {
    let sql = format!("SELECT {USER_COLS} FROM users WHERE id = $1 AND blocked = false");
    sqlx::query_as::<_, User>(&sql).bind(id).fetch_optional(db).await
}

/// Partial profile update (PATCH /api/me). Each field is optional — null leaves it unchanged.
pub async fn update_profile(
    db: &PgPool,
    id: Uuid,
    name: Option<&str>,
    bio: Option<&str>,
    avatar_url: Option<&str>,
    is_organizer: Option<bool>,
    dob: Option<NaiveDate>,
) -> Result<Option<User>, sqlx::Error> {
    let sql = format!(
        "UPDATE users SET \
            name = COALESCE($2, name), \
            bio = COALESCE($3, bio), \
            avatar_url = COALESCE($4, avatar_url), \
            is_organizer = COALESCE($5, is_organizer), \
            dob = COALESCE($6, dob) \
         WHERE id = $1 AND blocked = false RETURNING {USER_COLS}"
    );
    sqlx::query_as::<_, User>(&sql)
        .bind(id)
        .bind(name)
        .bind(bio)
        .bind(avatar_url)
        .bind(is_organizer)
        .bind(dob)
        .fetch_optional(db)
        .await
}

/// Whole years from `dob` as of `today` (pure, for deterministic tests).
fn age_on(dob: NaiveDate, today: NaiveDate) -> i32 {
    let mut age = today.year() - dob.year();
    if (today.month(), today.day()) < (dob.month(), dob.day()) {
        age -= 1;
    }
    age
}

/// Name shown to organisers and door staff for an attendee who never chose a display name
/// (TASK-302 CT8, decision by the orchestrator: a neutral placeholder rather than asking for a
/// name before the first booking).
pub const GUEST_FALLBACK_NAME: &str = "Гість";

/// The display-name rule as a string literal, so `concat!` can build constant queries from it
/// (TASK-305 R1). Everything else uses [`DISPLAY_NAME_SQL`]; no other copy of the rule exists
/// (`tests/task305_round2.rs` checks the sources).
///
/// «Гість» is shown only when the stored name is blank or is «Гість» itself (trimmed, any
/// letter case). A name a person typed is always shown, even when it equals their email's local
/// part (docs/decisions.md 2026-11-14). It never reads `u.email`. The case fold uses
/// `translate`, not `lower`: `lower` follows the database locale and leaves Cyrillic alone under
/// the C locale. The generated column `name_is_placeholder` (migration 0048) is no longer read.
#[macro_export]
macro_rules! display_name_sql {
    () => {
        "CASE WHEN u.name IS NULL OR btrim(u.name) = '' \
            OR translate(btrim(u.name), 'ГІСТЬ', 'гість') = 'гість' \
         THEN 'Гість' ELSE u.name END"
    };
}

/// SQL expression (users aliased `u`) for the name everyone but the user themself sees: the
/// user's name, or [`GUEST_FALLBACK_NAME`] when it is blank or «Гість» (see
/// [`display_name_sql!`]).
pub const DISPLAY_NAME_SQL: &str = display_name_sql!();

/// True if the user satisfies an event's minimum-age `age_rating`. No restriction
/// (none/unparseable) → always true; a restricted event requires a known dob old enough.
///
/// Generic over the executor so a caller holding row locks inside a transaction can run it on
/// that same connection (TASK-302: borrowing a second pool connection while holding the event
/// lock lets a burst of bookings exhaust the pool).
pub async fn meets_age<'e, E: sqlx::PgExecutor<'e>>(
    db: E,
    user_id: Uuid,
    age_rating: Option<&str>,
) -> Result<bool, sqlx::Error> {
    let Some(min) = age_rating.and_then(|s| s.parse::<i32>().ok()).filter(|&m| m > 0) else {
        return Ok(true);
    };
    let dob: Option<NaiveDate> = sqlx::query_scalar("SELECT dob FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(db)
        .await?;
    Ok(dob.is_some_and(|d| age_on(d, Utc::now().date_naive()) >= min))
}

/// Look up an active user by email (blocked users are invisible — TASK-115 passwordless login).
pub async fn find_by_email(db: &PgPool, email: &str) -> Result<Option<User>, sqlx::Error> {
    let sql = format!("SELECT {USER_COLS} FROM users WHERE email = $1 AND blocked = false");
    sqlx::query_as::<_, User>(&sql).bind(email).fetch_optional(db).await
}

/// Create a passwordless user (Google / email-code sign-up) — `password_hash` stays NULL.
/// The display name starts as [`GUEST_FALLBACK_NAME`] — never derived from the email, which
/// would publish most of the address on profiles, reviews, chats and gift previews (TASK-302
/// fix batch 1). The user sets a real name in their profile. Fails on a duplicate email.
pub async fn create_passwordless_user(db: &PgPool, email: &str) -> Result<User, sqlx::Error> {
    let name = GUEST_FALLBACK_NAME;
    let sql = format!("INSERT INTO users (email, name) VALUES ($1, $2) RETURNING {USER_COLS}");
    sqlx::query_as::<_, User>(&sql)
        .bind(email)
        .bind(name)
        .fetch_one(db)
        .await
}

pub async fn find_credentials(db: &PgPool, email: &str) -> Result<Option<Credentials>, sqlx::Error> {
    sqlx::query_as::<_, Credentials>("SELECT id, password_hash FROM users WHERE email = $1 AND blocked = false")
        .bind(email)
        .fetch_optional(db)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_name_sql_uses_the_fallback_and_never_the_email() {
        assert!(DISPLAY_NAME_SQL.contains(&format!("'{GUEST_FALLBACK_NAME}'")));
        assert!(!DISPLAY_NAME_SQL.contains("email"));
    }

    #[test]
    fn age_on_counts_whole_years() {
        let dob = NaiveDate::from_ymd_opt(2000, 6, 15).unwrap();
        // Birthday already passed this year.
        assert_eq!(age_on(dob, NaiveDate::from_ymd_opt(2024, 8, 1).unwrap()), 24);
        // Birthday not yet reached this year (one year less).
        assert_eq!(age_on(dob, NaiveDate::from_ymd_opt(2024, 3, 1).unwrap()), 23);
        // Exactly on the birthday counts.
        assert_eq!(age_on(dob, NaiveDate::from_ymd_opt(2024, 6, 15).unwrap()), 24);
    }
}

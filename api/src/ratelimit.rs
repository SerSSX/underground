//! Fixed-window rate limiting (TASK-113 / BR-017) — a small DB-backed guard against
//! brute-force and abuse on sensitive endpoints (login, passwordless verify, claim).
//!
//! `hit` counts attempts for a bucket within the window and rejects with 429 once the cap
//! is reached, otherwise records the attempt. Container-first and deterministically testable;
//! no in-memory state, so it works across API replicas.

use sqlx::PgPool;

use crate::error::AppError;

/// Record an attempt for `bucket`; error with [`AppError::RateLimited`] (429) if the number
/// of attempts in the last `window_secs` already reached `max`.
pub async fn hit(db: &PgPool, bucket: &str, max: i64, window_secs: i32) -> Result<(), AppError> {
    let recent: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM rate_events WHERE bucket = $1 AND created_at > now() - ($2 * interval '1 second')",
    )
    .bind(bucket)
    .bind(window_secs)
    .fetch_one(db)
    .await?;
    if recent >= max {
        return Err(AppError::RateLimited);
    }
    sqlx::query("INSERT INTO rate_events (bucket) VALUES ($1)")
        .bind(bucket)
        .execute(db)
        .await?;
    Ok(())
}

/// Drop attempt rows older than `older_than_secs`. Nothing else deletes from `rate_events`,
/// and every unauthenticated hit on a limited route is a row — without a sweep the table is
/// an unbounded, attacker-fed write path.
pub async fn sweep(db: &PgPool, older_than_secs: i32) -> Result<u64, sqlx::Error> {
    let r = sqlx::query("DELETE FROM rate_events WHERE created_at < now() - ($1 * interval '1 second')")
        .bind(older_than_secs)
        .execute(db)
        .await?;
    Ok(r.rows_affected())
}

//! Fixed-window rate limiting (TASK-113 / BR-017) — a small DB-backed guard against
//! brute-force and abuse on sensitive endpoints (login, passwordless verify, claim,
//! reserve, scan, transfer/gift).
//!
//! `hit` counts attempts for a bucket within the window and rejects with 429 once the cap
//! is reached, otherwise records the attempt. Container-first and deterministically testable;
//! no in-memory state, so it works across API replicas.
//!
//! The count and the insert run in one transaction under a per-bucket advisory lock
//! (TASK-302 P5): a parallel burst for one bucket is serialised, so N concurrent attempts
//! against a limit of `max` let at most `max` through.

use sqlx::PgPool;

use crate::error::AppError;

/// Namespace for the advisory lock key (first half of the two-int key), so a bucket hash can
/// never collide with an advisory lock taken by another feature.
const LOCK_NAMESPACE: i32 = 0x5254_4c4d; // "RTLM"

/// The longest window any caller uses (reserve: 1 hour). Rows older than this can never be
/// counted again and are pruned (TASK-302 P13).
pub const MAX_WINDOW_SECS: i32 = 3600;

/// Read a positive limit from the environment, falling back to `default` (D8: configurable).
pub fn env_limit(name: &str, default: i64) -> i64 {
    std::env::var(name)
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(default)
}

/// Record an attempt for `bucket`; error with [`AppError::RateLimited`] (429) if the number
/// of attempts in the last `window_secs` already reached `max`. A rejected attempt records
/// nothing.
pub async fn hit(db: &PgPool, bucket: &str, max: i64, window_secs: i32) -> Result<(), AppError> {
    debug_assert!(window_secs <= MAX_WINDOW_SECS, "raise MAX_WINDOW_SECS first");
    let mut tx = db.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1, hashtext($2))")
        .bind(LOCK_NAMESPACE)
        .bind(bucket)
        .execute(&mut *tx)
        .await?;
    let recent: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM rate_events WHERE bucket = $1 AND created_at > now() - ($2 * interval '1 second')",
    )
    .bind(bucket)
    .bind(window_secs)
    .fetch_one(&mut *tx)
    .await?;
    if recent >= max {
        // Dropping the transaction rolls back and releases the lock.
        return Err(AppError::RateLimited);
    }
    sqlx::query("INSERT INTO rate_events (bucket) VALUES ($1)")
        .bind(bucket)
        .execute(&mut *tx)
        .await?;
    // Opportunistic pruning (P13): this bucket's dead rows every time (indexed, under the lock
    // already held), and the whole table now and then.
    sqlx::query("DELETE FROM rate_events WHERE bucket = $1 AND created_at <= now() - ($2 * interval '1 second')")
        .bind(bucket)
        .bind(MAX_WINDOW_SECS)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    if uuid::Uuid::new_v4().as_bytes()[0] < 4 {
        // ~1 in 64 calls; failure here is harmless and never fails the request.
        let _ = prune(db).await;
    }
    Ok(())
}

/// Delete every rate event older than the longest window. Returns the number of rows deleted.
pub async fn prune(db: &PgPool) -> Result<u64, sqlx::Error> {
    let done = sqlx::query("DELETE FROM rate_events WHERE created_at <= now() - ($1 * interval '1 second')")
        .bind(MAX_WINDOW_SECS)
        .execute(db)
        .await?;
    Ok(done.rows_affected())
}

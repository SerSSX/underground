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

use std::net::{IpAddr, SocketAddr};

use async_trait::async_trait;
use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::request::Parts;
use sqlx::PgPool;

use crate::error::AppError;

/// The caller's IP for per-client buckets (TASK-302 fix batch 1, item 3).
///
/// The rightmost `X-Forwarded-For` entry is the address the reverse proxy (Caddy) itself saw and
/// appended, so a client cannot choose it; entries to its left are client-supplied and ignored.
/// Without the header (no proxy) the socket peer is used (`main` serves with connect info). If
/// neither is available (in-process tests) the key is `"unknown"`, one shared bucket.
pub struct ClientIp(pub String);

impl ClientIp {
    pub fn from_parts(parts: &Parts) -> Self {
        let forwarded = parts
            .headers
            .get_all("x-forwarded-for")
            .iter()
            .filter_map(|v| v.to_str().ok())
            .flat_map(|v| v.split(','))
            .last()
            .and_then(|s| s.trim().parse::<IpAddr>().ok());
        let peer = || {
            parts
                .extensions
                .get::<ConnectInfo<SocketAddr>>()
                .map(|ConnectInfo(addr)| addr.ip())
        };
        ClientIp(
            forwarded
                .or_else(peer)
                .map(|ip| ip.to_string())
                .unwrap_or_else(|| "unknown".into()),
        )
    }
}

#[async_trait]
impl<S: Send + Sync> FromRequestParts<S> for ClientIp {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(ClientIp::from_parts(parts))
    }
}

/// Per-IP sign-up bucket for `POST /api/auth/register`. Default 10 per hour. (The email-code
/// request has its own, larger bucket: it is also the password-less login.)
pub async fn hit_signup_ip(db: &PgPool, ip: &ClientIp) -> Result<(), AppError> {
    hit(
        db,
        &format!("signup-ip:{}", ip.0),
        env_limit("SIGNUP_LIMIT_PER_IP", 10),
        3600,
    )
    .await
}

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
        // ~1 in 64 calls, off the request path; failure here is harmless.
        let db = db.clone();
        tokio::spawn(async move {
            let _ = prune(&db).await;
        });
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;

    fn ip_of(req: Request<()>) -> String {
        let (parts, _) = req.into_parts();
        ClientIp::from_parts(&parts).0
    }

    #[test]
    fn client_ip_takes_the_rightmost_forwarded_entry() {
        let req = Request::builder()
            .header("x-forwarded-for", "6.6.6.6, 10.0.0.1, 203.0.113.7")
            .body(())
            .unwrap();
        assert_eq!(
            ip_of(req),
            "203.0.113.7",
            "[SEC] the client-chosen left entries are ignored"
        );
        let req = Request::builder()
            .header("x-forwarded-for", "6.6.6.6")
            .header("x-forwarded-for", "198.51.100.2")
            .body(())
            .unwrap();
        assert_eq!(ip_of(req), "198.51.100.2", "last header line, last entry");
    }

    #[test]
    fn client_ip_falls_back_to_the_socket_peer() {
        let mut req = Request::builder().body(()).unwrap();
        req.extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([192, 0, 2, 9], 4000))));
        assert_eq!(ip_of(req), "192.0.2.9");
        let mut req = Request::builder()
            .header("x-forwarded-for", "not-an-ip")
            .body(())
            .unwrap();
        req.extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([192, 0, 2, 9], 4000))));
        assert_eq!(ip_of(req), "192.0.2.9", "garbage header → peer");
        assert_eq!(ip_of(Request::builder().body(()).unwrap()), "unknown");
    }
}

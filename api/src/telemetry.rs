//! Lightweight product telemetry (TASK-017 M4). Server-side instrumentation of
//! north-star funnel events (signup, save, follow, ticket) plus a client endpoint
//! for impressions/shares, and an admin aggregation view.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::{CurrentAdmin, CurrentUser};
use crate::error::AppError;
use crate::state::AppState;

/// Anonymous client events stored per UTC day, overridable by env (TASK-305 round 3). Over it a
/// call answers as before and stores nothing.
fn daily_anon_cap() -> i64 {
    crate::ratelimit::env_limit("TELEMETRY_DAILY_ANON", 20_000)
}

/// All client events stored per UTC day, signed-in included, overridable by env (TASK-305 round
/// 3, N1): accounts are free, so only a global cap bounds the table. 40 000 a day at most ~2.3 KB
/// each, kept 30 days, is about 2.8 GB at worst.
fn daily_max() -> i64 {
    crate::ratelimit::env_limit("TELEMETRY_DAILY_MAX", 40_000)
}

/// Take one of today's (UTC) slots: true when the event may be stored. One conditional upsert on
/// the day's counter row (TASK-305 N3): it counts no events, and two calls cannot both take the
/// last slot. An anonymous call also needs room under the anonymous sub-cap.
async fn reserve_slot(db: &PgPool, anonymous: bool) -> Result<bool, sqlx::Error> {
    let taken: Option<i32> = sqlx::query_scalar(
        "INSERT INTO telemetry_daily AS d (day, total, anon) \
         VALUES ((now() AT TIME ZONE 'UTC')::date, 1, $1) \
         ON CONFLICT (day) DO UPDATE SET total = d.total + 1, anon = d.anon + EXCLUDED.anon \
         WHERE d.total < $2 AND (EXCLUDED.anon = 0 OR d.anon < $3) \
         RETURNING d.total",
    )
    .bind(i32::from(anonymous))
    .bind(daily_max().min(i64::from(i32::MAX)) as i32)
    .bind(daily_anon_cap().min(i64::from(i32::MAX)) as i32)
    .fetch_optional(db)
    .await?;
    Ok(taken.is_some())
}

/// Days an event is kept, overridable by env (TASK-305 round 3).
pub fn retention_days() -> i64 {
    crate::ratelimit::env_limit("TELEMETRY_RETENTION_DAYS", 30)
}

/// Rows one prune statement deletes; [`prune`] repeats it until done or out of time.
const PRUNE_BATCH: i64 = 5_000;

/// Delete events older than `days`, in batches, for at most `budget` (TASK-305 round 3).
/// Returns the number of rows deleted. Uses `telemetry_events_created_idx`.
pub async fn prune(db: &PgPool, days: i64, budget: std::time::Duration) -> Result<u64, sqlx::Error> {
    let started = std::time::Instant::now();
    let mut total = 0;
    loop {
        let done = sqlx::query(
            "DELETE FROM telemetry_events WHERE id IN ( \
                SELECT id FROM telemetry_events WHERE created_at < now() - ($1 * interval '1 day') LIMIT $2)",
        )
        .bind(days as i32)
        .bind(PRUNE_BATCH)
        .execute(db)
        .await?
        .rows_affected();
        total += done;
        if done < PRUNE_BATCH as u64 || started.elapsed() >= budget {
            // The daily counters of the same days (N3).
            sqlx::query("DELETE FROM telemetry_daily WHERE day < (now() AT TIME ZONE 'UTC')::date - $1")
                .bind(days as i32)
                .execute(db)
                .await?;
            return Ok(total);
        }
    }
}

/// Best-effort event log. Deliberately infallible from the caller's side: a
/// telemetry write must never break the action it measures.
pub async fn log(db: &PgPool, user_id: Option<Uuid>, name: &str, props: Value) {
    if let Err(e) = sqlx::query("INSERT INTO telemetry_events (user_id, name, props) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(name)
        .bind(props)
        .execute(db)
        .await
    {
        tracing::warn!(error = %e, name, "telemetry event dropped");
    }
}

#[derive(Deserialize)]
pub struct LogEventReq {
    pub name: String,
    #[serde(default)]
    pub props: Value,
}

/// Largest request body `POST /api/telemetry` reads (TASK-305 R3; set on the route).
pub const BODY_LIMIT: usize = 16 * 1024;

/// Largest `props`, serialized, that is stored (TASK-305 R3).
pub const PROPS_MAX: usize = 2 * 1024;

/// Client events per IP per minute (TASK-305 R3), overridable by env.
fn limit_per_ip() -> i64 {
    crate::ratelimit::env_limit("TELEMETRY_LIMIT_PER_IP", 120)
}

/// `POST /api/telemetry` — client-side events (discover impressions, shares,
/// onboarding completion). Auth is optional; anonymous events carry a null user.
///
/// Anyone can call it, so (TASK-305 R3) the body is capped at [`BODY_LIMIT`] (413), `props` at
/// [`PROPS_MAX`] serialized (422), and each client IP at 120 calls a minute (429). Over the daily
/// caps (round 3) the answer is the same 204 and nothing is stored.
pub async fn log_event(
    user: Option<CurrentUser>,
    State(st): State<AppState>,
    ip: crate::ratelimit::ClientIp,
    Json(req): Json<LogEventReq>,
) -> Result<StatusCode, AppError> {
    crate::ratelimit::hit(&st.db, &format!("telemetry-ip:{}", ip.0), limit_per_ip(), 60).await?;
    let name = req.name.trim();
    if name.is_empty() || name.len() > 64 {
        return Err(AppError::Validation("name is required (max 64 chars)".into()));
    }
    if req.props.to_string().len() > PROPS_MAX {
        return Err(AppError::Validation(format!("props: не більше {PROPS_MAX} байтів")));
    }
    let user_id = user.map(|u| u.id);
    if !reserve_slot(&st.db, user_id.is_none()).await? {
        // Over today's cap (all events, or anonymous ones): the same answer, nothing stored.
        return Ok(StatusCode::NO_CONTENT);
    }
    log(&st.db, user_id, name, req.props).await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, sqlx::FromRow)]
pub struct TelemetryStat {
    pub name: String,
    pub count: i64,
    pub last_at: Option<DateTime<Utc>>,
}

/// `GET /api/admin/telemetry` — per-event counts over the last 7 days (admin only).
pub async fn admin_telemetry(
    _admin: CurrentAdmin,
    State(st): State<AppState>,
) -> Result<Json<Vec<TelemetryStat>>, AppError> {
    let stats: Vec<TelemetryStat> = sqlx::query_as(
        "SELECT name, count(*) AS count, max(created_at) AS last_at \
         FROM telemetry_events WHERE created_at >= now() - interval '7 days' \
         GROUP BY name ORDER BY count DESC",
    )
    .fetch_all(&st.db)
    .await?;
    Ok(Json(stats))
}

#[derive(Serialize)]
pub struct FunnelStep {
    pub name: String,
    pub count: i64,
}

/// North-star funnel + viral K-factor (TASK-117 / UC-015).
#[derive(Serialize)]
pub struct Funnel {
    pub window_days: i64,
    pub steps: Vec<FunnelStep>,
    pub gift_created: i64,
    pub ticket_claimed: i64,
    /// Claims per gift over the window — a proxy for viral efficiency (share → claim).
    pub k_factor: f64,
}

/// Ordered funnel step event names (top → bottom).
const FUNNEL_STEPS: [&str; 5] = ["event_view", "signup", "ticket", "gift_created", "ticket_claimed"];

/// `GET /api/admin/analytics/funnel` — conversion funnel + K-factor over the last 7 days (admin).
pub async fn analytics_funnel(_admin: CurrentAdmin, State(st): State<AppState>) -> Result<Json<Funnel>, AppError> {
    let names: Vec<String> = FUNNEL_STEPS.iter().map(|s| s.to_string()).collect();
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT name, count(*) FROM telemetry_events \
         WHERE created_at >= now() - interval '7 days' AND name = ANY($1) GROUP BY name",
    )
    .bind(&names)
    .fetch_all(&st.db)
    .await?;
    let count_of = |n: &str| rows.iter().find(|(name, _)| name == n).map_or(0, |(_, c)| *c);
    let steps = FUNNEL_STEPS
        .iter()
        .map(|n| FunnelStep {
            name: (*n).to_string(),
            count: count_of(n),
        })
        .collect();
    let gift = count_of("gift_created");
    let claimed = count_of("ticket_claimed");
    let k_factor = if gift > 0 {
        ((claimed as f64 / gift as f64) * 1000.0).round() / 1000.0
    } else {
        0.0
    };
    Ok(Json(Funnel {
        window_days: 7,
        steps,
        gift_created: gift,
        ticket_claimed: claimed,
        k_factor,
    }))
}

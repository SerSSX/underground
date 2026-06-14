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

/// `POST /api/telemetry` — client-side events (discover impressions, shares,
/// onboarding completion). Auth is optional; anonymous events carry a null user.
pub async fn log_event(
    user: Option<CurrentUser>,
    State(st): State<AppState>,
    Json(req): Json<LogEventReq>,
) -> Result<StatusCode, AppError> {
    let name = req.name.trim();
    if name.is_empty() || name.len() > 64 {
        return Err(AppError::Validation("name is required (max 64 chars)".into()));
    }
    log(&st.db, user.map(|u| u.id), name, req.props).await;
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

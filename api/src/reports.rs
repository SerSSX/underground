//! Abuse / content reporting (P1 #76). Any authenticated user can flag an event, user,
//! post, or review; admins triage the queue and take moderation action separately.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::{CurrentAdmin, CurrentUser};
use crate::error::AppError;
use crate::state::AppState;

const TARGET_TYPES: [&str; 4] = ["event", "user", "post", "review"];
const STATUSES: [&str; 3] = ["open", "reviewed", "resolved"];

#[derive(Deserialize)]
pub struct CreateReportReq {
    pub target_type: String,
    pub target_id: Uuid,
    pub reason: String,
    pub details: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct Report {
    pub id: Uuid,
    pub reporter_id: Uuid,
    pub reporter_email: Option<String>,
    pub target_type: String,
    pub target_id: Uuid,
    pub reason: String,
    pub details: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

/// `POST /api/reports` — any authenticated user flags abuse/content.
pub async fn create_report(
    user: CurrentUser,
    State(st): State<AppState>,
    Json(req): Json<CreateReportReq>,
) -> Result<StatusCode, AppError> {
    if !TARGET_TYPES.contains(&req.target_type.as_str()) {
        return Err(AppError::Validation(
            "target_type must be event|user|post|review".into(),
        ));
    }
    let reason = req.reason.trim();
    if reason.is_empty() || reason.chars().count() > 200 {
        return Err(AppError::Validation("reason is required (max 200 chars)".into()));
    }
    let details = req.details.as_deref().map(str::trim).filter(|s| !s.is_empty());
    if details.is_some_and(|d| d.chars().count() > 1000) {
        return Err(AppError::Validation("details too long (max 1000 chars)".into()));
    }
    // The target must actually exist and match its declared type (no reporting ghost ids).
    let target_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS( \
            SELECT 1 FROM events WHERE id = $1 AND $2 = 'event' \
            UNION ALL SELECT 1 FROM users WHERE id = $1 AND $2 = 'user' \
            UNION ALL SELECT 1 FROM posts WHERE id = $1 AND $2 = 'post' \
            UNION ALL SELECT 1 FROM reviews WHERE id = $1 AND $2 = 'review')",
    )
    .bind(req.target_id)
    .bind(&req.target_type)
    .fetch_one(&st.db)
    .await?;
    if !target_exists {
        return Err(AppError::NotFound);
    }
    sqlx::query(
        "INSERT INTO reports (reporter_id, target_type, target_id, reason, details) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(user.id)
    .bind(&req.target_type)
    .bind(req.target_id)
    .bind(reason)
    .bind(details)
    .execute(&st.db)
    .await?;
    Ok(StatusCode::CREATED)
}

#[derive(Deserialize)]
pub struct ReportQuery {
    pub status: Option<String>,
}

/// `GET /api/admin/reports?status=` — moderation queue (newest first).
pub async fn list_reports(
    _admin: CurrentAdmin,
    State(st): State<AppState>,
    Query(q): Query<ReportQuery>,
) -> Result<Json<Vec<Report>>, AppError> {
    let reports = sqlx::query_as::<_, Report>(
        "SELECT r.id, r.reporter_id, u.email AS reporter_email, r.target_type, r.target_id, \
            r.reason, r.details, r.status, r.created_at \
         FROM reports r LEFT JOIN users u ON u.id = r.reporter_id \
         WHERE ($1::text IS NULL OR r.status = $1) \
         ORDER BY r.created_at DESC LIMIT 200",
    )
    .bind(q.status.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .fetch_all(&st.db)
    .await?;
    Ok(Json(reports))
}

#[derive(Deserialize)]
pub struct ResolveReq {
    pub status: String,
    /// Optional moderator decision note, captured into the audit trail (ADM-6).
    #[serde(default)]
    pub note: Option<String>,
}

/// `PATCH /api/admin/reports/:id` — advance a report's moderation status.
pub async fn resolve_report(
    admin: CurrentAdmin,
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<ResolveReq>,
) -> Result<StatusCode, AppError> {
    if !STATUSES.contains(&req.status.as_str()) {
        return Err(AppError::Validation("status must be open|reviewed|resolved".into()));
    }
    let mut tx = st.db.begin().await?;
    let res = sqlx::query("UPDATE reports SET status = $2 WHERE id = $1")
        .bind(id)
        .bind(&req.status)
        .execute(&mut *tx)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    crate::audit::log(
        &mut *tx,
        admin.id,
        "report_resolve",
        "report",
        Some(id),
        serde_json::json!({ "status": req.status, "note": req.note }),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

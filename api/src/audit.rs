//! Moderation audit-trail (structural bet): an append-only log of privileged admin actions,
//! so Trust & Safety has an accountable record of who did what, when, and to whom.
//!
//! [`log`] is called from each admin handler *after* its mutation succeeds. It deliberately
//! returns its error to the caller (the trail is part of the action's contract, not best-effort
//! telemetry) — a failed insert fails the request rather than silently losing the record.

use axum::extract::State;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::auth::CurrentAdmin;
use crate::error::AppError;
use crate::state::AppState;

/// Record one admin action.
///
/// Generic over the executor so callers can pass the SAME transaction as their mutation
/// (`&mut *tx`) — the audit row and the mutation then commit or roll back together, keeping the
/// trail authoritative: there is no window where a change persists without its log entry.
pub async fn log<'e, E>(
    executor: E,
    admin_id: Uuid,
    action: &str,
    target_type: &str,
    target_id: Option<Uuid>,
    detail: Value,
) -> Result<(), sqlx::Error>
where
    E: sqlx::PgExecutor<'e>,
{
    sqlx::query(
        "INSERT INTO admin_actions (admin_id, action, target_type, target_id, detail) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(admin_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(detail)
    .execute(executor)
    .await?;
    Ok(())
}

/// One row of the audit trail, with the acting admin's current name/email resolved.
#[derive(Serialize, sqlx::FromRow)]
pub struct AdminAction {
    pub id: Uuid,
    pub admin_name: Option<String>,
    pub admin_email: Option<String>,
    pub action: String,
    pub target_type: String,
    pub target_id: Option<Uuid>,
    pub detail: Value,
    pub created_at: DateTime<Utc>,
}

/// `GET /api/admin/actions` — the most recent admin actions, newest first.
pub async fn list_actions(
    _admin: CurrentAdmin,
    State(st): State<AppState>,
) -> Result<Json<Vec<AdminAction>>, AppError> {
    let rows = sqlx::query_as::<_, AdminAction>(
        "SELECT a.id, u.name AS admin_name, u.email AS admin_email, \
                a.action, a.target_type, a.target_id, a.detail, a.created_at \
         FROM admin_actions a LEFT JOIN users u ON u.id = a.admin_id \
         ORDER BY a.created_at DESC LIMIT 200",
    )
    .fetch_all(&st.db)
    .await?;
    Ok(Json(rows))
}

//! Delegated door staff (TASK-118, UC-014, BR-012, WF-DoorScan).
//!
//! An organizer invites a controller by single-use token; accepting grants a scoped
//! `event_staff` row (scan-only, one event). Scanning is event-scoped so a ticket for a
//! different event yields a clear `wrong_event` verdict — the controller never gains access
//! to money, editing, or other events.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::passwords::token_hash;
use crate::state::AppState;

/// Staff-invite lifetime (hours), overridable via env; default 72h.
fn invite_ttl_hours() -> i64 {
    std::env::var("STAFF_INVITE_TTL_HOURS")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|h| *h > 0)
        .unwrap_or(72)
}

/// True if `user_id` may scan tickets for `event_id`: the owner, or scan-permissioned staff.
async fn can_scan(db: &sqlx::PgPool, event_id: Uuid, user_id: Uuid) -> Result<bool, AppError> {
    let owner: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM events WHERE id = $1 AND organizer_id = $2)")
        .bind(event_id)
        .bind(user_id)
        .fetch_one(db)
        .await?;
    if owner {
        return Ok(true);
    }
    let staff: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM event_staff WHERE event_id = $1 AND user_id = $2 AND permission = 'scan')",
    )
    .bind(event_id)
    .bind(user_id)
    .fetch_one(db)
    .await?;
    Ok(staff)
}

#[derive(Serialize)]
pub struct InviteResp {
    pub invite_token: String,
    pub expires_at: DateTime<Utc>,
}

/// `POST /api/events/:id/staff/invite` — owner creates a single-use controller invite.
pub async fn invite_staff(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(event_id): Path<Uuid>,
) -> Result<(StatusCode, Json<InviteResp>), AppError> {
    let organizer: Option<Option<Uuid>> = sqlx::query_scalar("SELECT organizer_id FROM events WHERE id = $1")
        .bind(event_id)
        .fetch_optional(&st.db)
        .await?;
    match organizer {
        None => return Err(AppError::NotFound),
        Some(o) if o != Some(user.id) => return Err(AppError::Forbidden),
        Some(_) => {}
    }
    let raw = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let expires_at = Utc::now() + Duration::hours(invite_ttl_hours());
    sqlx::query("INSERT INTO staff_invites (event_id, invited_by, token_hash, expires_at) VALUES ($1, $2, $3, $4)")
        .bind(event_id)
        .bind(user.id)
        .bind(token_hash(&raw))
        .bind(expires_at)
        .execute(&st.db)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(InviteResp {
            invite_token: raw,
            expires_at,
        }),
    ))
}

#[derive(Deserialize)]
pub struct AcceptReq {
    pub token: String,
}

/// `POST /api/staff/accept` — a user redeems a controller invite, gaining scan rights.
pub async fn accept_invite(
    user: CurrentUser,
    State(st): State<AppState>,
    Json(req): Json<AcceptReq>,
) -> Result<StatusCode, AppError> {
    let mut tx = st.db.begin().await?;
    let inv: Option<(Uuid, Uuid, DateTime<Utc>)> = sqlx::query_as(
        "SELECT id, event_id, expires_at FROM staff_invites WHERE token_hash = $1 AND status = 'pending' FOR UPDATE",
    )
    .bind(token_hash(req.token.trim()))
    .fetch_optional(&mut *tx)
    .await?;
    let (invite_id, event_id, expires_at) = inv.ok_or(AppError::NotFound)?;
    if expires_at < Utc::now() {
        sqlx::query("UPDATE staff_invites SET status = 'expired' WHERE id = $1")
            .bind(invite_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        return Err(AppError::Conflict("термін дії запрошення минув".into()));
    }
    sqlx::query(
        "INSERT INTO event_staff (event_id, user_id, permission, invited_by) \
         SELECT $1, $2, 'scan', invited_by FROM staff_invites WHERE id = $3 \
         ON CONFLICT (event_id, user_id) DO NOTHING",
    )
    .bind(event_id)
    .bind(user.id)
    .bind(invite_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE staff_invites SET status = 'accepted', accepted_by = $2 WHERE id = $1")
        .bind(invite_id)
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct ScanReq {
    pub qr_token: String,
}

#[derive(Serialize)]
pub struct ScanResp {
    pub valid: bool,
    pub reason: String,
    pub event_title: String,
}

/// `POST /api/events/:id/scan` — owner or delegated staff validates a QR at the door.
/// Verdicts: valid | already_used | wrong_event | returned | inactive | not_found.
pub async fn scan(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(event_id): Path<Uuid>,
    Json(req): Json<ScanReq>,
) -> Result<Json<ScanResp>, AppError> {
    if !can_scan(&st.db, event_id, user.id).await? {
        return Err(AppError::Forbidden);
    }
    let mut tx = st.db.begin().await?;
    // Only a signed short-TTL QR is accepted at the door (BR-023); an invalid or expired token
    // resolves to nothing → the "not_found" verdict. The raw qr_token is no longer accepted.
    let row: Option<(Uuid, String, Uuid, String)> = match crate::qr::verify_qr(req.qr_token.trim(), &st.jwt_secret) {
        Some(c) => {
            sqlx::query_as(
                "SELECT t.id, t.status, t.event_id, e.title FROM tickets t \
                     JOIN events e ON e.id = t.event_id WHERE t.id = $1 FOR UPDATE OF t",
            )
            .bind(c.tid)
            .fetch_optional(&mut *tx)
            .await?
        }
        None => None,
    };
    let Some((ticket_id, status, ticket_event, title)) = row else {
        return Ok(Json(ScanResp {
            valid: false,
            reason: "not_found".into(),
            event_title: String::new(),
        }));
    };
    let (valid, reason) = if ticket_event != event_id {
        (false, "wrong_event")
    } else if status == "active" {
        sqlx::query("UPDATE tickets SET status = 'used', validated_at = now() WHERE id = $1")
            .bind(ticket_id)
            .execute(&mut *tx)
            .await?;
        (true, "valid")
    } else if status == "used" {
        (false, "already_used")
    } else if status == "returned" {
        (false, "returned")
    } else {
        (false, "inactive")
    };
    tx.commit().await?;
    Ok(Json(ScanResp {
        valid,
        reason: reason.to_string(),
        event_title: title,
    }))
}

/// `GET /api/me/staff/events` — events the current user may scan at the door: their own
/// events plus any where they hold delegated `scan` permission. Cancelled events are omitted.
/// Additive (does not change existing endpoints); lets a controller reopen the scanner without
/// re-using the single-use invite link.
pub async fn my_staff_events(
    user: CurrentUser,
    State(st): State<AppState>,
) -> Result<axum::Json<Vec<crate::events::Event>>, AppError> {
    let sql = format!(
        "SELECT {cols} FROM events e WHERE e.status <> 'cancelled' AND ( \
             e.organizer_id = $1 \
             OR EXISTS (SELECT 1 FROM event_staff s \
                        WHERE s.event_id = e.id AND s.user_id = $1 AND s.permission = 'scan') \
         ) ORDER BY e.starts_at ASC NULLS LAST",
        cols = crate::events::EVENT_COLS,
    );
    let rows = sqlx::query_as::<_, crate::events::Event>(&sql)
        .bind(user.id)
        .fetch_all(&st.db)
        .await?;
    Ok(axum::Json(rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invite_ttl_honors_env_then_defaults() {
        std::env::set_var("STAFF_INVITE_TTL_HOURS", "6");
        assert_eq!(invite_ttl_hours(), 6);
        std::env::set_var("STAFF_INVITE_TTL_HOURS", "0");
        assert_eq!(invite_ttl_hours(), 72, "non-positive TTL is ignored");
        std::env::remove_var("STAFF_INVITE_TTL_HOURS");
        assert_eq!(invite_ttl_hours(), 72);
    }
}

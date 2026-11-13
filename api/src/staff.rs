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

/// Most live (pending, unexpired) staff invites one event may have at once.
pub const MAX_PENDING_INVITES: i64 = 20;

/// `POST /api/events/:id/staff/invite` — owner creates a single-use controller invite.
pub async fn invite_staff(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(event_id): Path<Uuid>,
) -> Result<(StatusCode, Json<InviteResp>), AppError> {
    let mut tx = st.db.begin().await?;
    // Row lock: concurrent invite calls for one event are serialised, so the cap below holds.
    let event: Option<(Option<Uuid>, String)> =
        sqlx::query_as("SELECT organizer_id, status FROM events WHERE id = $1 FOR UPDATE")
            .bind(event_id)
            .fetch_optional(&mut *tx)
            .await?;
    match event {
        None => return Err(AppError::NotFound),
        Some((o, _)) if o != Some(user.id) => return Err(AppError::Forbidden),
        // CT12 / I08: no door staff for an event that will not take place.
        Some((_, status)) if status == "cancelled" || status == "blocked" => {
            return Err(AppError::Conflict("подію скасовано — запрошення неможливе".into()))
        }
        Some(_) => {}
    }
    // Fix batch 1, item 7: at most MAX_PENDING_INVITES live invites per event, so a leaked or
    // scripted organiser session cannot mint an unbounded number of bearer tokens.
    let pending: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM staff_invites WHERE event_id = $1 AND status = 'pending' AND expires_at > now()",
    )
    .bind(event_id)
    .fetch_one(&mut *tx)
    .await?;
    if pending >= MAX_PENDING_INVITES {
        return Err(AppError::Conflict(format!(
            "забагато активних запрошень (до {MAX_PENDING_INVITES}); скасуйте непотрібні"
        )));
    }
    let raw = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let expires_at = Utc::now() + Duration::hours(invite_ttl_hours());
    sqlx::query("INSERT INTO staff_invites (event_id, invited_by, token_hash, expires_at) VALUES ($1, $2, $3, $4)")
        .bind(event_id)
        .bind(user.id)
        .bind(token_hash(&raw))
        .bind(expires_at)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
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

#[derive(Serialize)]
pub struct AcceptResp {
    /// The event the caller may now scan — the client opens its scanner.
    pub event_id: Uuid,
}

/// `POST /api/staff/accept` — a user redeems a controller invite, gaining scan rights.
/// 200 `{"event_id": ...}`.
pub async fn accept_invite(
    user: CurrentUser,
    State(st): State<AppState>,
    Json(req): Json<AcceptReq>,
) -> Result<Json<AcceptResp>, AppError> {
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
    // CT12 / I06: the organiser already scans their own event; accepting would only burn the
    // invite meant for someone else. Rolled back: the invite stays pending.
    let is_organizer: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM events WHERE id = $1 AND organizer_id = $2)")
            .bind(event_id)
            .bind(user.id)
            .fetch_one(&mut *tx)
            .await?;
    if is_organizer {
        return Err(AppError::Conflict(
            "організатор не може прийняти власне запрошення".into(),
        ));
    }
    // TASK-305: no door rights for a cancelled or finished event; the invite stays pending.
    let sql = format!(
        "SELECT e.status IN ('cancelled', 'blocked') OR {} FROM events e WHERE e.id = $1",
        crate::tickets::EVENT_OVER_SQL
    );
    let gone: Option<bool> = sqlx::query_scalar(&sql).bind(event_id).fetch_one(&mut *tx).await?;
    if gone.unwrap_or(false) {
        return Err(AppError::Conflict("подію скасовано або вона вже минула".into()));
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
    Ok(Json(AcceptResp { event_id }))
}

#[derive(Deserialize)]
pub struct ScanReq {
    pub qr_token: String,
}

/// Door-scan verdict. `holder_*` fields describe the ticket holder at THIS event and are set
/// only when the scanned ticket belongs to this event (never for `not_found`, `wrong_event`,
/// `event_cancelled`). The holder's email is never part of it (CT15).
#[derive(Serialize)]
pub struct ScanResp {
    pub valid: bool,
    pub reason: String,
    /// This event's title; empty for `not_found` and `wrong_event` (P10: no other event leaks).
    pub event_title: String,
    /// Display name with the CT8 fallback ("Гість").
    pub holder_name: Option<String>,
    /// The holder's live places at this event (active + pending_claim + used).
    pub places: Option<i64>,
    /// How many of them are already checked in (used), this scan included.
    pub checked_in: Option<i64>,
}

impl ScanResp {
    fn bare(valid: bool, reason: &str, event_title: String) -> Self {
        ScanResp {
            valid,
            reason: reason.to_string(),
            event_title,
            holder_name: None,
            places: None,
            checked_in: None,
        }
    }
}

fn scan_limit_per_event() -> i64 {
    crate::ratelimit::env_limit("SCAN_LIMIT_PER_EVENT", 120)
}

/// `POST /api/events/:id/scan` — owner or delegated staff validates a QR at the door.
/// Verdicts: valid | already_used | wrong_event | returned | inactive | not_found | event_cancelled.
pub async fn scan(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(event_id): Path<Uuid>,
    Json(req): Json<ScanReq>,
) -> Result<Json<ScanResp>, AppError> {
    // CT6 / D8: 120 scans per scanner per event per minute.
    crate::ratelimit::hit(
        &st.db,
        &format!("scan:{}:{}", user.id, event_id),
        scan_limit_per_event(),
        60,
    )
    .await?;
    if !can_scan(&st.db, event_id, user.id).await? {
        return Err(AppError::Forbidden);
    }
    let mut tx = st.db.begin().await?;
    // Only a signed short-TTL QR is accepted at the door (BR-023); an invalid or expired token
    // resolves to nothing → the "not_found" verdict. The raw qr_token is no longer accepted.
    type Row = (Uuid, String, Uuid, Uuid, String, String);
    let row: Option<Row> = match crate::qr::verify_qr(req.qr_token.trim(), &st.jwt_secret) {
        Some(c) => sqlx::query_as::<_, Row>(
            "SELECT t.id, t.status, t.event_id, t.owner_id, e.title, e.status FROM tickets t \
                 JOIN events e ON e.id = t.event_id WHERE t.id = $1 FOR UPDATE OF t",
        )
        .bind(c.tid)
        .fetch_optional(&mut *tx)
        .await?
        // CT10: the QR is bound to the owner it was minted for; after a transfer or a claimed
        // gift the previous owner's QR is dead.
        .filter(|r| r.3 == c.oid),
        None => None,
    };
    let Some((ticket_id, status, ticket_event, owner_id, title, event_status)) = row else {
        return Ok(Json(ScanResp::bare(false, "not_found", String::new())));
    };
    if ticket_event != event_id {
        // P10: nothing about the other event (title, holder) reaches this event's staff.
        return Ok(Json(ScanResp::bare(false, "wrong_event", String::new())));
    }
    if event_status == "cancelled" || event_status == "blocked" {
        // CT11 / D7: nobody is admitted to a cancelled or blocked event; nothing changes.
        return Ok(Json(ScanResp::bare(false, "event_cancelled", title)));
    }
    let (valid, reason) = if status == "active" {
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
    let sql = format!(
        "SELECT {name} AS name, \
            (SELECT count(*) FROM tickets t WHERE t.event_id = $1 AND t.owner_id = u.id \
                AND t.status IN ('active', 'pending_claim', 'used')) AS places, \
            (SELECT count(*) FROM tickets t WHERE t.event_id = $1 AND t.owner_id = u.id \
                AND t.status = 'used') AS checked_in \
         FROM users u WHERE u.id = $2",
        name = crate::users::DISPLAY_NAME_SQL,
    );
    let (holder_name, places, checked_in): (String, i64, i64) = sqlx::query_as(&sql)
        .bind(event_id)
        .bind(owner_id)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(ScanResp {
        valid,
        reason: reason.to_string(),
        event_title: title,
        holder_name: Some(holder_name),
        places: Some(places),
        checked_in: Some(checked_in),
    }))
}

// ── Staff management (TASK-302 CT12) ───────────────────────────────────────────────

#[derive(Serialize, sqlx::FromRow)]
pub struct StaffMember {
    /// Needed by the client to call `DELETE /api/events/:id/staff/:user_id`.
    pub user_id: Uuid,
    /// Display name with the CT8 fallback; never the email.
    pub name: String,
    pub added_at: DateTime<Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct StaffInvite {
    /// Needed by the client to call `DELETE /api/events/:id/staff/invites/:invite_id`.
    pub id: Uuid,
    /// `pending` or `expired` (an outstanding invite past its expiry).
    pub status: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct StaffList {
    pub staff: Vec<StaffMember>,
    /// Invites not yet accepted or revoked. No token or token hash, ever.
    pub invites: Vec<StaffInvite>,
}

/// `GET /api/events/:id/staff` — organiser only: who can scan, and the invites still open.
pub async fn list_staff(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(event_id): Path<Uuid>,
) -> Result<Json<StaffList>, AppError> {
    crate::events::require_owner(&st.db, event_id, user.id).await?;
    let sql = format!(
        "SELECT s.user_id, {name} AS name, s.created_at AS added_at \
         FROM event_staff s JOIN users u ON u.id = s.user_id \
         WHERE s.event_id = $1 ORDER BY s.created_at",
        name = crate::users::DISPLAY_NAME_SQL,
    );
    let staff: Vec<StaffMember> = sqlx::query_as(&sql).bind(event_id).fetch_all(&st.db).await?;
    let invites: Vec<StaffInvite> = sqlx::query_as(
        "SELECT id, CASE WHEN expires_at < now() THEN 'expired' ELSE 'pending' END AS status, \
            expires_at, created_at \
         FROM staff_invites WHERE event_id = $1 AND status = 'pending' \
         ORDER BY created_at DESC LIMIT 200",
    )
    .bind(event_id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(StaffList { staff, invites }))
}

/// `DELETE /api/events/:id/staff/invites/:invite_id` — organiser revokes an invite that has not
/// been accepted. 204; 404 if there is no such pending invite on this event. A concurrent
/// accept holds the invite row lock, so exactly one of the two wins.
pub async fn revoke_invite(
    user: CurrentUser,
    State(st): State<AppState>,
    Path((event_id, invite_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    crate::events::require_owner(&st.db, event_id, user.id).await?;
    let done = sqlx::query(
        "UPDATE staff_invites SET status = 'revoked' WHERE id = $1 AND event_id = $2 AND status = 'pending'",
    )
    .bind(invite_id)
    .bind(event_id)
    .execute(&st.db)
    .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /api/events/:id/staff/:user_id` — organiser removes a staff member's scan rights.
/// 204; 404 if that user is not staff of this event.
pub async fn remove_staff(
    user: CurrentUser,
    State(st): State<AppState>,
    Path((event_id, staff_user)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    crate::events::require_owner(&st.db, event_id, user.id).await?;
    let done = sqlx::query("DELETE FROM event_staff WHERE event_id = $1 AND user_id = $2")
        .bind(event_id)
        .bind(staff_user)
        .execute(&st.db)
        .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
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

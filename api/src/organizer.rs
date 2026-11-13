//! Organizer web-cabinet APIs: sales dashboard + per-event ticket list (TASK-014, Фаза 3).

use axum::extract::{Path, State};
use axum::Json;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::state::AppState;

#[derive(Serialize, sqlx::FromRow)]
pub struct OrganizerStats {
    pub events_count: i64,
    pub tickets_sold: i64,
    pub tickets_validated: i64,
    pub revenue: Decimal,
    /// Funds in escrow (held or clearing) — event not yet fully cleared.
    pub escrow_held: Decimal,
    /// Cleared funds awaiting an admin payout (TASK-017 M6).
    pub escrow_cleared: Decimal,
    /// Funds already settled to the organizer.
    pub paid_out: Decimal,
}

/// `GET /api/organizer/stats` — the caller's sales dashboard numbers.
pub async fn stats(user: CurrentUser, State(st): State<AppState>) -> Result<Json<OrganizerStats>, AppError> {
    let stats: OrganizerStats = sqlx::query_as(
        "SELECT \
            (SELECT count(*) FROM events WHERE organizer_id = $1) AS events_count, \
            (SELECT count(*) FROM tickets t JOIN events e ON e.id = t.event_id \
                WHERE e.organizer_id = $1 AND t.status IN ('active','used')) AS tickets_sold, \
            (SELECT count(*) FROM tickets t JOIN events e ON e.id = t.event_id \
                WHERE e.organizer_id = $1 AND t.status = 'used') AS tickets_validated, \
            COALESCE((SELECT sum(o.amount) FROM orders o JOIN events e ON e.id = o.event_id \
                WHERE e.organizer_id = $1 AND o.status = 'paid'), 0) AS revenue, \
            COALESCE((SELECT sum(o.amount) FROM orders o JOIN events e ON e.id = o.event_id \
                WHERE e.organizer_id = $1 AND o.escrow_status IN ('held','clearing')), 0) AS escrow_held, \
            COALESCE((SELECT sum(o.amount) FROM orders o JOIN events e ON e.id = o.event_id \
                WHERE e.organizer_id = $1 AND o.escrow_status = 'cleared'), 0) AS escrow_cleared, \
            COALESCE((SELECT sum(o.amount) FROM orders o JOIN events e ON e.id = o.event_id \
                WHERE e.organizer_id = $1 AND o.escrow_status = 'paid_out'), 0) AS paid_out",
    )
    .bind(user.id)
    .fetch_one(&st.db)
    .await?;
    Ok(Json(stats))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct DailySales {
    pub day: NaiveDate,
    pub tickets: i64,
    pub revenue: Decimal,
}

/// `GET /api/organizer/analytics` — paid sales per day over the last 30 days (TASK-017 M8).
pub async fn analytics(user: CurrentUser, State(st): State<AppState>) -> Result<Json<Vec<DailySales>>, AppError> {
    let rows: Vec<DailySales> = sqlx::query_as(
        "SELECT (o.created_at AT TIME ZONE 'UTC')::date AS day, \
            count(*) AS tickets, \
            COALESCE(sum(o.amount), 0) AS revenue \
         FROM orders o JOIN events e ON e.id = o.event_id \
         WHERE e.organizer_id = $1 AND o.status = 'paid' \
           AND o.created_at >= now() - interval '30 days' \
         GROUP BY 1 ORDER BY 1",
    )
    .bind(user.id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(rows))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct MyPayout {
    pub id: Uuid,
    pub amount: Decimal,
    pub currency: String,
    pub order_count: i32,
    pub status: String,
    pub failure_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// `GET /api/organizer/payouts` — the caller's OWN settlement history (R5). Scoped to
/// `organizer_id = caller`, so an organizer can finally see whether/when they were paid.
pub async fn my_payouts(user: CurrentUser, State(st): State<AppState>) -> Result<Json<Vec<MyPayout>>, AppError> {
    let rows: Vec<MyPayout> = sqlx::query_as(
        "SELECT id, amount, currency, order_count, status, failure_reason, created_at \
         FROM payouts WHERE organizer_id = $1 ORDER BY created_at DESC LIMIT 200",
    )
    .bind(user.id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(rows))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct EventTicket {
    pub id: Uuid,
    /// Display name with the CT8 fallback. The email is no longer returned (TASK-302 Change E).
    pub owner_name: String,
    pub status: String,
    pub price: Decimal,
    pub created_at: DateTime<Utc>,
    pub validated_at: Option<DateTime<Utc>>,
}

/// `GET /api/events/:id/tickets` — per-ticket attendee list, organizer-only.
pub async fn event_tickets(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(event_id): Path<Uuid>,
) -> Result<Json<Vec<EventTicket>>, AppError> {
    crate::events::require_owner(&st.db, event_id, user.id).await?;
    let sql = format!(
        "SELECT t.id, {name} AS owner_name, t.status, t.price, t.created_at, t.validated_at \
         FROM tickets t JOIN users u ON u.id = t.owner_id \
         WHERE t.event_id = $1 ORDER BY t.created_at DESC",
        name = crate::users::DISPLAY_NAME_SQL,
    );
    let tickets: Vec<EventTicket> = sqlx::query_as(&sql).bind(event_id).fetch_all(&st.db).await?;
    Ok(Json(tickets))
}

/// One attendee on the guest list: a person, not a ticket (TASK-302 CT7).
#[derive(Serialize, sqlx::FromRow)]
pub struct Guest {
    /// The attendee's account, so the organiser can message them (TASK-305, coordinator-approved
    /// 2026-11-14). Only the event's organiser gets this list.
    pub user_id: Uuid,
    /// Display name with the CT8 fallback ("Гість"); never the email.
    pub name: String,
    /// Live places: active + pending_claim + used.
    pub places: i64,
    /// Places already scanned at the door (used).
    pub checked_in: i64,
    /// When the earliest of these places was booked.
    pub booked_at: DateTime<Utc>,
}

/// `GET /api/events/:id/guests` — the organiser's guest list, one row per attendee. Organiser
/// only: staff (scan-only, D5) and everyone else get 403. The SQL never selects the email.
pub async fn event_guests(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(event_id): Path<Uuid>,
) -> Result<Json<Vec<Guest>>, AppError> {
    crate::events::require_owner(&st.db, event_id, user.id).await?;
    let sql = format!(
        "SELECT u.id AS user_id, {name} AS name, count(*) AS places, \
            count(*) FILTER (WHERE t.status = 'used') AS checked_in, \
            min(t.created_at) AS booked_at \
         FROM tickets t JOIN users u ON u.id = t.owner_id \
         WHERE t.event_id = $1 AND t.status IN ('active', 'pending_claim', 'used') \
         GROUP BY u.id \
         ORDER BY booked_at DESC",
        name = crate::users::DISPLAY_NAME_SQL,
    );
    let guests: Vec<Guest> = sqlx::query_as(&sql).bind(event_id).fetch_all(&st.db).await?;
    Ok(Json(guests))
}

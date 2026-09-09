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
    pub owner_name: String,
    pub owner_email: String,
    pub status: String,
    pub price: Decimal,
    pub created_at: DateTime<Utc>,
    pub validated_at: Option<DateTime<Utc>>,
}

/// `GET /api/events/:id/tickets` — attendee list, organizer-only.
pub async fn event_tickets(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(event_id): Path<Uuid>,
) -> Result<Json<Vec<EventTicket>>, AppError> {
    let organizer: Option<Option<Uuid>> = sqlx::query_scalar("SELECT organizer_id FROM events WHERE id = $1")
        .bind(event_id)
        .fetch_optional(&st.db)
        .await?;
    match organizer {
        None => return Err(AppError::NotFound),
        Some(o) if o == Some(user.id) => {}
        Some(_) => return Err(AppError::Forbidden),
    }
    let tickets: Vec<EventTicket> = sqlx::query_as(
        "SELECT t.id, u.name AS owner_name, u.email AS owner_email, t.status, t.price, \
            t.created_at, t.validated_at \
         FROM tickets t JOIN users u ON u.id = t.owner_id \
         WHERE t.event_id = $1 ORDER BY t.created_at DESC",
    )
    .bind(event_id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(tickets))
}

/// `GET /api/events/:id/refund-progress` — how a cancellation is draining.
///
/// Owner or platform admin only: refund progress reveals how many tickets an event sold and for
/// how much, which is the organiser's business and nobody else's.
pub async fn refund_progress(
    user: crate::auth::CurrentUser,
    State(st): State<AppState>,
    axum::extract::Path(event_id): axum::extract::Path<uuid::Uuid>,
) -> Result<Json<crate::refunds::Progress>, AppError> {
    let allowed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM events e, users u \
         WHERE e.id = $1 AND u.id = $2 AND (e.organizer_id = u.id OR u.role = 'ADMIN'))",
    )
    .bind(event_id)
    .bind(user.id)
    .fetch_one(&st.db)
    .await?;
    if !allowed {
        return Err(AppError::Forbidden);
    }
    Ok(Json(crate::refunds::event_progress(&st.db, event_id).await?))
}

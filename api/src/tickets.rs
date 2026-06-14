//! E-tickets: reserve a free ticket + list own tickets. Shared minting helpers
//! are reused by the paid checkout flow (`orders`).

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::state::AppState;

const TICKET_SELECT: &str = "SELECT t.id, t.event_id, t.qr_token, t.status, t.price, t.created_at, \
        e.title AS event_title, e.starts_at AS event_starts_at, e.city AS event_city, \
        e.venue_address AS event_venue_address, e.poster_url AS event_poster_url, \
        o.id AS order_id, o.escrow_status AS escrow_status \
     FROM tickets t JOIN events e ON e.id = t.event_id \
     LEFT JOIN orders o ON o.ticket_id = t.id AND o.buyer_id = t.owner_id";

#[derive(Serialize, sqlx::FromRow)]
pub struct Ticket {
    pub id: Uuid,
    pub event_id: Uuid,
    pub qr_token: String,
    pub status: String,
    pub price: Decimal,
    pub created_at: DateTime<Utc>,
    pub event_title: String,
    pub event_starts_at: Option<DateTime<Utc>>,
    pub event_city: Option<String>,
    pub event_venue_address: Option<String>,
    pub event_poster_url: Option<String>,
    /// The paid order this ticket came from (None for free reservations) — lets the buyer
    /// dispute from "My tickets". `escrow_status` says whether a dispute is still possible
    /// (held|clearing → yes; frozen → already disputed; cleared/refunded/paid_out → no).
    pub order_id: Option<Uuid>,
    pub escrow_status: Option<String>,
}

pub async fn fetch_ticket(db: &PgPool, id: Uuid) -> Result<Ticket, sqlx::Error> {
    let sql = format!("{TICKET_SELECT} WHERE t.id = $1");
    sqlx::query_as::<_, Ticket>(&sql).bind(id).fetch_one(db).await
}

/// Atomically decrements remaining tickets; returns None when sold out.
pub async fn decrement_tickets(conn: &mut sqlx::PgConnection, event_id: Uuid) -> Result<Option<i32>, sqlx::Error> {
    sqlx::query_scalar(
        "UPDATE events SET tickets_left = tickets_left - 1 WHERE id = $1 AND tickets_left > 0 RETURNING tickets_left",
    )
    .bind(event_id)
    .fetch_optional(conn)
    .await
}

/// Inserts an active ticket with a fresh QR token; returns its id.
pub async fn mint_ticket(
    conn: &mut sqlx::PgConnection,
    event_id: Uuid,
    owner_id: Uuid,
    price: Decimal,
) -> Result<Uuid, sqlx::Error> {
    let qr_token = format!("UG-{}", Uuid::new_v4());
    sqlx::query_scalar(
        "INSERT INTO tickets (event_id, owner_id, qr_token, price, status) \
         VALUES ($1, $2, $3, $4, 'active') RETURNING id",
    )
    .bind(event_id)
    .bind(owner_id)
    .bind(&qr_token)
    .bind(price)
    .fetch_one(conn)
    .await
}

/// `GET /api/me/tickets` — newest first.
pub async fn my_tickets(user: CurrentUser, State(st): State<AppState>) -> Result<Json<Vec<Ticket>>, AppError> {
    let sql = format!("{TICKET_SELECT} WHERE t.owner_id = $1 ORDER BY t.created_at DESC");
    let tickets = sqlx::query_as::<_, Ticket>(&sql)
        .bind(user.id)
        .fetch_all(&st.db)
        .await?;
    Ok(Json(tickets))
}

/// `POST /api/events/:id/reserve` — mints a free ticket for the caller.
pub async fn reserve(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(event_id): Path<Uuid>,
) -> Result<(StatusCode, Json<Ticket>), AppError> {
    let mut tx = st.db.begin().await?;

    let row: Option<(String, bool, bool, Option<String>)> = sqlx::query_as(
        "SELECT cost_type, sales_closed, (starts_at IS NOT NULL AND starts_at < now()) AS past, age_rating \
         FROM events WHERE id = $1 AND status = 'published'",
    )
    .bind(event_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (cost_type, sales_closed, past, age_rating) = row.ok_or(AppError::NotFound)?;
    if past {
        return Err(AppError::Conflict("подія вже завершилась".into()));
    }
    if sales_closed {
        return Err(AppError::Conflict("продаж квитків призупинено організатором".into()));
    }
    if cost_type != "free" {
        return Err(AppError::Conflict("платні квитки — через оплату (скоро)".into()));
    }
    if !crate::users::meets_age(&st.db, user.id, age_rating.as_deref()).await? {
        return Err(AppError::Conflict("ця подія має вікове обмеження".into()));
    }
    // One free ticket per user per event — prevents unlimited free minting. The partial
    // unique index is the race backstop; this gives a clean 409 instead of a DB error.
    let already: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM tickets WHERE event_id = $1 AND owner_id = $2 AND status = 'active' AND price = 0)",
    )
    .bind(event_id)
    .bind(user.id)
    .fetch_one(&mut *tx)
    .await?;
    if already {
        return Err(AppError::Conflict(
            "ви вже маєте безкоштовний квиток на цю подію".into(),
        ));
    }

    if decrement_tickets(&mut tx, event_id).await?.is_none() {
        return Err(AppError::Conflict("квитки закінчились".into()));
    }
    let id = mint_ticket(&mut tx, event_id, user.id, Decimal::ZERO).await?;

    tx.commit().await?;
    let ticket = fetch_ticket(&st.db, id).await?;
    crate::telemetry::log(
        &st.db,
        Some(user.id),
        "ticket",
        serde_json::json!({ "event_id": event_id, "free": true }),
    )
    .await;
    Ok((StatusCode::CREATED, Json(ticket)))
}

#[derive(Deserialize)]
pub struct ValidateReq {
    pub qr_token: String,
}

#[derive(Serialize)]
pub struct ValidateResp {
    pub valid: bool,
    pub reason: String,
    pub event_title: String,
}

/// `POST /api/tickets/validate` — organizer scans a QR at the door. Marks an active
/// ticket `used` (single-use). 403 if not the organizer's event, 404 if unknown token.
pub async fn validate(
    user: CurrentUser,
    State(st): State<AppState>,
    Json(req): Json<ValidateReq>,
) -> Result<Json<ValidateResp>, AppError> {
    let mut tx = st.db.begin().await?;
    let row: Option<(Uuid, String, Option<Uuid>, String)> = sqlx::query_as(
        "SELECT t.id, t.status, e.organizer_id, e.title \
         FROM tickets t JOIN events e ON e.id = t.event_id \
         WHERE t.qr_token = $1 FOR UPDATE OF t",
    )
    .bind(&req.qr_token)
    .fetch_optional(&mut *tx)
    .await?;
    let (ticket_id, status, organizer_id, title) = row.ok_or(AppError::NotFound)?;

    if organizer_id != Some(user.id) {
        return Err(AppError::Forbidden);
    }

    let (valid, reason) = match status.as_str() {
        "active" => {
            sqlx::query("UPDATE tickets SET status = 'used', validated_at = now() WHERE id = $1")
                .bind(ticket_id)
                .execute(&mut *tx)
                .await?;
            (true, "ok")
        }
        "used" => (false, "already_used"),
        _ => (false, "inactive"),
    };
    tx.commit().await?;
    Ok(Json(ValidateResp {
        valid,
        reason: reason.to_string(),
        event_title: title,
    }))
}

/// `POST /api/tickets/:id/return` — owner returns an active ticket (frees a seat,
/// refunds escrow for paid orders).
pub async fn return_ticket(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(ticket_id): Path<Uuid>,
) -> Result<Json<Ticket>, AppError> {
    let mut tx = st.db.begin().await?;
    let row: Option<(Uuid, String, Option<String>)> = sqlx::query_as(
        "SELECT t.event_id, t.status, o.escrow_status \
         FROM tickets t LEFT JOIN orders o ON o.ticket_id = t.id \
         WHERE t.id = $1 AND t.owner_id = $2 FOR UPDATE OF t",
    )
    .bind(ticket_id)
    .bind(user.id)
    .fetch_optional(&mut *tx)
    .await?;
    let (event_id, status, escrow) = row.ok_or(AppError::NotFound)?;
    if status != "active" {
        return Err(AppError::Conflict("квиток уже використаний або повернений".into()));
    }
    // Once funds have cleared (or been paid out) the refund window is over — the
    // organizer is owed/paid, so the buyer can't self-return for a refund.
    if matches!(escrow.as_deref(), Some("cleared") | Some("paid_out")) {
        return Err(AppError::Conflict("термін повернення квитка минув".into()));
    }

    sqlx::query("UPDATE tickets SET status = 'returned' WHERE id = $1")
        .bind(ticket_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE events SET tickets_left = tickets_left + 1 WHERE id = $1")
        .bind(event_id)
        .execute(&mut *tx)
        .await?;
    // Refund escrow while funds are still recallable (held or mid-clearing).
    sqlx::query(
        "UPDATE orders SET escrow_status = 'refunded' WHERE ticket_id = $1 AND escrow_status IN ('held', 'clearing')",
    )
    .bind(ticket_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(Json(fetch_ticket(&st.db, ticket_id).await?))
}

#[derive(Deserialize)]
pub struct TransferReq {
    pub email: String,
}

/// `POST /api/tickets/:id/transfer` — owner gifts an active ticket to another user by email.
pub async fn transfer_ticket(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(ticket_id): Path<Uuid>,
    Json(req): Json<TransferReq>,
) -> Result<Json<Ticket>, AppError> {
    let recipient: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE email = $1 AND blocked = false")
        .bind(req.email.trim().to_lowercase())
        .fetch_optional(&st.db)
        .await?;
    let recipient = recipient.ok_or(AppError::Validation("отримувача з таким email не знайдено".into()))?;
    if recipient == user.id {
        return Err(AppError::Validation("не можна подарувати квиток собі".into()));
    }
    // An age-restricted event's ticket cannot be gifted to someone who doesn't meet the rating
    // (otherwise transfer would bypass the checkout/reserve age gate).
    let age_rating: Option<String> = sqlx::query_scalar(
        "SELECT e.age_rating FROM tickets t JOIN events e ON e.id = t.event_id \
         WHERE t.id = $1 AND t.owner_id = $2 AND t.status = 'active'",
    )
    .bind(ticket_id)
    .bind(user.id)
    .fetch_optional(&st.db)
    .await?
    .flatten();
    if !crate::users::meets_age(&st.db, recipient, age_rating.as_deref()).await? {
        return Err(AppError::Conflict(
            "отримувач не відповідає віковому обмеженню події".into(),
        ));
    }
    // Money policy (BACK-1): a transfer gifts ENTRY only — the purchaser keeps the order/escrow
    // relationship and the right to dispute (the ticket DTO only surfaces the order to the buyer,
    // see TICKET_SELECT's `o.buyer_id = t.owner_id`). But a ticket whose order is under active
    // dispute (escrow frozen) must NOT be gifted away — a pending refund could void the QR.
    let frozen: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM orders WHERE ticket_id = $1 AND escrow_status = 'frozen')")
            .bind(ticket_id)
            .fetch_one(&st.db)
            .await?;
    if frozen {
        return Err(AppError::Conflict("квиток під спором — передавання недоступне".into()));
    }

    let updated: Option<Uuid> = sqlx::query_scalar(
        "UPDATE tickets SET owner_id = $3 WHERE id = $1 AND owner_id = $2 AND status = 'active' RETURNING id",
    )
    .bind(ticket_id)
    .bind(user.id)
    .bind(recipient)
    .fetch_optional(&st.db)
    .await?;
    updated.ok_or(AppError::NotFound)?;

    Ok(Json(fetch_ticket(&st.db, ticket_id).await?))
}

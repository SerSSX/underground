//! E-tickets: reserve a free ticket + list own tickets. Shared minting helpers
//! are reused by the paid checkout flow (`orders`).

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Duration, Utc};
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

/// Hand `n` seats back. The inverse of [`decrement_tickets`] and the ONLY place that adds to
/// `tickets_left`: clamped to `tickets_total`, because an organiser can lower capacity after
/// a sale and a return must not then create a seat that never existed. Every caller — a
/// buyer's return, an admin refund, an expired hold, a failed charge — goes through here so
/// the clamp cannot be forgotten at one of them.
pub async fn release_seats(conn: &mut sqlx::PgConnection, event_id: Uuid, n: i32) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE events SET tickets_left = LEAST(tickets_total, tickets_left + $2) WHERE id = $1")
        .bind(event_id)
        .bind(n)
        .execute(conn)
        .await?;
    Ok(())
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
#[derive(Serialize)]
pub struct QrResp {
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

/// `GET /api/tickets/:id/qr` — mint a signed, short-TTL door-QR for the owner's active ticket
/// (BR-023). The raw `qr_token` is never returned here; the client renders this rotating token
/// and auto-refreshes it before expiry.
pub async fn ticket_qr(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(ticket_id): Path<Uuid>,
) -> Result<Json<QrResp>, AppError> {
    let row: Option<(Uuid, String)> =
        sqlx::query_as("SELECT event_id, status FROM tickets WHERE id = $1 AND owner_id = $2")
            .bind(ticket_id)
            .bind(user.id)
            .fetch_optional(&st.db)
            .await?;
    let (event_id, status) = row.ok_or(AppError::NotFound)?;
    if status != "active" {
        return Err(AppError::Conflict("квиток неактивний".into()));
    }
    let (token, expires_at) = crate::qr::sign_qr(ticket_id, event_id, &st.jwt_secret)?;
    Ok(Json(QrResp { token, expires_at }))
}

pub async fn validate(
    user: CurrentUser,
    State(st): State<AppState>,
    Json(req): Json<ValidateReq>,
) -> Result<Json<ValidateResp>, AppError> {
    let mut tx = st.db.begin().await?;
    // Only a signed short-TTL QR is accepted at the door (BR-023). An invalid or expired token
    // resolves to nothing → NotFound, exactly like an unknown ticket. The raw qr_token is no
    // longer accepted (its plaintext value is never surfaced to a scanner).
    let row: Option<(Uuid, String, Option<Uuid>, String)> =
        match crate::qr::verify_qr(req.qr_token.trim(), &st.jwt_secret) {
            Some(c) => {
                sqlx::query_as(
                    "SELECT t.id, t.status, e.organizer_id, e.title \
                     FROM tickets t JOIN events e ON e.id = t.event_id WHERE t.id = $1 FOR UPDATE OF t",
                )
                .bind(c.tid)
                .fetch_optional(&mut *tx)
                .await?
            }
            None => None,
        };
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
    release_seats(&mut tx, event_id, 1).await?;
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

// ── Gift / claim (TASK-110, BR-018/BR-022, WF-GiftClaim) ────────────────────────────

/// SHA-256 of a raw claim token — only the hash is stored (BR-022).
fn hash_token(raw: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(raw.as_bytes()))
}

/// Claim-token lifetime (hours), overridable via env; default 48h (BR-022).
fn claim_ttl_hours() -> i64 {
    std::env::var("CLAIM_TOKEN_TTL_HOURS")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|h| *h > 0)
        .unwrap_or(48)
}

#[derive(Deserialize)]
pub struct GiftReq {
    pub email: String,
}

#[derive(Serialize)]
pub struct GiftResp {
    /// Raw single-use token — returned ONCE so the client can build the share link.
    pub claim_token: String,
    pub to_email: String,
    pub expires_at: DateTime<Utc>,
}

/// Ensure a ticket can be gifted now: not under dispute, and either `active` or a
/// `pending_claim` whose outstanding gift already expired (reclaimed for re-gifting).
async fn ensure_giftable(tx: &mut sqlx::PgConnection, ticket_id: Uuid, status: &str) -> Result<(), AppError> {
    let frozen: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM orders WHERE ticket_id = $1 AND escrow_status = 'frozen')")
            .bind(ticket_id)
            .fetch_one(&mut *tx)
            .await?;
    if frozen {
        return Err(AppError::Conflict("квиток під спором — передавання недоступне".into()));
    }
    match status {
        "active" => Ok(()),
        "pending_claim" => {
            let expired: bool = sqlx::query_scalar(
                "SELECT COALESCE(bool_or(expires_at < now()), false) \
                 FROM ticket_gifts WHERE ticket_id = $1 AND status = 'pending'",
            )
            .bind(ticket_id)
            .fetch_one(&mut *tx)
            .await?;
            if !expired {
                return Err(AppError::Conflict("квиток уже надіслано в подарунок".into()));
            }
            sqlx::query("UPDATE ticket_gifts SET status = 'expired' WHERE ticket_id = $1 AND status = 'pending'")
                .bind(ticket_id)
                .execute(&mut *tx)
                .await?;
            Ok(())
        }
        _ => Err(AppError::Conflict("квиток уже використаний або повернений".into())),
    }
}

/// `POST /api/tickets/:id/gift` — owner gifts a ticket by email. Works even if the
/// recipient has no account: a single-use claim link is created and the ticket locked.
pub async fn gift_ticket(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(ticket_id): Path<Uuid>,
    Json(req): Json<GiftReq>,
) -> Result<(StatusCode, Json<GiftResp>), AppError> {
    let email = req.email.trim().to_lowercase();
    if email.is_empty() {
        return Err(AppError::Validation("email отримувача обов'язковий".into()));
    }
    let mut tx = st.db.begin().await?;
    let status: Option<String> =
        sqlx::query_scalar("SELECT status FROM tickets WHERE id = $1 AND owner_id = $2 FOR UPDATE")
            .bind(ticket_id)
            .bind(user.id)
            .fetch_optional(&mut *tx)
            .await?;
    ensure_giftable(&mut tx, ticket_id, &status.ok_or(AppError::NotFound)?).await?;
    let raw = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let expires_at = Utc::now() + Duration::hours(claim_ttl_hours());
    sqlx::query(
        "INSERT INTO ticket_gifts (ticket_id, from_user_id, to_email, token_hash, expires_at) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(ticket_id)
    .bind(user.id)
    .bind(&email)
    .bind(hash_token(&raw))
    .bind(expires_at)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE tickets SET status = 'pending_claim' WHERE id = $1")
        .bind(ticket_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    crate::telemetry::log(
        &st.db,
        Some(user.id),
        "gift_created",
        serde_json::json!({ "ticket_id": ticket_id }),
    )
    .await;
    Ok((
        StatusCode::CREATED,
        Json(GiftResp {
            claim_token: raw,
            to_email: email,
            expires_at,
        }),
    ))
}

/// `DELETE /api/tickets/:id/gift` — the owner cancels an outstanding (unclaimed) gift,
/// reclaiming the ticket back to `active`. Atomic and race-safe: if the recipient already
/// claimed, there is no `pending` gift to lock and the revoke is rejected (claim wins, BR-018).
/// Money-neutral: does not touch `escrow.rs` (a gift never moved funds).
pub async fn revoke_gift(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(ticket_id): Path<Uuid>,
) -> Result<Json<Ticket>, AppError> {
    let mut tx = st.db.begin().await?;
    // Lock the outstanding gift (and its ticket) first — same lock order as `claim_ticket`
    // (gift → ticket) so a concurrent claim/revoke cannot deadlock.
    let gift: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT g.id, t.owner_id FROM ticket_gifts g JOIN tickets t ON t.id = g.ticket_id \
         WHERE g.ticket_id = $1 AND g.status = 'pending' FOR UPDATE OF g, t",
    )
    .bind(ticket_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (gift_id, owner_id) =
        gift.ok_or_else(|| AppError::Conflict("немає активного подарунка для скасування".into()))?;
    if owner_id != user.id {
        return Err(AppError::NotFound);
    }
    sqlx::query("UPDATE ticket_gifts SET status = 'revoked' WHERE id = $1")
        .bind(gift_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE tickets SET status = 'active' WHERE id = $1 AND status = 'pending_claim'")
        .bind(ticket_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    crate::telemetry::log(
        &st.db,
        Some(user.id),
        "gift_revoked",
        serde_json::json!({ "ticket_id": ticket_id }),
    )
    .await;
    Ok(Json(fetch_ticket(&st.db, ticket_id).await?))
}

#[derive(Deserialize)]
pub struct ClaimReq {
    pub token: String,
}

/// `POST /api/tickets/claim` — an authenticated user redeems a gift token, becoming the
/// ticket owner. The token is the bearer capability (BR-018: first valid claim wins).
pub async fn claim_ticket(
    user: CurrentUser,
    State(st): State<AppState>,
    Json(req): Json<ClaimReq>,
) -> Result<Json<Ticket>, AppError> {
    let mut tx = st.db.begin().await?;
    let gift: Option<(Uuid, Uuid, DateTime<Utc>)> = sqlx::query_as(
        "SELECT id, ticket_id, expires_at FROM ticket_gifts WHERE token_hash = $1 AND status = 'pending' FOR UPDATE",
    )
    .bind(hash_token(req.token.trim()))
    .fetch_optional(&mut *tx)
    .await?;
    let (gift_id, ticket_id, expires_at) = gift.ok_or(AppError::NotFound)?;
    if expires_at < Utc::now() {
        sqlx::query("UPDATE ticket_gifts SET status = 'expired' WHERE id = $1")
            .bind(gift_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE tickets SET status = 'active' WHERE id = $1 AND status = 'pending_claim'")
            .bind(ticket_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        return Err(AppError::Conflict("термін дії подарунка минув".into()));
    }
    let age: Option<String> =
        sqlx::query_scalar("SELECT e.age_rating FROM tickets t JOIN events e ON e.id = t.event_id WHERE t.id = $1")
            .bind(ticket_id)
            .fetch_optional(&mut *tx)
            .await?
            .flatten();
    if !crate::users::meets_age(&st.db, user.id, age.as_deref()).await? {
        return Err(AppError::Conflict("ви не відповідаєте віковому обмеженню події".into()));
    }
    sqlx::query("UPDATE tickets SET owner_id = $2, status = 'active' WHERE id = $1")
        .bind(ticket_id)
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE ticket_gifts SET status = 'claimed', claimed_by = $2, claimed_at = now() WHERE id = $1")
        .bind(gift_id)
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    crate::telemetry::log(
        &st.db,
        Some(user.id),
        "ticket_claimed",
        serde_json::json!({ "ticket_id": ticket_id }),
    )
    .await;
    Ok(Json(fetch_ticket(&st.db, ticket_id).await?))
}

/// Public preview of a pending gift, shown on the claim landing before sign-in
/// (browse-before-auth, UC-013): what's being gifted and by whom.
#[derive(Serialize, sqlx::FromRow)]
pub struct GiftPreview {
    pub from_name: String,
    pub event_title: String,
    pub event_starts_at: Option<DateTime<Utc>>,
    pub event_city: Option<String>,
    pub event_poster_url: Option<String>,
    pub expires_at: DateTime<Utc>,
}

/// `GET /api/gifts/:token` — public gift preview for the claim landing. 404 once the gift
/// is claimed/expired/unknown, so a spent link reveals nothing.
pub async fn gift_preview(
    State(st): State<AppState>,
    Path(token): Path<String>,
) -> Result<Json<GiftPreview>, AppError> {
    sqlx::query_as::<_, GiftPreview>(
        "SELECT u.name AS from_name, e.title AS event_title, e.starts_at AS event_starts_at, \
            e.city AS event_city, e.poster_url AS event_poster_url, g.expires_at \
         FROM ticket_gifts g JOIN tickets t ON t.id = g.ticket_id JOIN events e ON e.id = t.event_id \
         JOIN users u ON u.id = g.from_user_id \
         WHERE g.token_hash = $1 AND g.status = 'pending' AND g.expires_at > now()",
    )
    .bind(hash_token(token.trim()))
    .fetch_optional(&st.db)
    .await?
    .map(Json)
    .ok_or(AppError::NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claim_ttl_honors_env_then_defaults() {
        std::env::set_var("CLAIM_TOKEN_TTL_HOURS", "12");
        assert_eq!(claim_ttl_hours(), 12);
        std::env::set_var("CLAIM_TOKEN_TTL_HOURS", "0");
        assert_eq!(claim_ttl_hours(), 48, "non-positive TTL is ignored");
        std::env::remove_var("CLAIM_TOKEN_TTL_HOURS");
        assert_eq!(claim_ttl_hours(), 48);
    }

    #[test]
    fn hash_token_is_stable_and_distinct() {
        assert_eq!(hash_token("abc").len(), 64);
        assert_ne!(hash_token("abc"), hash_token("abd"));
    }
}

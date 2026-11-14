//! E-tickets: reserve a free ticket + list own tickets. Shared minting helpers
//! are reused by the paid checkout flow (`orders`).

use axum::body::Bytes;
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
        e.title AS event_title, e.starts_at AS event_starts_at, e.ends_at AS event_ends_at, e.city AS event_city, \
        e.venue_address AS event_venue_address, e.poster_url AS event_poster_url, e.timezone AS event_timezone, \
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
    /// When the event ends, if set. The web treats an event as over at `ends_at ?? starts_at + 12h`
    /// so late guests can still open their QR.
    #[sqlx(default)]
    pub event_ends_at: Option<DateTime<Utc>>,
    pub event_city: Option<String>,
    pub event_venue_address: Option<String>,
    pub event_poster_url: Option<String>,
    /// IANA zone the event's local time is shown in, so a ticket reads the same time as the event page.
    #[sqlx(default)]
    pub event_timezone: Option<String>,
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

/// Most places one person may hold on one free event (owner decision, TASK-302 Change C).
/// Counted over `active`, `pending_claim` and `used` tickets (D1): gifts still outstanding and
/// tickets already scanned keep counting, so neither gifting out nor entering frees the cap.
pub const MAX_PLACES_PER_PERSON: i64 = 5;

/// Rate-limit defaults (D8), overridable by env.
fn reserve_limit_per_event() -> i64 {
    crate::ratelimit::env_limit("RESERVE_LIMIT_PER_EVENT", 10)
}
fn reserve_limit_per_user() -> i64 {
    crate::ratelimit::env_limit("RESERVE_LIMIT_PER_USER", 30)
}
fn transfer_limit_per_user() -> i64 {
    crate::ratelimit::env_limit("TRANSFER_LIMIT_PER_USER", 30)
}

/// Parse the optional reserve body into a place count (TASK-302 CT1).
///
/// An empty body, `{}` and `{"quantity": null}` mean 1 (the Android client sends no body; the
/// web client may send `Content-Type: application/json` with no body). Anything else must be a
/// JSON object whose `quantity` is an integer in 1..=5; otherwise the request fails. It never
/// falls back to 1 on a bad body: that would book a place the caller did not ask for.
///
/// - not UTF-8, or JSON syntax / truncation errors → 400;
/// - not an object (`5`, `"5"`, `[2]` — a derived struct would accept the array form), wrong
///   type (`"2"`, `2.0`, `true`, `{..}`), duplicate `quantity`, or out of range → 422.
///
/// Unknown fields are ignored: the owner, event and price come from the token, path and event.
pub fn parse_quantity(body: &[u8]) -> Result<i64, AppError> {
    if body.is_empty() {
        return Ok(1);
    }
    let text =
        std::str::from_utf8(body).map_err(|_| AppError::BadRequest("тіло запиту не є коректним UTF-8".into()))?;
    if !text.trim_start_matches([' ', '\t', '\n', '\r']).starts_with('{') {
        return Err(AppError::Validation(
            "очікується JSON-об'єкт {\"quantity\": 1..5}".into(),
        ));
    }
    #[derive(Deserialize)]
    struct ReserveBody {
        quantity: Option<i64>,
    }
    let parsed: ReserveBody = serde_json::from_str(text).map_err(|e| match e.classify() {
        serde_json::error::Category::Data => AppError::Validation("quantity має бути цілим числом від 1 до 5".into()),
        _ => AppError::BadRequest("некоректний JSON".into()),
    })?;
    let quantity = parsed.quantity.unwrap_or(1);
    if !(1..=MAX_PLACES_PER_PERSON).contains(&quantity) {
        return Err(AppError::Validation("quantity має бути цілим числом від 1 до 5".into()));
    }
    Ok(quantity)
}

/// Live places `owner_id` holds on `event_id` (D1), optionally not counting one ticket (the
/// ticket being handed to them). Call under the event row lock.
async fn holdings(
    conn: &mut sqlx::PgConnection,
    event_id: Uuid,
    owner_id: Uuid,
    except_ticket: Option<Uuid>,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT count(*) FROM tickets \
         WHERE event_id = $1 AND owner_id = $2 AND status IN ('active', 'pending_claim', 'used') \
           AND ($3::uuid IS NULL OR id <> $3)",
    )
    .bind(event_id)
    .bind(owner_id)
    .bind(except_ticket)
    .fetch_one(conn)
    .await
}

/// Lock the event row. Every path that adds places to a person on a free event (reserve,
/// transfer, gift claim) takes this lock before counting, so the 5-place cap and the capacity
/// hold under concurrency (CT2, CT4). Lock order everywhere: gift → ticket → event.
async fn lock_event(conn: &mut sqlx::PgConnection, event_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT 1 FROM events WHERE id = $1 FOR UPDATE")
        .bind(event_id)
        .execute(conn)
        .await?;
    Ok(())
}

/// Recipient side of the cap (D3, CT4): 409 when `recipient` already holds the maximum on a
/// free event. Takes the event lock; call inside the transaction that moves the ticket.
async fn ensure_recipient_has_room(
    conn: &mut sqlx::PgConnection,
    event_id: Uuid,
    cost_type: &str,
    recipient: Uuid,
    ticket_id: Uuid,
) -> Result<(), AppError> {
    if cost_type != "free" {
        return Ok(());
    }
    lock_event(&mut *conn, event_id).await?;
    if holdings(&mut *conn, event_id, recipient, Some(ticket_id)).await? >= MAX_PLACES_PER_PERSON {
        return Err(AppError::Conflict(
            "отримувач уже має максимум місць на цю подію (5)".into(),
        ));
    }
    Ok(())
}

/// `POST /api/events/:id/reserve` — books 1..=5 free places for the caller (TASK-302 Change C).
///
/// Body: optional `{"quantity": 1..5}` (see [`parse_quantity`]). All-or-nothing: either every
/// requested place is booked or none is. Responds 201 with the FIRST created ticket, the same
/// shape as before, so clients that send no body keep working (CT16).
pub async fn reserve(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(event_id): Path<Uuid>,
    body: Bytes,
) -> Result<(StatusCode, Json<Ticket>), AppError> {
    // CT6: every call counts, whatever its outcome (a malformed body included).
    crate::ratelimit::hit(
        &st.db,
        &format!("reserve:{}:{}", user.id, event_id),
        reserve_limit_per_event(),
        3600,
    )
    .await?;
    crate::ratelimit::hit(&st.db, &format!("reserve:{}", user.id), reserve_limit_per_user(), 3600).await?;
    let quantity = parse_quantity(&body)?;

    let mut tx = st.db.begin().await?;
    // The row lock serialises every booking on this event: the cap count and the capacity
    // update below cannot interleave with another reserve, transfer or claim (CT2, CT3).
    let row: Option<(String, bool, bool, Option<String>)> = sqlx::query_as(
        "SELECT cost_type, sales_closed, (starts_at IS NOT NULL AND starts_at < now()) AS past, age_rating \
         FROM events WHERE id = $1 AND status = 'published' FOR UPDATE",
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
    if !crate::users::meets_age(&mut *tx, user.id, age_rating.as_deref()).await? {
        return Err(AppError::Conflict("ця подія має вікове обмеження".into()));
    }
    let held = holdings(&mut tx, event_id, user.id, None).await?;
    if held + quantity > MAX_PLACES_PER_PERSON {
        return Err(AppError::Conflict(format!(
            "на одну людину — не більше {MAX_PLACES_PER_PERSON} місць (у вас уже {held})"
        )));
    }
    // CT3: one conditional decrement for all places; no partial fill.
    let left: Option<i32> = sqlx::query_scalar(
        "UPDATE events SET tickets_left = tickets_left - $2 WHERE id = $1 AND tickets_left >= $2 RETURNING tickets_left",
    )
    .bind(event_id)
    .bind(quantity as i32)
    .fetch_optional(&mut *tx)
    .await?;
    if left.is_none() {
        return Err(AppError::Conflict("недостатньо вільних місць".into()));
    }
    let mut first = None;
    for _ in 0..quantity {
        let id = mint_ticket(&mut tx, event_id, user.id, Decimal::ZERO).await?;
        first.get_or_insert(id);
    }
    let first = first.ok_or(AppError::Internal)?;
    // UX7: the organiser gets an alert for the booking, in the same transaction.
    crate::notifications::notify_booking(&mut *tx, event_id, user.id, quantity).await?;

    tx.commit().await?;
    let ticket = fetch_ticket(&st.db, first).await?;
    crate::telemetry::log(
        &st.db,
        Some(user.id),
        "ticket",
        serde_json::json!({ "event_id": event_id, "free": true, "quantity": quantity }),
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
    let (token, expires_at) = crate::qr::sign_qr(ticket_id, event_id, user.id, &st.jwt_secret)?;
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
    type Row = (
        Uuid,
        String,
        Uuid,
        Option<Uuid>,
        String,
        String,
        Option<DateTime<Utc>>,
        String,
    );
    let row: Option<Row> = match crate::qr::verify_qr(req.qr_token.trim(), &st.jwt_secret) {
        Some(c) => sqlx::query_as::<_, Row>(
            "SELECT t.id, t.status, t.owner_id, e.organizer_id, e.title, e.status, e.starts_at, e.timezone \
                     FROM tickets t JOIN events e ON e.id = t.event_id WHERE t.id = $1 FOR UPDATE OF t",
        )
        .bind(c.tid)
        .fetch_optional(&mut *tx)
        .await?
        // CT10: a QR minted for a previous owner resolves to nothing.
        .filter(|r| r.2 == c.oid),
        None => None,
    };
    let (ticket_id, status, _owner, organizer_id, title, event_status, starts_at, timezone) =
        row.ok_or(AppError::NotFound)?;

    if organizer_id != Some(user.id) {
        return Err(AppError::Forbidden);
    }
    // UX2: the door opens 6 hours before the start; nothing changes before that.
    ensure_checkin_open(starts_at, &timezone, Utc::now())?;

    let (valid, reason) = if event_status == "cancelled" || event_status == "blocked" {
        // CT11 / D7: nobody is admitted to a cancelled or blocked event; nothing changes.
        (false, "event_cancelled")
    } else {
        match status.as_str() {
            "active" => {
                sqlx::query("UPDATE tickets SET status = 'used', validated_at = now() WHERE id = $1")
                    .bind(ticket_id)
                    .execute(&mut *tx)
                    .await?;
                (true, "ok")
            }
            "used" => (false, "already_used"),
            _ => (false, "inactive"),
        }
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

/// The one answer to a transfer by its owner, whatever the email (TASK-305 R2).
#[derive(Serialize)]
pub struct TransferSent {
    /// Always `"sent"`.
    pub status: &'static str,
}

/// SQL (events aliased `e`): the event is over — its end, or 12 hours after its start when it has
/// no end, has passed (the web's rule for when a ticket stops being useful).
pub const EVENT_OVER_SQL: &str = "COALESCE(e.ends_at, e.starts_at + interval '12 hours') < now()";

/// SQL (events aliased `e`): nobody can be given a place any more — the event is cancelled,
/// blocked or over (transfer, gift, claim). Never NULL.
fn event_gone_sql() -> String {
    format!("(e.status IN ('cancelled', 'blocked') OR {EVENT_OVER_SQL}) IS TRUE")
}

/// Door check-in opens this many hours before an event starts (TASK-305 UX2).
pub const CHECKIN_OPENS_HOURS: i64 = 6;

/// 409 «Ще рано: подія починається …» when `now` is earlier than [`CHECKIN_OPENS_HOURS`] before
/// the start, with the start in the event's own time zone. An event without a start is open.
/// Every scan and validate endpoint calls this before it changes anything (TASK-305 UX2).
pub fn ensure_checkin_open(
    starts_at: Option<DateTime<Utc>>,
    timezone: &str,
    now: DateTime<Utc>,
) -> Result<(), AppError> {
    let Some(start) = starts_at else {
        return Ok(());
    };
    if now >= start - Duration::hours(CHECKIN_OPENS_HOURS) {
        return Ok(());
    }
    let zone: chrono_tz::Tz = timezone.parse().unwrap_or(chrono_tz::Europe::Kyiv);
    let local = start.with_timezone(&zone);
    Err(AppError::Conflict(format!(
        "Ще рано: подія починається {}",
        local.format("%d.%m.%Y о %H:%M")
    )))
}

/// The answer for a ticket of a cancelled, blocked or finished event (transfer, gift, claim).
pub const EVENT_GONE: &str = "подію скасовано або вона вже минула";

/// 409 when the ticket's order is under dispute (escrow frozen): a pending refund could void the
/// QR, so the ticket cannot be handed on (BACK-1).
async fn ensure_not_disputed(tx: &mut sqlx::PgConnection, ticket_id: Uuid) -> Result<(), AppError> {
    let frozen: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM orders WHERE ticket_id = $1 AND escrow_status = 'frozen')")
            .bind(ticket_id)
            .fetch_one(&mut *tx)
            .await?;
    if frozen {
        return Err(AppError::Conflict("квиток під спором — передавання недоступне".into()));
    }
    Ok(())
}

/// The caller's own active ticket for a transfer, locked. 404 unless the caller owns it and it
/// is active (TASK-305 F2: checked before anything about the recipient is looked at); 409 when
/// its event is cancelled or over, or it is under dispute. Money policy (BACK-1): a transfer
/// gifts ENTRY only — the purchaser keeps the order/escrow relationship and the right to dispute.
///
/// A ticket whose earlier offer expired unclaimed counts as active again (TASK-305 round 3, as a
/// gift does): the expired offer is closed and the new one is made. An offer still open is 404.
async fn own_transferable_ticket(tx: &mut sqlx::PgConnection, ticket_id: Uuid, owner: Uuid) -> Result<(), AppError> {
    lock_pending_offers(tx, ticket_id).await?;
    let sql = format!(
        "SELECT t.status, {gone} FROM tickets t JOIN events e ON e.id = t.event_id \
         WHERE t.id = $1 AND t.owner_id = $2 AND t.status IN ('active', 'pending_claim') FOR UPDATE OF t",
        gone = event_gone_sql()
    );
    let row: Option<(String, bool)> = sqlx::query_as(&sql)
        .bind(ticket_id)
        .bind(owner)
        .fetch_optional(&mut *tx)
        .await?;
    let (status, gone) = row.ok_or(AppError::NotFound)?;
    if status == "pending_claim" && !reclaim_expired_offer(tx, ticket_id).await? {
        return Err(AppError::NotFound);
    }
    if gone {
        return Err(AppError::Conflict(EVENT_GONE.into()));
    }
    ensure_not_disputed(tx, ticket_id).await
}

/// The most characters an offer's email may have (RFC 5321 path limit).
pub const OFFER_EMAIL_MAX: usize = 254;

/// The one answer for an email that is not a single valid address (gift and transfer, TASK-305
/// round 3). It is decided from the text alone, never from whether an account has it.
pub const OFFER_EMAIL_REFUSED: &str = "вкажіть одну коректну адресу email";

/// The recipient email of an offer, trimmed and lower-cased: one address of at most
/// [`OFFER_EMAIL_MAX`] characters, or 422 [`OFFER_EMAIL_REFUSED`]. It is stored in
/// `ticket_gifts.to_email`, so its size is bounded here.
fn offer_email(raw: &str) -> Result<String, AppError> {
    let email = crate::auth::normalize_email(raw);
    if email.chars().count() > OFFER_EMAIL_MAX || !crate::mail::is_single_address(&email) {
        return Err(AppError::Validation(OFFER_EMAIL_REFUSED.into()));
    }
    Ok(email)
}

/// The account a place offer to `email` alerts: an active, not deleted account other than the
/// sender, found in one query (TASK-305 R2). Its age and place cap are not looked at here; they
/// are checked when the recipient accepts (`claim_ticket`).
async fn offer_recipient(tx: &mut sqlx::PgConnection, email: &str, sender: Uuid) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT id FROM users WHERE lower(email) = $1 AND blocked = false AND deleted_at IS NULL AND id <> $2",
    )
    .bind(email)
    .bind(sender)
    .fetch_optional(&mut *tx)
    .await
}

/// `POST /api/tickets/:id/transfer` — the owner offers an active ticket to an email
/// (owner decision 2026-11-14: a transfer is an offer the recipient accepts, like a gift).
///
/// The caller's own ticket is checked first (404 / 409 as for any action on it). After that the
/// answer is always 202 `{"status":"sent"}`, whatever the email, and the same thing happens to the
/// ticket: a pending offer (a gift without a link for the sender) that the sender sees and can
/// cancel with `DELETE /api/tickets/:id/gift`. Only when the email belongs to another account does
/// that account also get an in-app alert with the claim link; it accepts through
/// `POST /api/tickets/claim`, where the event's age rating and the place cap are checked and any
/// refusal is shown to the recipient alone. The ticket stays the sender's until then.
pub async fn transfer_ticket(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(ticket_id): Path<Uuid>,
    Json(req): Json<TransferReq>,
) -> Result<(StatusCode, Json<TransferSent>), AppError> {
    // P14: transfer and gift share one bucket per sender.
    crate::ratelimit::hit(
        &st.db,
        &format!("transfer:{}", user.id),
        transfer_limit_per_user(),
        3600,
    )
    .await?;
    let mut tx = st.db.begin().await?;
    // Lock order ticket → event, the same as `return_ticket`.
    own_transferable_ticket(&mut tx, ticket_id, user.id).await?;
    let email = offer_email(&req.email)?;
    // One lookup, and the same writes for every email; only the alert row depends on it.
    let recipient = offer_recipient(&mut tx, &email, user.id).await?;
    let offer = create_offer(&mut tx, &st.jwt_secret, ticket_id, user.id, &email).await?;
    if let Some(recipient) = recipient {
        crate::notifications::notify_place_offer(&mut *tx, recipient, user.id, ticket_id, offer.gift_id).await?;
    }
    tx.commit().await?;
    Ok((StatusCode::ACCEPTED, Json(TransferSent { status: "sent" })))
}

// ── Gift / claim (TASK-110, BR-018/BR-022, WF-GiftClaim) ────────────────────────────

/// The raw claim token of a gift: HMAC-SHA256 of the gift id under the server secret (TASK-305
/// R2, UX7). Only its SHA-256 is stored (BR-022), so a database leak still cannot rebuild a
/// working link, while the server can show the link again to the recipient of an offer.
pub(crate) fn claim_token(secret: &str, gift_id: Uuid) -> String {
    use hmac::{Hmac, Mac};
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC takes a key of any length");
    mac.update(b"underground/gift-claim/v1:");
    mac.update(gift_id.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

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

/// A pending offer of a ticket: the gift row and the raw token (returned to a gift's sender
/// only, never to a transfer's).
struct Offer {
    gift_id: Uuid,
    raw: String,
    expires_at: DateTime<Utc>,
}

/// Put a ticket on offer to `to_email` (gift and transfer): a pending gift row and the ticket in
/// `pending_claim`. The caller holds the ticket lock and has checked the ticket.
async fn create_offer(
    tx: &mut sqlx::PgConnection,
    secret: &str,
    ticket_id: Uuid,
    from: Uuid,
    to_email: &str,
) -> Result<Offer, AppError> {
    let gift_id = Uuid::new_v4();
    let raw = claim_token(secret, gift_id);
    let expires_at = Utc::now() + Duration::hours(claim_ttl_hours());
    sqlx::query(
        "INSERT INTO ticket_gifts (id, ticket_id, from_user_id, to_email, token_hash, expires_at) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(gift_id)
    .bind(ticket_id)
    .bind(from)
    .bind(to_email)
    .bind(hash_token(&raw))
    .bind(expires_at)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE tickets SET status = 'pending_claim' WHERE id = $1")
        .bind(ticket_id)
        .execute(&mut *tx)
        .await?;
    Ok(Offer {
        gift_id,
        raw,
        expires_at,
    })
}

/// Lock a ticket's pending offers before the ticket itself (TASK-305 round 3, N2): accepting and
/// cancelling lock the gift first, then the ticket, so a re-offer that may close an expired gift
/// takes them in the same order and the two cannot deadlock. Returns nothing: the caller has not
/// yet shown it owns the ticket.
async fn lock_pending_offers(tx: &mut sqlx::PgConnection, ticket_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT 1 FROM ticket_gifts WHERE ticket_id = $1 AND status = 'pending' ORDER BY id FOR UPDATE")
        .bind(ticket_id)
        .execute(&mut *tx)
        .await?;
    Ok(())
}

/// A `pending_claim` ticket whose outstanding offer has expired: the offer is marked expired and
/// true is returned (the caller makes a new one). False while the offer is still open. Runs under
/// the ticket lock.
async fn reclaim_expired_offer(tx: &mut sqlx::PgConnection, ticket_id: Uuid) -> Result<bool, AppError> {
    let expired: bool = sqlx::query_scalar(
        "SELECT COALESCE(bool_or(expires_at < now()), false) \
         FROM ticket_gifts WHERE ticket_id = $1 AND status = 'pending'",
    )
    .bind(ticket_id)
    .fetch_one(&mut *tx)
    .await?;
    if expired {
        sqlx::query("UPDATE ticket_gifts SET status = 'expired' WHERE ticket_id = $1 AND status = 'pending'")
            .bind(ticket_id)
            .execute(&mut *tx)
            .await?;
    }
    Ok(expired)
}

/// Ensure a ticket can be gifted now: its event is not cancelled, blocked or over (TASK-305 R6,
/// as for a transfer), not under dispute, and the ticket is either `active` or a `pending_claim`
/// whose outstanding gift already expired (reclaimed for re-gifting).
async fn ensure_giftable(
    tx: &mut sqlx::PgConnection,
    ticket_id: Uuid,
    status: &str,
    gone: bool,
) -> Result<(), AppError> {
    if gone {
        return Err(AppError::Conflict(EVENT_GONE.into()));
    }
    ensure_not_disputed(tx, ticket_id).await?;
    match status {
        "active" => Ok(()),
        "pending_claim" => {
            if !reclaim_expired_offer(tx, ticket_id).await? {
                return Err(AppError::Conflict("квиток уже надіслано в подарунок".into()));
            }
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
    // P14: transfer and gift share one bucket per sender.
    crate::ratelimit::hit(
        &st.db,
        &format!("transfer:{}", user.id),
        transfer_limit_per_user(),
        3600,
    )
    .await?;
    let email = offer_email(&req.email)?;
    let mut tx = st.db.begin().await?;
    // Lock order gift → ticket, as in claim and revoke (N2).
    lock_pending_offers(&mut tx, ticket_id).await?;
    let sql = format!(
        "SELECT t.status, {gone} FROM tickets t JOIN events e ON e.id = t.event_id \
         WHERE t.id = $1 AND t.owner_id = $2 FOR UPDATE OF t",
        gone = event_gone_sql()
    );
    let row: Option<(String, bool)> = sqlx::query_as(&sql)
        .bind(ticket_id)
        .bind(user.id)
        .fetch_optional(&mut *tx)
        .await?;
    let (status, gone) = row.ok_or(AppError::NotFound)?;
    ensure_giftable(&mut tx, ticket_id, &status, gone).await?;
    let recipient = offer_recipient(&mut tx, &email, user.id).await?;
    let Offer {
        gift_id,
        raw,
        expires_at,
    } = create_offer(&mut tx, &st.jwt_secret, ticket_id, user.id, &email).await?;
    // UX7: an account with that email gets an alert with the claim link.
    if let Some(recipient) = recipient {
        crate::notifications::notify_place_offer(&mut *tx, recipient, user.id, ticket_id, gift_id).await?;
    }
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
    // Lock order gift → ticket → event.
    let sql = format!(
        "SELECT t.event_id, e.age_rating, e.cost_type, {gone} \
         FROM tickets t JOIN events e ON e.id = t.event_id WHERE t.id = $1 FOR UPDATE OF t",
        gone = event_gone_sql()
    );
    let (event_id, age, cost_type, gone): (Uuid, Option<String>, String, bool) =
        sqlx::query_as(&sql).bind(ticket_id).fetch_one(&mut *tx).await?;
    // TASK-305: a gift for a cancelled or finished event is not claimed; the gift stays pending
    // (rolled back) so the sender can revoke it.
    if gone {
        return Err(AppError::Conflict(EVENT_GONE.into()));
    }
    if !crate::users::meets_age(&mut *tx, user.id, age.as_deref()).await? {
        return Err(AppError::Conflict("ви не відповідаєте віковому обмеженню події".into()));
    }
    // CT4 / D3: a claim at the cap is refused; the gift stays pending (the sender can revoke).
    ensure_recipient_has_room(&mut tx, event_id, &cost_type, user.id, ticket_id).await?;
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
    /// IANA zone the gift's event time is shown in (N3).
    #[sqlx(default)]
    pub event_timezone: Option<String>,
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
    // TASK-305 P: the sender's display name, never a placeholder that repeats their email.
    let sql = format!(
        "SELECT {name} AS from_name, e.title AS event_title, e.starts_at AS event_starts_at, \
            e.city AS event_city, e.poster_url AS event_poster_url, \
            e.timezone AS event_timezone, g.expires_at \
         FROM ticket_gifts g JOIN tickets t ON t.id = g.ticket_id JOIN events e ON e.id = t.event_id \
         JOIN users u ON u.id = g.from_user_id \
         WHERE g.token_hash = $1 AND g.status = 'pending' AND g.expires_at > now()",
        name = crate::users::DISPLAY_NAME_SQL
    );
    sqlx::query_as::<_, GiftPreview>(&sql)
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

    /// UX2: the door opens 6 hours before the start; earlier, 409 with the start in the event's
    /// own zone.
    #[test]
    fn checkin_opens_six_hours_before_the_start() {
        let start: DateTime<Utc> = "2030-07-01T17:00:00Z".parse().unwrap();
        let at = |h: i64, m: i64| start - Duration::hours(h) + Duration::minutes(m);
        assert!(ensure_checkin_open(Some(start), "Europe/Kyiv", at(6, 0)).is_ok());
        assert!(ensure_checkin_open(Some(start), "Europe/Kyiv", at(1, 0)).is_ok());
        assert!(ensure_checkin_open(Some(start), "Europe/Kyiv", start + Duration::hours(3)).is_ok());
        assert!(
            ensure_checkin_open(None, "Europe/Kyiv", at(100, 0)).is_ok(),
            "no start: open"
        );
        match ensure_checkin_open(Some(start), "Europe/Kyiv", at(6, -1)) {
            Err(AppError::Conflict(m)) => assert_eq!(m, "Ще рано: подія починається 01.07.2030 о 20:00"),
            other => panic!("{:?}", other.err()),
        }
        match ensure_checkin_open(Some(start), "America/New_York", at(48, 0)) {
            Err(AppError::Conflict(m)) => assert_eq!(m, "Ще рано: подія починається 01.07.2030 о 13:00"),
            other => panic!("{:?}", other.err()),
        }
    }

    /// The claim token is keyed: the same gift id under another secret gives another token, and
    /// it has the 64-hex shape of the earlier random tokens.
    #[test]
    fn claim_token_is_keyed_by_the_secret() {
        let id = Uuid::new_v4();
        let t = claim_token("s1", id);
        assert_eq!(t.len(), 64);
        assert!(t.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(t, claim_token("s1", id));
        assert_ne!(t, claim_token("s2", id));
        assert_ne!(t, claim_token("s1", Uuid::new_v4()));
    }

    #[test]
    fn hash_token_is_stable_and_distinct() {
        assert_eq!(hash_token("abc").len(), 64);
        assert_ne!(hash_token("abc"), hash_token("abd"));
    }
}

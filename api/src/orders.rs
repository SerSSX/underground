//! Paid checkout, the seat hold that protects it, and fulfilment once a provider says "paid".
//!
//! Money order of operations (PAY-3): the seat is taken at checkout, inside the same
//! transaction as the order row, for [`HOLD_MINUTES`]. Payment then either arrives in time
//! (ticket minted, seat already ours), arrives late (seat taken again if any is left), or
//! never arrives (the hold lapses and the seat goes back on sale). Nothing on the payment
//! path can fail after the buyer has been charged: when no ticket can be issued the order
//! becomes `refund_due` and the money goes back through `refund_jobs`.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::payments;
use crate::state::AppState;
use crate::tickets::{self, Ticket};

/// How long a checkout keeps its seat while the buyer pays. Long enough for a card form and
/// a 3-D Secure round trip; short enough that an abandoned tab does not sell out an event.
pub const HOLD_MINUTES: i32 = 15;

#[derive(Serialize)]
pub struct CheckoutResp {
    pub order_id: Uuid,
    pub mode: String,
    pub amount: Decimal,
    pub currency: String,
    pub test_mode: bool,
    /// Where the buyer goes to pay (hosted checkout); `None` for providers that settle inline.
    pub checkout_url: Option<String>,
    /// Until when the seat is held for this order. Clients show a countdown from it.
    pub hold_expires_at: DateTime<Utc>,
}

/// `GET /api/orders/:id` — the buyer's own order, for the payment screen and its countdown.
#[derive(Serialize, sqlx::FromRow)]
pub struct OrderView {
    pub id: Uuid,
    pub event_id: Uuid,
    pub status: String,
    pub escrow_status: String,
    pub amount: Decimal,
    pub currency: String,
    pub hold_expires_at: Option<DateTime<Utc>>,
    pub ticket_id: Option<Uuid>,
    pub checkout_url: Option<String>,
}

/// What [`fulfill`] did with a payment. It never fails on business grounds — see the module
/// doc — so a webhook handler can answer 200 on every arm.
#[derive(Debug, Clone, PartialEq)]
pub enum FulfillOutcome {
    /// A ticket was issued.
    Fulfilled(Uuid),
    /// The order was already paid, failed, or otherwise terminal. A replayed delivery.
    AlreadyHandled,
    /// The money arrived but no ticket could be issued (sold out, or the promo code was
    /// exhausted by an earlier payer). A refund has been queued.
    RefundDue,
}

#[derive(Deserialize)]
pub struct CheckoutQuery {
    /// Optional organizer promo code (`?promo=CODE`).
    pub promo: Option<String>,
}

/// Validate a promo code and apply its discount to the price. The use is NOT counted here —
/// counting happens at payment (`fulfill`), so an abandoned checkout never burns the code.
/// Unknown/expired/maxed codes are a hard error so the buyer isn't silently charged full price.
async fn apply_promo(
    db: &sqlx::PgPool,
    event_id: Uuid,
    price: Decimal,
    code: Option<&str>,
) -> Result<(Decimal, Option<String>), AppError> {
    let Some(code) = code.map(str::trim).filter(|c| !c.is_empty()) else {
        return Ok((price, None));
    };
    // Read-only validation (no mutation). The atomic max_uses enforcement is at payment.
    let pct: Option<i32> = sqlx::query_scalar(
        "SELECT percent_off FROM promo_codes \
         WHERE event_id = $1 AND lower(code) = lower($2) AND active \
           AND (expires_at IS NULL OR expires_at > now()) \
           AND (max_uses IS NULL OR used_count < max_uses)",
    )
    .bind(event_id)
    .bind(code)
    .fetch_optional(db)
    .await?;
    let pct = pct.ok_or_else(|| AppError::Validation("промокод недійсний".into()))?;
    let amount = (price * Decimal::from(100 - pct) / Decimal::from(100)).round_dp(2);
    Ok((amount, Some(code.to_string())))
}

/// `POST /api/events/:id/checkout` — creates a pending order for a paid event.
pub async fn checkout(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(event_id): Path<Uuid>,
    Query(q): Query<CheckoutQuery>,
) -> Result<(StatusCode, Json<CheckoutResp>), AppError> {
    // (title, price, currency, cost_type, tickets_left, sales_closed, past, age_rating).
    type CheckoutEventRow = (String, Decimal, String, String, i32, bool, bool, Option<String>);
    let row: Option<CheckoutEventRow> = sqlx::query_as(
        "SELECT title, price, currency, cost_type, tickets_left, sales_closed, \
            (starts_at IS NOT NULL AND starts_at < now()) AS past, age_rating \
         FROM events WHERE id = $1 AND status = 'published'",
    )
    .bind(event_id)
    .fetch_optional(&st.db)
    .await?;
    let (title, price, currency, cost_type, tickets_left, sales_closed, past, age_rating) =
        row.ok_or(AppError::NotFound)?;
    if past {
        return Err(AppError::Conflict("подія вже завершилась".into()));
    }
    if sales_closed {
        return Err(AppError::Conflict("продаж квитків призупинено організатором".into()));
    }
    if cost_type == "free" {
        return Err(AppError::Conflict(
            "безкоштовна подія — отримайте квиток без оплати".into(),
        ));
    }
    if tickets_left <= 0 {
        return Err(AppError::Conflict("квитки закінчились".into()));
    }
    if !crate::users::meets_age(&st.db, user.id, age_rating.as_deref()).await? {
        return Err(AppError::Conflict("ця подія має вікове обмеження".into()));
    }

    let settings = payments::load_settings(&st.db).await?;
    let test_mode = settings.mode == "test";
    // Resolve BEFORE inserting the order. A provider name the code does not know is an
    // operator error, and PAY-1 makes it loud: we no longer substitute the test provider
    // silently, because that either sells tickets nobody can pay for or — worse, once a real
    // key is present — takes money through a path nobody configured.
    let provider = crate::provider::resolve(&settings)
        .map_err(|e| AppError::Conflict(format!("платіжний провайдер не налаштований: {e}")))?;
    let (amount, applied_promo) = apply_promo(&st.db, event_id, price, q.promo.as_deref()).await?;
    // Our commission, snapshotted now. `platform_fee_bps` can change before this order is
    // refunded, and a refund must return what was actually taken.
    let fee = (amount * Decimal::from(settings.platform_fee_bps) / Decimal::from(10_000)).round_dp(2);

    // The reservation and the order row commit together, or not at all.
    let mut tx = st.db.begin().await?;
    // One buyer never holds two seats: an earlier unpaid checkout of theirs on this event is
    // closed and its seat handed back before a new one is taken. A buyer who reloads the
    // payment page gets a fresh hold, not a second one. If their old session still completes
    // at the provider, the late-payment path in `fulfill` handles it like any lapsed hold.
    let superseded: Vec<Uuid> = sqlx::query_scalar(
        "UPDATE orders SET status = 'expired' \
         WHERE event_id = $1 AND buyer_id = $2 AND status = 'pending' AND hold_expires_at IS NOT NULL \
         RETURNING id",
    )
    .bind(event_id)
    .bind(user.id)
    .fetch_all(&mut *tx)
    .await?;
    if !superseded.is_empty() {
        tickets::release_seats(&mut tx, event_id, superseded.len() as i32).await?;
    }
    // THE gate. A conditional atomic decrement is the only check that survives two buyers
    // arriving in the same millisecond; the `tickets_left <= 0` read above is a courtesy that
    // gives a clean message on the common path. Dropping `tx` here rolls everything back.
    if tickets::decrement_tickets(&mut tx, event_id).await?.is_none() {
        return Err(AppError::Conflict("квитки закінчились".into()));
    }
    let (order_id, hold_expires_at): (Uuid, DateTime<Utc>) = sqlx::query_as(
        "INSERT INTO orders (event_id, buyer_id, amount, currency, provider, promo_code, application_fee, hold_expires_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, now() + make_interval(mins => $8)) \
         RETURNING id, hold_expires_at",
    )
    .bind(event_id)
    .bind(user.id)
    .bind(amount)
    .bind(&currency)
    .bind(provider.name())
    .bind(&applied_promo)
    .bind(fee)
    .bind(HOLD_MINUTES)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;

    // Ask the provider to collect — AFTER the commit, so a slow or hung provider call never
    // holds a database transaction (and its connection) open. The order row exists first
    // because its id is the reference we hand the provider (metadata + idempotency key) and
    // the handle a refund will later need.
    let charge = provider
        .create_charge(crate::provider::ChargeReq {
            order_id,
            amount,
            currency: currency.clone(),
            return_url: Some(crate::passwords::web_base_url()),
            description: title,
            // Direct charges arrive with PAY-5, when an organiser has a connected account.
            connected_account: None,
            application_fee: fee,
        })
        .await;
    let charge = match charge {
        Ok(c) => c,
        Err(e) => {
            // Never leave a pending row the buyer could later be charged against — and give
            // the seat back, since this order will never be paid. Keyed on rows_affected so a
            // concurrent expiry cannot make us release the same seat twice.
            let mut tx = st.db.begin().await?;
            let closed = sqlx::query("UPDATE orders SET status = 'failed' WHERE id = $1 AND status = 'pending'")
                .bind(order_id)
                .execute(&mut *tx)
                .await?;
            if closed.rows_affected() > 0 {
                tickets::release_seats(&mut tx, event_id, 1).await?;
            }
            tx.commit().await?;
            return Err(AppError::Conflict(format!("оплату не вдалося створити: {e}")));
        }
    };
    sqlx::query("UPDATE orders SET provider_ref = $2, checkout_url = $3 WHERE id = $1")
        .bind(order_id)
        .bind(&charge.provider_ref)
        .bind(&charge.checkout_url)
        .execute(&st.db)
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(CheckoutResp {
            order_id,
            mode: settings.mode,
            amount,
            currency,
            test_mode,
            checkout_url: charge.checkout_url,
            hold_expires_at,
        }),
    ))
}

/// `GET /api/orders/:id` — buyer-scoped. An order that is not yours does not exist (404, not
/// 403), so the endpoint cannot be used to probe which ids are real.
pub async fn get_order(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(order_id): Path<Uuid>,
) -> Result<Json<OrderView>, AppError> {
    let order: Option<OrderView> = sqlx::query_as(
        "SELECT id, event_id, status, escrow_status, amount, currency, hold_expires_at, ticket_id, checkout_url \
         FROM orders WHERE id = $1 AND buyer_id = $2",
    )
    .bind(order_id)
    .bind(user.id)
    .fetch_optional(&st.db)
    .await?;
    Ok(Json(order.ok_or(AppError::NotFound)?))
}

/// Lapse every hold whose time is up and put its seat back on sale.
///
/// One statement, so a row cannot be counted twice: the orders flip and the seats return
/// under the same lock. A payment that races this — `fulfill` takes `FOR UPDATE` on the same
/// row — either lands before (order still pending, seat kept) or after (order expired, seat
/// re-taken if one is left). `hold_expires_at IS NOT NULL` excludes orders that predate the
/// hold and never reserved anything.
pub async fn expire_holds(db: &sqlx::PgPool) -> Result<u64, sqlx::Error> {
    let res = sqlx::query(
        "WITH lapsed AS ( \
             UPDATE orders SET status = 'expired' \
             WHERE status = 'pending' AND hold_expires_at IS NOT NULL AND hold_expires_at < now() \
             RETURNING event_id \
         ), per_event AS ( \
             SELECT event_id, count(*)::int AS n FROM lapsed GROUP BY event_id \
         ) \
         UPDATE events e SET tickets_left = LEAST(e.tickets_total, e.tickets_left + p.n) \
         FROM per_event p WHERE e.id = p.event_id",
    )
    .execute(db)
    .await?;
    Ok(res.rows_affected())
}

/// The money is in, the ticket is not coming: record the debt and queue the return.
/// Exactly one job per order, however many deliveries arrive (`refund_jobs.order_id` is unique).
async fn refund_due(
    conn: &mut sqlx::PgConnection,
    order_id: Uuid,
    amount: Decimal,
    currency: &str,
    reason: &str,
) -> Result<FulfillOutcome, AppError> {
    sqlx::query("UPDATE orders SET status = 'paid', escrow_status = 'refund_due' WHERE id = $1")
        .bind(order_id)
        .execute(&mut *conn)
        .await?;
    sqlx::query(
        "INSERT INTO refund_jobs (order_id, amount, currency, reason) VALUES ($1, $2, $3, $4) \
         ON CONFLICT (order_id) DO NOTHING",
    )
    .bind(order_id)
    .bind(amount)
    .bind(currency)
    .bind(reason)
    .execute(&mut *conn)
    .await?;
    tracing::warn!(%order_id, reason, "payment landed with no ticket to issue — refund queued");
    Ok(FulfillOutcome::RefundDue)
}

/// Apply a payment to an order: mint the ticket, move to paid + escrow `held`.
///
/// Never fails on business grounds. The seat was reserved at checkout; if the hold lapsed
/// and the seat is gone, or the promo code was used up by an earlier payer, the outcome is
/// [`FulfillOutcome::RefundDue`] — the buyer was charged, so the only honest answer is to
/// give the money back, not to tell the provider its delivery failed. Idempotent: a replayed
/// delivery for a handled order is [`FulfillOutcome::AlreadyHandled`]. Caller owns the
/// transaction.
pub async fn fulfill(conn: &mut sqlx::PgConnection, order_id: Uuid) -> Result<FulfillOutcome, AppError> {
    // (event_id, buyer_id, amount, currency, status, promo_code) for the locked order.
    type FulfillRow = (Uuid, Uuid, Decimal, String, String, Option<String>);
    let order: Option<FulfillRow> = sqlx::query_as(
        "SELECT event_id, buyer_id, amount, currency, status, promo_code FROM orders WHERE id = $1 FOR UPDATE",
    )
    .bind(order_id)
    .fetch_optional(&mut *conn)
    .await?;
    let (event_id, buyer_id, amount, currency, status, promo_code) = order.ok_or(AppError::NotFound)?;
    let seat_held = match status.as_str() {
        // Reserved at checkout; still ours.
        "pending" => true,
        // The hold lapsed and its seat went back on sale. The buyer paid anyway (a slow 3-DS
        // page, a tab reopened an hour later) — take a seat now if one is left.
        "expired" => tickets::decrement_tickets(conn, event_id).await?.is_some(),
        _ => return Ok(FulfillOutcome::AlreadyHandled),
    };
    if !seat_held {
        return refund_due(conn, order_id, amount, &currency, "місць не залишилось").await;
    }
    // Count the promo use at payment, atomically enforcing max_uses while the order row is
    // locked. Claimed BEFORE minting: if the code was exhausted since checkout (another buyer
    // paid first) the seat we hold goes back and the money goes back — no ticket at a price
    // the organiser did not agree to.
    // NOTE: we deliberately do NOT re-check `expires_at` here. The discount was validated and
    // locked when the order was created (apply_promo); honouring it at payment respects a
    // committed transaction. Only max_uses must be atomic, to prevent over-issuing the code.
    if let Some(code) = &promo_code {
        let claimed: Option<Uuid> = sqlx::query_scalar(
            "UPDATE promo_codes SET used_count = used_count + 1 \
             WHERE event_id = $1 AND lower(code) = lower($2) AND active \
               AND (max_uses IS NULL OR used_count < max_uses) \
             RETURNING id",
        )
        .bind(event_id)
        .bind(code)
        .fetch_optional(&mut *conn)
        .await?;
        if claimed.is_none() {
            tickets::release_seats(conn, event_id, 1).await?;
            return refund_due(conn, order_id, amount, &currency, "промокод вичерпано").await;
        }
    }
    let ticket_id = tickets::mint_ticket(conn, event_id, buyer_id, amount).await?;
    sqlx::query("UPDATE orders SET status = 'paid', escrow_status = 'held', ticket_id = $2 WHERE id = $1")
        .bind(order_id)
        .bind(ticket_id)
        .execute(&mut *conn)
        .await?;
    Ok(FulfillOutcome::Fulfilled(ticket_id))
}

/// `POST /api/orders/:id/pay-test` — simulates a successful payment (test mode only):
/// mints the ticket and moves funds into escrow `held`.
pub async fn pay_test(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(order_id): Path<Uuid>,
) -> Result<(StatusCode, Json<Ticket>), AppError> {
    let settings = payments::load_settings(&st.db).await?;
    if settings.mode != "test" {
        return Err(AppError::Conflict("тестова оплата доступна лише в test-режимі".into()));
    }
    // Authorize: only the buyer may pay-test their own order (unknown/other order → 404).
    let row: Option<(Uuid, String)> = sqlx::query_as("SELECT buyer_id, status FROM orders WHERE id = $1")
        .bind(order_id)
        .fetch_optional(&st.db)
        .await?;
    let Some((owner, status)) = row else {
        return Err(AppError::NotFound);
    };
    if owner != user.id {
        return Err(AppError::NotFound);
    }
    // A human paying in test mode is paying NOW. A lapsed hold means "start over", the same
    // thing the real payment page tells them — the late-payment path is for a provider
    // delivering a payment the buyer completed before the hold ran out.
    if status == "expired" {
        return Err(AppError::Conflict("час бронювання минув, спробуйте ще раз".into()));
    }

    let mut tx = st.db.begin().await?;
    let outcome = fulfill(&mut tx, order_id).await?;
    tx.commit().await?;
    match outcome {
        FulfillOutcome::Fulfilled(ticket_id) => {
            let ticket = tickets::fetch_ticket(&st.db, ticket_id).await?;
            Ok((StatusCode::CREATED, Json(ticket)))
        }
        FulfillOutcome::AlreadyHandled => Err(AppError::Conflict("замовлення вже оброблене".into())),
        FulfillOutcome::RefundDue => Err(AppError::Conflict("місць не залишилось — гроші повертаються".into())),
    }
}

/// `POST /api/orders/:id/dispute` — the buyer opens a dispute; freezes escrow so the
/// organizer can't be paid out until an admin resolves it (refund or release).
pub async fn dispute(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(order_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let res = sqlx::query(
        "UPDATE orders SET escrow_status = 'frozen' \
         WHERE id = $1 AND buyer_id = $2 AND escrow_status IN ('held', 'clearing')",
    )
    .bind(order_id)
    .bind(user.id)
    .execute(&st.db)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::Conflict(
            "дисп можливий лише для активного замовлення з вашими коштами".into(),
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

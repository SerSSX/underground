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
use std::sync::Arc;

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
    let (title, price, currency, cost_type, _tickets_left, sales_closed, past, age_rating) =
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
    // No early `tickets_left <= 0` refusal here: a buyer re-checking out on the LAST seat is
    // holding that seat themselves, and the supersede step below hands it back before the
    // atomic decrement — which is the real gate and gives the same 409 when truly sold out.
    if !crate::users::meets_age(&st.db, user.id, age_rating.as_deref()).await? {
        return Err(AppError::Conflict("ця подія має вікове обмеження".into()));
    }

    let settings = payments::load_settings(&st.db).await?;
    let test_mode = settings.mode == "test";
    // Resolve BEFORE inserting the order. A provider name the code does not know is an
    // operator error, and PAY-1 makes it loud: we no longer substitute the test provider
    // silently, because that either sells tickets nobody can pay for or — worse, once a real
    // key is present — takes money through a path nobody configured.
    // The provider's reason is for the operator's log, not the buyer's screen: it names our
    // configuration state ("stripe selected but no secret key configured"), and a buyer is
    // nobody who should learn that.
    // Resolved ONCE into an Arc: the checkout uses it for `create_charge`, and the superseded-
    // session void (spawned below) needs an owned handle too. Re-resolving there would be a
    // second, deterministically-identical call whose error arm is unreachable (REVIEW-4 gate).
    let provider: Arc<dyn crate::provider::PaymentProvider> = crate::provider::resolve(&settings)
        .map_err(|e| {
            tracing::error!(error = %e, "checkout: payment provider unavailable");
            AppError::Conflict("платіжна система тимчасово недоступна, спробуйте за хвилину".into())
        })?
        .into();
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
    // Hold-less orders (from before the reservation existed) are superseded too, so no buyer
    // keeps a payable order the one-hold rule cannot see (REVIEW-3 #6) — but only a HELD
    // order hands a seat back, because only a held order ever took one (BR-025).
    let superseded: Vec<(Uuid, Option<String>, bool)> = sqlx::query_as(
        "UPDATE orders SET status = 'expired', terminal_event_seen = 'superseded' \
         WHERE event_id = $1 AND buyer_id = $2 AND status = 'pending' \
         RETURNING id, provider_ref, hold_expires_at IS NOT NULL",
    )
    .bind(event_id)
    .bind(user.id)
    .fetch_all(&mut *tx)
    .await?;
    let held = superseded.iter().filter(|(_, _, held)| *held).count() as i32;
    if held > 0 {
        tickets::release_seats(&mut tx, event_id, held).await?;
    }
    // THE gate. A conditional atomic decrement is the only check that survives two buyers
    // arriving in the same millisecond; the `tickets_left <= 0` read above is a courtesy that
    // gives a clean message on the common path. Dropping `tx` here rolls everything back.
    if tickets::decrement_tickets(&mut tx, event_id).await?.is_none() {
        return Err(AppError::Conflict("квитки закінчились".into()));
    }
    let inserted: Result<(Uuid, DateTime<Utc>), sqlx::Error> = sqlx::query_as(
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
    .await;
    let (order_id, hold_expires_at) = match inserted {
        Ok(row) => row,
        // Two checkouts of ONE buyer racing each other both saw nothing to supersede — the
        // database is the referee: the partial unique index `orders_one_live_hold_uidx`
        // (one pending order per buyer per event) refuses the second INSERT, and dropping
        // `tx` hands back the seat it had just taken (BR-026, REVIEW-4 #2).
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            return Err(AppError::Conflict(
                "у вас уже є активний checkout на цю подію — відкрийте його або спробуйте ще раз".into(),
            ));
        }
        Err(e) => return Err(AppError::Db(e)),
    };
    tx.commit().await?;

    // The superseded checkouts may still have a live payment page open somewhere. Void them at
    // the provider so that page stops accepting money; best effort — the `superseded` marker
    // on the order is what actually turns a late payment into a refund rather than a ticket.
    // Off the buyer's request (REVIEW-3 #4): a degraded provider must not add its timeout to
    // a re-checkout for work that is best effort by contract.
    let victims: Vec<(Uuid, String)> = superseded
        .into_iter()
        .filter_map(|(id, r, _)| r.map(|r| (id, r)))
        .collect();
    if !victims.is_empty() {
        // A clone of the already-resolved provider — no second resolve, so no unreachable error
        // arm — moved into the task that owns the best-effort void.
        let voider = Arc::clone(&provider);
        tokio::spawn(async move {
            for (old_id, r) in victims {
                if let Err(e) = voider.cancel_charge(&r).await {
                    tracing::warn!(order_id = %old_id, error = %e, "checkout: could not void superseded session");
                }
            }
        });
    }

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
            // 1 in every real run; 0 only if a concurrent sweep or payment already moved the
            // order off 'pending', and releasing zero seats is a no-op by construction.
            tickets::release_seats(&mut tx, event_id, closed.rows_affected() as i32).await?;
            tx.commit().await?;
            // Same rule: the provider's text goes to the log. An `Invalid` from our own
            // pre-checks (below the currency minimum, fee ≥ amount) IS buyer-actionable and
            // contains nothing about us, so it is passed through; transport and upstream
            // failures are not.
            tracing::error!(%order_id, error = %e, "checkout: provider refused the charge");
            let msg = match &e {
                crate::provider::ProviderError::Invalid(m) => m.clone(),
                _ => "платіжна система тимчасово недоступна, спробуйте за хвилину".to_string(),
            };
            return Err(AppError::Conflict(msg));
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

/// The provider says the buyer has their money back in full: undo the sale on our side.
///
/// One routine for every refund path (webhook, admin, buyer return) so the four side effects
/// cannot drift apart: the ticket is no longer valid at the door, the seat is back on sale,
/// the promo use is returned, and the escrow row stops advancing toward a payout of money
/// that has already gone back. Idempotent — keyed on the ticket flip's `rows_affected`, a
/// second full-refund notice changes nothing.
pub async fn apply_full_refund(conn: &mut sqlx::PgConnection, order_id: Uuid) -> Result<(), AppError> {
    type Row = (
        Uuid,
        Option<Uuid>,
        Option<String>,
        Decimal,
        String,
        Option<DateTime<Utc>>,
    );
    let row: Option<Row> = sqlx::query_as(
        "SELECT event_id, ticket_id, promo_code, amount, status, hold_expires_at FROM orders WHERE id = $1 FOR UPDATE",
    )
    .bind(order_id)
    .fetch_optional(&mut *conn)
    .await?;
    let Some((event_id, ticket_id, promo_code, amount, status, hold_expires_at)) = row else {
        return Ok(());
    };
    return_holdings(
        conn,
        event_id,
        ticket_id,
        promo_code.as_deref(),
        &status,
        hold_expires_at,
    )
    .await?;
    // The provider itself returned the money, so a debt we had queued for the refund executor is
    // settled — the ledger must not keep saying we owe what has demonstrably gone back (REVIEW-4 #1).
    sqlx::query("UPDATE refund_jobs SET status = 'done' WHERE order_id = $1 AND status = 'pending'")
        .bind(order_id)
        .execute(&mut *conn)
        .await?;
    sqlx::query(
        "UPDATE orders SET status = 'refunded', escrow_status = 'refunded', refunded_amount = $2, \
            terminal_event_seen = COALESCE(terminal_event_seen, 'refunded') WHERE id = $1",
    )
    .bind(order_id)
    .bind(amount)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Hand back whatever an order still holds: an issued ticket (returned; its seat and promo use
/// go back) or a pending hold's seat (a pending order with a hold took a seat and has no
/// ticket to return). The ONE routine behind the admin refund and the provider's full-refund
/// notice — they drifted apart twice before this existed (REVIEW-2 N4, REVIEW-3 #2).
pub async fn return_holdings(
    conn: &mut sqlx::PgConnection,
    event_id: Uuid,
    ticket_id: Option<Uuid>,
    promo_code: Option<&str>,
    status: &str,
    hold_expires_at: Option<DateTime<Utc>>,
) -> Result<(), AppError> {
    if let Some(ticket_id) = ticket_id {
        // Keyed on the ticket flip's rows_affected so a repeated refund is a no-op.
        let returned =
            sqlx::query("UPDATE tickets SET status = 'returned' WHERE id = $1 AND status IN ('active','used')")
                .bind(ticket_id)
                .execute(&mut *conn)
                .await?;
        if returned.rows_affected() > 0 {
            tickets::release_seats(conn, event_id, 1).await?;
            // The fulfilled order counted its promo use at payment — give it back.
            if let Some(code) = promo_code {
                sqlx::query(
                    "UPDATE promo_codes SET used_count = used_count - 1 \
                     WHERE event_id = $1 AND lower(code) = lower($2) AND used_count > 0",
                )
                .bind(event_id)
                .bind(code)
                .execute(&mut *conn)
                .await?;
            }
        }
    } else if status == "pending" && hold_expires_at.is_some() {
        // Once the order leaves 'pending' the expiry sweep never sees it again, so this is the
        // only moment its seat can come back.
        tickets::release_seats(conn, event_id, 1).await?;
    }
    Ok(())
}

/// Record the debt: exactly one refund job per order, however many deliveries or admin
/// actions arrive (`refund_jobs.order_id` is unique). The refund executor (PAY-11) drains it.
/// Returns whether the job is NEW — false means the debt was already on record.
pub async fn queue_refund(
    conn: &mut sqlx::PgConnection,
    order_id: Uuid,
    amount: Decimal,
    currency: &str,
    reason: &str,
) -> Result<bool, AppError> {
    let inserted = sqlx::query(
        "INSERT INTO refund_jobs (order_id, amount, currency, reason) VALUES ($1, $2, $3, $4) \
         ON CONFLICT (order_id) DO NOTHING",
    )
    .bind(order_id)
    .bind(amount)
    .bind(currency)
    .bind(reason)
    .execute(&mut *conn)
    .await?;
    Ok(inserted.rows_affected() > 0)
}

/// The money is in, the ticket is not coming: mark the order and queue the return.
pub async fn refund_due(
    conn: &mut sqlx::PgConnection,
    order_id: Uuid,
    amount: Decimal,
    currency: &str,
    reason: &str,
) -> Result<FulfillOutcome, AppError> {
    // Queue FIRST and key everything on whether the job is new: a further payment event on an
    // order whose debt is already on record — or already settled by an admin — must not flip
    // the order back to refund_due. One debt, one row, one status change (REVIEW-4).
    if !queue_refund(conn, order_id, amount, currency, reason).await? {
        tracing::info!(%order_id, "payment on an order whose refund is already on record — nothing new");
        return Ok(FulfillOutcome::AlreadyHandled);
    }
    // A distinct status, not `paid`: `paid` is what the confirmation email, organiser revenue
    // and platform GMV all key on, and none of those may count money that is going back.
    sqlx::query("UPDATE orders SET status = 'refund_due', escrow_status = 'refund_due' WHERE id = $1")
        .bind(order_id)
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
    // (event_id, buyer_id, amount, currency, status, promo_code, hold, terminal marker).
    type FulfillRow = (
        Uuid,
        Uuid,
        Decimal,
        String,
        String,
        Option<String>,
        Option<DateTime<Utc>>,
        Option<String>,
    );
    let order: Option<FulfillRow> = sqlx::query_as(
        "SELECT event_id, buyer_id, amount, currency, status, promo_code, hold_expires_at, terminal_event_seen \
         FROM orders WHERE id = $1 FOR UPDATE",
    )
    .bind(order_id)
    .fetch_optional(&mut *conn)
    .await?;
    let (event_id, buyer_id, amount, currency, status, promo_code, hold_expires_at, terminal) =
        order.ok_or(AppError::NotFound)?;
    // The guard lives HERE, in the function that mints, not only in the webhook that usually
    // calls it (REVIEW-3 #5). A terminal marker means the order was closed before this money
    // arrived; which way it was closed decides what happens to the money.
    if let Some(marker) = terminal.as_deref() {
        return match marker {
            // Closed on our side before any payment — at re-checkout (superseded) or by an
            // admin (cancelled). Its seat is gone; the money goes back (BR-014, BR-026).
            "superseded" | "cancelled" => {
                refund_due(conn, order_id, amount, &currency, "замовлення закрито до оплати").await
            }
            // Refunded or disputed AT THE PROVIDER: the money is already on its way back through
            // the provider's own process — queueing a second return would pay twice.
            _ => Ok(FulfillOutcome::AlreadyHandled),
        };
    }
    let seat_held = match status.as_str() {
        // Reserved at checkout; still ours. ONLY with a hold: an order from before the hold
        // existed (hold_expires_at NULL) never took a seat, so treating it as reserved would
        // mint a ticket without consuming capacity (BR-025, REVIEW-2 N2).
        "pending" if hold_expires_at.is_some() => true,
        // The hold lapsed and its seat went back on sale. The buyer paid anyway (a slow 3-DS
        // page, a tab reopened an hour later) — take a seat now if one is left.
        "pending" | "expired" => tickets::decrement_tickets(conn, event_id).await?.is_some(),
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

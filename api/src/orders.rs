//! Paid checkout + test-mode payment simulation. Real Lemon Squeezy live flow
//! (hosted checkout + webhook) plugs in here once configured via admin.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::payments;
use crate::state::AppState;
use crate::tickets::{self, Ticket};

#[derive(Serialize)]
pub struct CheckoutResp {
    pub order_id: Uuid,
    pub mode: String,
    pub amount: Decimal,
    pub currency: String,
    pub test_mode: bool,
    /// Lemon Squeezy hosted-checkout URL (live mode only); the order id rides along as
    /// custom data so the webhook can match the payment back to this order.
    pub checkout_url: Option<String>,
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
    // (price, currency, cost_type, tickets_left, sales_closed, past, age_rating) for a published event.
    type CheckoutEventRow = (Decimal, String, String, i32, bool, bool, Option<String>);
    let row: Option<CheckoutEventRow> = sqlx::query_as(
        "SELECT price, currency, cost_type, tickets_left, sales_closed, \
            (starts_at IS NOT NULL AND starts_at < now()) AS past, age_rating \
         FROM events WHERE id = $1 AND status = 'published'",
    )
    .bind(event_id)
    .fetch_optional(&st.db)
    .await?;
    let (price, currency, cost_type, tickets_left, sales_closed, past, age_rating) = row.ok_or(AppError::NotFound)?;
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
    // Live mode with no hosted-checkout variant configured can't actually take a payment —
    // reject before creating an order so we don't leave an orphan pending row (SEC/$).
    if !test_mode
        && settings
            .ls_variant_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .is_none()
    {
        return Err(AppError::Conflict("live-платежі ще не налаштовані".into()));
    }
    let (amount, applied_promo) = apply_promo(&st.db, event_id, price, q.promo.as_deref()).await?;
    let order_id: Uuid = sqlx::query_scalar(
        "INSERT INTO orders (event_id, buyer_id, amount, currency, provider, promo_code) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(event_id)
    .bind(user.id)
    .bind(amount)
    .bind(&currency)
    .bind(&settings.provider)
    .bind(&applied_promo)
    .fetch_one(&st.db)
    .await?;

    // In live mode, hand the client a Lemon Squeezy hosted-checkout URL carrying our order id.
    let checkout_url = (!test_mode)
        .then(|| {
            settings.ls_variant_id.as_deref().map(|variant| {
                let store = settings.ls_store_id.as_deref().unwrap_or("store");
                format!("https://{store}.lemonsqueezy.com/buy/{variant}?checkout[custom][order_id]={order_id}")
            })
        })
        .flatten();
    Ok((
        StatusCode::CREATED,
        Json(CheckoutResp {
            order_id,
            mode: settings.mode,
            amount,
            currency,
            test_mode,
            checkout_url,
        }),
    ))
}

/// Fulfill a pending order: decrement stock, mint the ticket, move to paid + escrow `held`.
/// Returns the new ticket id, or `None` if the order is no longer pending (idempotent — a
/// duplicate webhook or double-submit is a safe no-op). Caller owns the transaction.
pub async fn fulfill(conn: &mut sqlx::PgConnection, order_id: Uuid) -> Result<Option<Uuid>, AppError> {
    // (event_id, buyer_id, amount, status, promo_code) for the locked pending order.
    type FulfillRow = (Uuid, Uuid, Decimal, String, Option<String>);
    let order: Option<FulfillRow> =
        sqlx::query_as("SELECT event_id, buyer_id, amount, status, promo_code FROM orders WHERE id = $1 FOR UPDATE")
            .bind(order_id)
            .fetch_optional(&mut *conn)
            .await?;
    let (event_id, buyer_id, amount, status, promo_code) = order.ok_or(AppError::NotFound)?;
    if status != "pending" {
        return Ok(None);
    }
    if tickets::decrement_tickets(conn, event_id).await?.is_none() {
        return Err(AppError::Conflict("квитки закінчились".into()));
    }
    let ticket_id = tickets::mint_ticket(conn, event_id, buyer_id, amount).await?;
    // UX7: the organiser gets an alert for the paid booking, in the same transaction.
    crate::notifications::notify_booking(&mut *conn, event_id, buyer_id, 1).await?;
    sqlx::query("UPDATE orders SET status = 'paid', escrow_status = 'held', ticket_id = $2 WHERE id = $1")
        .bind(order_id)
        .bind(ticket_id)
        .execute(&mut *conn)
        .await?;
    // Count the promo use at payment, atomically enforcing max_uses while the order row is
    // locked. If the code was exhausted since checkout (e.g. another buyer paid first), the
    // whole fulfillment rolls back — no ticket, no stock loss — and the caller sees 409.
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
            return Err(AppError::Conflict("промокод вичерпано".into()));
        }
    }
    Ok(Some(ticket_id))
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
    let owner: Option<Uuid> = sqlx::query_scalar("SELECT buyer_id FROM orders WHERE id = $1")
        .bind(order_id)
        .fetch_optional(&st.db)
        .await?;
    if owner != Some(user.id) {
        return Err(AppError::NotFound);
    }

    let mut tx = st.db.begin().await?;
    let ticket_id = fulfill(&mut tx, order_id)
        .await?
        .ok_or_else(|| AppError::Conflict("замовлення вже оброблене".into()))?;
    tx.commit().await?;
    let ticket = tickets::fetch_ticket(&st.db, ticket_id).await?;
    Ok((StatusCode::CREATED, Json(ticket)))
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

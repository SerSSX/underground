//! Organizer payouts (TASK-017 M6). Settling CLEARED escrow to an organizer, net of the
//! platform commission — one payout row per currency. This is the deliberate seam: money
//! only reaches an organizer through an explicit admin-issued settlement after clearing.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::CurrentAdmin;
use crate::error::AppError;
use crate::state::AppState;

#[derive(Serialize, sqlx::FromRow)]
pub struct Payout {
    pub id: Uuid,
    pub organizer_id: Uuid,
    pub amount: Decimal,
    pub currency: String,
    pub order_count: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

/// `POST /api/admin/organizers/:id/payout` — settle all CLEARED escrow for an organizer:
/// mark those orders `paid_out` and record one payout per currency (net of commission).
/// Returns the created payouts (empty when there is nothing to settle).
pub async fn create_payout(
    admin: CurrentAdmin,
    State(st): State<AppState>,
    Path(organizer_id): Path<Uuid>,
) -> Result<Json<Vec<Payout>>, AppError> {
    // Nothing is settled unless the organiser is active (not blocked/erased), has passed OUR
    // moderation, AND the payment provider says their account can actually receive money.
    //
    // That last condition is the one PAY-5 adds, and it is not redundant: `kyc_status` is our
    // own verdict, which an admin sets by hand. On its own it would let us mark orders
    // `paid_out` for an organiser whose provider account is restricted — bookkeeping that says
    // the money went somewhere it cannot go.
    type GateRow = (bool, String, bool, Option<String>);
    let row: Option<GateRow> = sqlx::query_as(
        "SELECT blocked, kyc_status, psp_payouts_enabled, psp_account_id FROM users \
         WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(organizer_id)
    .fetch_optional(&st.db)
    .await?;
    let ready = matches!(
        row,
        Some((false, ref s, true, Some(ref acct))) if s == "verified" && !acct.is_empty()
    );
    if !ready {
        return Err(AppError::Conflict(
            "організатор заблокований, не пройшов KYC або не підключив виплати — виплата заблокована".into(),
        ));
    }
    let mut tx = st.db.begin().await?;
    let payouts: Vec<Payout> = sqlx::query_as(
        "WITH to_pay AS ( \
            SELECT o.id, o.currency, o.amount FROM orders o JOIN events e ON e.id = o.event_id \
            WHERE e.organizer_id = $1 AND o.escrow_status = 'cleared' \
         ), \
         marked AS (UPDATE orders SET escrow_status = 'paid_out' WHERE id IN (SELECT id FROM to_pay)), \
         fee AS (SELECT platform_fee_bps AS bps FROM payment_settings WHERE id = TRUE) \
         INSERT INTO payouts (organizer_id, amount, currency, order_count) \
         SELECT $1, round(sum(tp.amount) * (10000 - fee.bps)::numeric / 10000, 2), tp.currency, count(*) \
         FROM to_pay tp, fee GROUP BY tp.currency, fee.bps \
         RETURNING id, organizer_id, amount, currency, order_count, status, created_at",
    )
    .bind(organizer_id)
    .fetch_all(&mut *tx)
    .await?;
    let count = payouts.len();
    crate::audit::log(
        &mut *tx,
        admin.id,
        "payout_create",
        "organizer",
        Some(organizer_id),
        serde_json::json!({ "settlements": count }),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(payouts))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AdminPayout {
    pub id: Uuid,
    pub organizer_id: Uuid,
    pub organizer_email: Option<String>,
    pub amount: Decimal,
    pub currency: String,
    pub order_count: i32,
    pub status: String,
    pub failure_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// `GET /api/admin/payouts?status=` — settlement history (newest first), optional status filter.
pub async fn list_payouts(
    _admin: CurrentAdmin,
    State(st): State<AppState>,
    axum::extract::Query(q): axum::extract::Query<PayoutQuery>,
) -> Result<Json<Vec<AdminPayout>>, AppError> {
    let payouts: Vec<AdminPayout> = sqlx::query_as(
        "SELECT p.id, p.organizer_id, u.email AS organizer_email, p.amount, p.currency, \
            p.order_count, p.status, p.failure_reason, p.created_at \
         FROM payouts p LEFT JOIN users u ON u.id = p.organizer_id \
         WHERE ($1::text IS NULL OR p.status = $1) \
         ORDER BY p.created_at DESC LIMIT 200",
    )
    .bind(q.status.as_deref().filter(|s| !s.is_empty()))
    .fetch_all(&st.db)
    .await?;
    Ok(Json(payouts))
}

#[derive(Deserialize)]
pub struct PayoutQuery {
    pub status: Option<String>,
}

const PAYOUT_STATUSES: [&str; 3] = ["pending", "paid", "failed"];

#[derive(Deserialize)]
pub struct PayoutStatusReq {
    pub status: String,
    #[serde(default)]
    pub failure_reason: Option<String>,
}

/// `PATCH /api/admin/payouts/:id` — manual settlement workflow: mark a payout paid / failed
/// (with a reason) / pending (retry). Logged to the audit trail.
pub async fn update_payout(
    admin: CurrentAdmin,
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<PayoutStatusReq>,
) -> Result<StatusCode, AppError> {
    if !PAYOUT_STATUSES.contains(&req.status.as_str()) {
        return Err(AppError::Validation("status must be pending|paid|failed".into()));
    }
    // Only a 'failed' payout carries a reason; clear it otherwise.
    let reason = if req.status == "failed" {
        req.failure_reason.as_deref()
    } else {
        None
    };
    let mut tx = st.db.begin().await?;
    let res = sqlx::query("UPDATE payouts SET status = $2, failure_reason = $3 WHERE id = $1")
        .bind(id)
        .bind(&req.status)
        .bind(reason)
        .execute(&mut *tx)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    crate::audit::log(
        &mut *tx,
        admin.id,
        "payout_status",
        "payout",
        Some(id),
        serde_json::json!({ "status": req.status, "failure_reason": reason }),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

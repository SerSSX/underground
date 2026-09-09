//! Platform admin APIs: dashboard, users, events moderation, orders/escrow (TASK-014, Фаза 4).

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::audit;
use crate::auth::CurrentAdmin;
use crate::error::AppError;
use crate::state::AppState;

#[derive(Serialize, sqlx::FromRow)]
pub struct AdminStats {
    pub users_count: i64,
    pub events_count: i64,
    pub tickets_count: i64,
    pub gmv: Decimal,
    pub platform_fee: Decimal,
}

/// `GET /api/admin/stats` — platform dashboard numbers.
pub async fn stats(_admin: CurrentAdmin, State(st): State<AppState>) -> Result<Json<AdminStats>, AppError> {
    let stats: AdminStats = sqlx::query_as(
        "SELECT \
            (SELECT count(*) FROM users) AS users_count, \
            (SELECT count(*) FROM events) AS events_count, \
            (SELECT count(*) FROM tickets WHERE status IN ('active','used')) AS tickets_count, \
            COALESCE((SELECT sum(amount) FROM orders WHERE status = 'paid'), 0) AS gmv, \
            round(COALESCE((SELECT sum(amount) FROM orders WHERE status = 'paid'), 0) \
                * (SELECT platform_fee_bps FROM payment_settings LIMIT 1) / 10000.0, 2) AS platform_fee",
    )
    .fetch_one(&st.db)
    .await?;
    Ok(Json(stats))
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub q: Option<String>,
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

fn page(q: &ListQuery) -> (i64, i64) {
    (q.limit.unwrap_or(50).clamp(1, 200), q.offset.unwrap_or(0).max(0))
}

/// Escape LIKE/ILIKE metacharacters so an admin search matches the term literally
/// (Postgres LIKE treats `\` as the default escape char) — no wildcard enumeration.
fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AdminUser {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub role: String,
    pub blocked: bool,
    pub verified: bool,
    pub kyc_status: String,
    /// Read-only mirror of the payment provider's view of this organiser's account. Shown so an
    /// operator can tell "we have not verified them" apart from "the provider has not enabled
    /// them" — two different problems with two different fixes. Never writable from here.
    pub psp_connected: bool,
    pub psp_charges_enabled: bool,
    pub psp_payouts_enabled: bool,
    pub created_at: DateTime<Utc>,
}

/// `GET /api/admin/users?q=&limit=&offset=`
pub async fn users(
    _admin: CurrentAdmin,
    State(st): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<AdminUser>>, AppError> {
    let (limit, offset) = page(&query);
    let users: Vec<AdminUser> = sqlx::query_as(
        "SELECT id, email, name, role, blocked, verified, kyc_status, \
            psp_account_id IS NOT NULL AS psp_connected, psp_charges_enabled, psp_payouts_enabled, created_at \
         FROM users \
         WHERE ($1::text IS NULL OR email ILIKE '%' || $1 || '%' OR name ILIKE '%' || $1 || '%') \
         ORDER BY created_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(
        query
            .q
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(escape_like),
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(users))
}

#[derive(Deserialize)]
pub struct PatchUserReq {
    pub blocked: Option<bool>,
    pub role: Option<String>,
    /// Organizer verification badge (trust & safety).
    pub verified: Option<bool>,
}

/// `PATCH /api/admin/users/:id` — block/unblock, change role, verify organizer.
pub async fn patch_user(
    admin: CurrentAdmin,
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<PatchUserReq>,
) -> Result<Json<AdminUser>, AppError> {
    if req.role.as_deref().is_some_and(|r| r != "USER" && r != "ADMIN") {
        return Err(AppError::Validation("role must be USER|ADMIN".into()));
    }
    let mut tx = st.db.begin().await?;
    let user: Option<AdminUser> = sqlx::query_as(
        "UPDATE users SET blocked = COALESCE($2, blocked), role = COALESCE($3, role), \
            verified = COALESCE($4, verified) \
         WHERE id = $1 \
         RETURNING id, email, name, role, blocked, verified, kyc_status, \
             psp_account_id IS NOT NULL AS psp_connected, psp_charges_enabled, psp_payouts_enabled, created_at",
    )
    .bind(id)
    .bind(req.blocked)
    .bind(req.role.as_deref())
    .bind(req.verified)
    .fetch_optional(&mut *tx)
    .await?;
    let user = user.ok_or(AppError::NotFound)?;
    let detail = serde_json::json!({ "blocked": req.blocked, "role": req.role, "verified": req.verified });
    audit::log(&mut *tx, admin.id, "user_update", "user", Some(id), detail).await?;
    tx.commit().await?;
    Ok(Json(user))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AdminEvent {
    pub id: Uuid,
    pub title: String,
    pub status: String,
    pub category: String,
    pub city: Option<String>,
    pub organizer_email: Option<String>,
    pub starts_at: Option<DateTime<Utc>>,
    pub tickets_left: i32,
    pub sales_closed: bool,
}

/// `GET /api/admin/events?status=&q=`
pub async fn events(
    _admin: CurrentAdmin,
    State(st): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<AdminEvent>>, AppError> {
    let (limit, offset) = page(&query);
    let events: Vec<AdminEvent> = sqlx::query_as(
        "SELECT e.id, e.title, e.status, e.category, e.city, u.email AS organizer_email, \
            e.starts_at, e.tickets_left, e.sales_closed \
         FROM events e LEFT JOIN users u ON u.id = e.organizer_id \
         WHERE ($1::text IS NULL OR e.status = $1) \
           AND ($2::text IS NULL OR e.title ILIKE '%' || $2 || '%') \
         ORDER BY e.created_at DESC LIMIT $3 OFFSET $4",
    )
    .bind(query.status.as_deref().filter(|s| !s.is_empty()))
    .bind(
        query
            .q
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(escape_like),
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(events))
}

#[derive(Deserialize)]
pub struct PatchEventReq {
    pub status: String,
}

/// `PATCH /api/admin/events/:id` — moderation: published | hidden | blocked.
pub async fn patch_event(
    admin: CurrentAdmin,
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<PatchEventReq>,
) -> Result<Json<AdminEvent>, AppError> {
    if !["published", "hidden", "blocked"].contains(&req.status.as_str()) {
        return Err(AppError::Validation("status must be published|hidden|blocked".into()));
    }
    let mut tx = st.db.begin().await?;
    let event: Option<AdminEvent> = sqlx::query_as(
        "WITH updated AS (UPDATE events SET status = $2 WHERE id = $1 RETURNING *) \
         SELECT e.id, e.title, e.status, e.category, e.city, u.email AS organizer_email, \
            e.starts_at, e.tickets_left, e.sales_closed \
         FROM updated e LEFT JOIN users u ON u.id = e.organizer_id",
    )
    .bind(id)
    .bind(&req.status)
    .fetch_optional(&mut *tx)
    .await?;
    let event = event.ok_or(AppError::NotFound)?;
    audit::log(
        &mut *tx,
        admin.id,
        "event_moderate",
        "event",
        Some(id),
        serde_json::json!({ "status": req.status }),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(event))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AdminOrder {
    pub id: Uuid,
    pub event_title: Option<String>,
    pub buyer_email: Option<String>,
    pub amount: Decimal,
    pub currency: String,
    pub status: String,
    pub escrow_status: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// `GET /api/admin/orders?status=`
pub async fn orders(
    _admin: CurrentAdmin,
    State(st): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<AdminOrder>>, AppError> {
    let (limit, offset) = page(&query);
    let orders: Vec<AdminOrder> = sqlx::query_as(
        "SELECT o.id, e.title AS event_title, u.email AS buyer_email, o.amount, o.currency, \
            o.status, o.escrow_status, o.created_at \
         FROM orders o \
         LEFT JOIN events e ON e.id = o.event_id \
         LEFT JOIN users u ON u.id = o.buyer_id \
         WHERE ($1::text IS NULL OR o.status = $1) \
         ORDER BY o.created_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(query.status.as_deref().filter(|s| !s.is_empty()))
    .bind(limit)
    .bind(offset)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(orders))
}

#[derive(Deserialize)]
pub struct EscrowActionReq {
    pub action: String, // refund | freeze | release
    /// `release` only clears a frozen (disputed) order when `force = true` — the explicit
    /// admin override that the dispute is resolved in the organizer's favour.
    #[serde(default)]
    pub force: Option<bool>,
}

/// An order row locked for escrow control: (escrow_status, ticket_id, event_id, promo_code).
type EscrowOrderRow = (
    Option<String>,
    Option<Uuid>,
    Uuid,
    Option<String>,
    String,
    Option<chrono::DateTime<chrono::Utc>>,
    rust_decimal::Decimal,
    String,
);

/// `POST /api/admin/orders/:id/escrow` — manual escrow control.
/// refund: money back, ticket returned, seat freed. freeze: dispute hold.
/// release: held|clearing → cleared; a frozen order needs `force` (dispute override).
pub async fn escrow_action(
    admin: CurrentAdmin,
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<EscrowActionReq>,
) -> Result<Json<AdminOrder>, AppError> {
    let mut tx = st.db.begin().await?;
    let row: Option<EscrowOrderRow> =
        sqlx::query_as("SELECT escrow_status, ticket_id, event_id, promo_code, status, hold_expires_at, amount, currency FROM orders WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    let (escrow, ticket_id, event_id, promo_code, order_status, hold_expires_at, amount, currency) =
        row.ok_or(AppError::NotFound)?;
    if escrow.as_deref() == Some("paid_out") {
        return Err(AppError::Conflict("кошти вже виплачені організатору".into()));
    }
    let new_escrow = match req.action.as_str() {
        "refund" => {
            // One routine, shared with the provider's full-refund notice, hands back whatever
            // the order holds — a ticket and its seat, or a pending hold's seat (REVIEW-3 #2).
            crate::orders::return_holdings(
                &mut tx,
                event_id,
                ticket_id,
                promo_code.as_deref(),
                &order_status,
                hold_expires_at,
            )
            .await?;
            // Was money ever taken? 'paid' = yes; 'refund_due' = yes, and its return is already
            // queued (the insert below is a no-op). pending/expired/failed = no money yet.
            let money_taken = matches!(order_status.as_str(), "paid" | "refund_due");
            if money_taken {
                // The return is a debt on record, drained by the refund executor — not a status
                // flip that nobody acts on.
                crate::orders::queue_refund(&mut tx, id, amount, &currency, "повернення адміністратором").await?;
            }
            // The marker `fulfill` reads if this order's payment lands AFTER the admin closed it:
            // 'cancelled' → the money goes back (nothing was taken yet, so it must be returned
            // now); 'refunded' → already returned, never mint and never pay twice (REVIEW-3 #1).
            let marker = if money_taken { "refunded" } else { "cancelled" };
            sqlx::query(
                "UPDATE orders SET status = 'refunded', terminal_event_seen = COALESCE(terminal_event_seen, $2) WHERE id = $1",
            )
            .bind(id)
            .bind(marker)
            .execute(&mut *tx)
            .await?;
            // Where money was actually taken the escrow is `refund_due`, NOT `refunded`: a job
            // is queued and the worker has not moved anything yet. Claiming `refunded` here is
            // how the admin list came to say "returned" about money still sitting at the
            // provider. The executor flips it once the provider confirms.
            if money_taken {
                "refund_due"
            } else {
                "refunded"
            }
        }
        "freeze" => "frozen",
        // held|clearing clear normally; a frozen (disputed) order needs an explicit force.
        "release" => match escrow.as_deref() {
            Some("held") | Some("clearing") => "cleared",
            Some("frozen") if req.force.unwrap_or(false) => "cleared",
            _ => {
                return Err(AppError::Conflict(
                    "release дозволено лише для held|clearing; спірне замовлення потребує force".into(),
                ))
            }
        },
        _ => return Err(AppError::Validation("action must be refund|freeze|release".into())),
    };
    sqlx::query("UPDATE orders SET escrow_status = $2 WHERE id = $1")
        .bind(id)
        .bind(new_escrow)
        .execute(&mut *tx)
        .await?;
    // Audit row shares the escrow transaction → the change and its log commit atomically.
    let detail = serde_json::json!({ "action": req.action, "force": req.force, "new_escrow": new_escrow });
    audit::log(&mut *tx, admin.id, "escrow_action", "order", Some(id), detail).await?;
    tx.commit().await?;

    let order: AdminOrder = sqlx::query_as(
        "SELECT o.id, e.title AS event_title, u.email AS buyer_email, o.amount, o.currency, \
            o.status, o.escrow_status, o.created_at \
         FROM orders o \
         LEFT JOIN events e ON e.id = o.event_id \
         LEFT JOIN users u ON u.id = o.buyer_id \
         WHERE o.id = $1",
    )
    .bind(id)
    .fetch_one(&st.db)
    .await?;
    Ok(Json(order))
}

/// `DELETE /api/admin/posts/:id` — moderation removal of any post (cascades likes/comments).
///
/// Idempotent: a second delete (or resolving a report whose target is already gone)
/// returns 204, not 404, so the moderation queue can enforce without spurious errors.
pub async fn delete_post(
    admin: CurrentAdmin,
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let mut tx = st.db.begin().await?;
    sqlx::query("DELETE FROM posts WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    audit::log(
        &mut *tx,
        admin.id,
        "post_delete",
        "post",
        Some(id),
        serde_json::json!({}),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /api/admin/reviews/:id` — moderation removal of any review.
///
/// Idempotent (see [`delete_post`]): re-deleting an already-removed review returns 204.
pub async fn delete_review(
    admin: CurrentAdmin,
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let mut tx = st.db.begin().await?;
    sqlx::query("DELETE FROM reviews WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    audit::log(
        &mut *tx,
        admin.id,
        "review_delete",
        "review",
        Some(id),
        serde_json::json!({}),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

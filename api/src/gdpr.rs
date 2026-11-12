//! GDPR self-serve: account erasure (Art. 17) and data export (Art. 20). Erasure
//! anonymizes the user row and deletes personal content, but RETAINS financial records
//! (orders/tickets) — de-linked from PII — for tax/audit retention.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::auth::{verify_password, CurrentUser};
use crate::error::AppError;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct DeleteMeReq {
    /// Re-authentication for password accounts: erasure is destructive, so the password is
    /// required.
    pub password: Option<String>,
    /// Password-less accounts (email code, Google) have nothing to re-enter; they confirm with
    /// the literal `"DELETE"` instead (TASK-302 P15).
    pub confirm: Option<String>,
}

/// How recent a password-less user's sign-in must be to erase the account.
const RECENT_LOGIN_SECS: i64 = 10 * 60;

/// `DELETE /api/me` — GDPR Art. 17 erasure.
pub async fn delete_me(
    user: CurrentUser,
    State(st): State<AppState>,
    Json(req): Json<DeleteMeReq>,
) -> Result<StatusCode, AppError> {
    let hash: Option<String> = sqlx::query_scalar("SELECT password_hash FROM users WHERE id = $1 AND blocked = false")
        .bind(user.id)
        .fetch_one(&st.db)
        .await?;
    match hash {
        Some(hash) => {
            let password = req
                .password
                .ok_or_else(|| AppError::Validation("password is required".into()))?;
            if !verify_password(&password, &hash) {
                return Err(AppError::Unauthorized);
            }
        }
        None => {
            if req.confirm.as_deref() != Some("DELETE") {
                return Err(AppError::Validation(
                    "підтвердіть видалення: {\"confirm\": \"DELETE\"}".into(),
                ));
            }
            // Step-up (TASK-302 fix batch 1, item 2): with no password to re-enter, the sign-in
            // itself must be recent, so a week-old stolen token cannot erase the account.
            if Utc::now().timestamp() - user.iat > RECENT_LOGIN_SECS {
                return Err(AppError::ReauthRequired("увійдіть ще раз, щоб видалити акаунт".into()));
            }
        }
    }

    let mut tx = st.db.begin().await?;
    // Delete personal content (the user's own contributions / social graph / device data).
    for sql in [
        "DELETE FROM posts WHERE author_id = $1",
        "DELETE FROM post_likes WHERE user_id = $1",
        "DELETE FROM post_comments WHERE author_id = $1",
        "DELETE FROM reviews WHERE author_id = $1",
        "DELETE FROM messages WHERE sender_id = $1 OR recipient_id = $1",
        "DELETE FROM follows WHERE follower_id = $1 OR followee_id = $1",
        "DELETE FROM event_saves WHERE user_id = $1",
        "DELETE FROM device_tokens WHERE user_id = $1",
        "DELETE FROM notifications WHERE user_id = $1",
        "DELETE FROM event_reminders WHERE user_id = $1",
        "DELETE FROM telemetry_events WHERE user_id = $1",
    ] {
        sqlx::query(sql).bind(user.id).execute(&mut *tx).await?;
    }
    // P16: the account's free places go back to their events (the account can no longer fetch a
    // QR). Outstanding gifts it sent are revoked first, so those `pending_claim` places return
    // too. Free events only (by `events.cost_type`): paid tickets are a refund decision.
    sqlx::query(
        "UPDATE ticket_gifts g SET status = 'revoked' \
         FROM tickets t JOIN events e ON e.id = t.event_id \
         WHERE g.ticket_id = t.id AND g.status = 'pending' AND t.owner_id = $1 AND e.cost_type = 'free'",
    )
    .bind(user.id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "WITH r AS ( \
             UPDATE tickets t SET status = 'returned' FROM events e \
             WHERE e.id = t.event_id AND t.owner_id = $1 AND e.cost_type = 'free' \
               AND t.status IN ('active', 'pending_claim') \
             RETURNING t.event_id) \
         UPDATE events e SET tickets_left = e.tickets_left + c.n \
         FROM (SELECT event_id, count(*)::int AS n FROM r GROUP BY event_id) c \
         WHERE e.id = c.event_id",
    )
    .bind(user.id)
    .execute(&mut *tx)
    .await?;
    // Anonymize the account row; block it so the credential can no longer authenticate.
    // Clears ALL identifying / financial PII: email, name, bio, avatar, dob, password,
    // Google identity (re-identification risk), and KYC bank details.
    sqlx::query(
        "UPDATE users SET email = 'deleted+' || id || '@deleted.invalid', name = 'Видалений користувач', \
            bio = NULL, avatar_url = NULL, dob = NULL, password_hash = NULL, google_sub = NULL, \
            payout_name = NULL, payout_iban = NULL, payout_swift = NULL, kyc_status = 'none', \
            is_organizer = false, verified = false, blocked = true, deleted_at = now() WHERE id = $1",
    )
    .bind(user.id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, sqlx::FromRow)]
struct ExportTicket {
    id: Uuid,
    event_id: Uuid,
    status: String,
    price: Decimal,
    created_at: DateTime<Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
struct ExportOrder {
    id: Uuid,
    event_id: Uuid,
    amount: Decimal,
    currency: String,
    status: String,
    created_at: DateTime<Utc>,
}

/// `GET /api/me/export` — GDPR Art. 20 portability: the caller's own data as JSON.
pub async fn export_me(user: CurrentUser, State(st): State<AppState>) -> Result<Json<Value>, AppError> {
    let profile = crate::users::find_by_id(&st.db, user.id)
        .await?
        .ok_or(AppError::NotFound)?;
    let events = crate::events::fetch_by_organizer(&st.db, user.id).await?;
    let tickets: Vec<ExportTicket> = sqlx::query_as(
        "SELECT id, event_id, status, price, created_at FROM tickets WHERE owner_id = $1 ORDER BY created_at DESC",
    )
    .bind(user.id)
    .fetch_all(&st.db)
    .await?;
    let orders: Vec<ExportOrder> = sqlx::query_as(
        "SELECT id, event_id, amount, currency, status, created_at FROM orders WHERE buyer_id = $1 \
         ORDER BY created_at DESC",
    )
    .bind(user.id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(json!({
        "profile": profile,
        "events": events,
        "tickets": tickets,
        "orders": orders,
    })))
}

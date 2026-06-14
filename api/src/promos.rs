//! Organizer promo codes (TASK-017 M8). An organizer issues percentage-off codes for
//! their own event; buyers apply them at checkout (`?promo=CODE`).

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::state::AppState;

#[derive(Serialize, sqlx::FromRow)]
pub struct PromoCode {
    pub id: Uuid,
    pub event_id: Uuid,
    pub code: String,
    pub percent_off: i32,
    pub max_uses: Option<i32>,
    pub used_count: i32,
    pub expires_at: Option<DateTime<Utc>>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

async fn require_event_owner(db: &PgPool, event_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
    let organizer: Option<Option<Uuid>> = sqlx::query_scalar("SELECT organizer_id FROM events WHERE id = $1")
        .bind(event_id)
        .fetch_optional(db)
        .await?;
    match organizer {
        None => Err(AppError::NotFound),
        Some(o) if o == Some(user_id) => Ok(()),
        Some(_) => Err(AppError::Forbidden),
    }
}

#[derive(Deserialize)]
pub struct CreatePromoReq {
    pub code: String,
    pub percent_off: i32,
    pub max_uses: Option<i32>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// `POST /api/events/:id/promos` — owner creates a code for their event.
pub async fn create_promo(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(event_id): Path<Uuid>,
    Json(req): Json<CreatePromoReq>,
) -> Result<(StatusCode, Json<PromoCode>), AppError> {
    require_event_owner(&st.db, event_id, user.id).await?;
    let code = req.code.trim();
    if code.is_empty() || code.len() > 40 {
        return Err(AppError::Validation("code is required (max 40 chars)".into()));
    }
    if req.percent_off < 1 || req.percent_off > 100 {
        return Err(AppError::Validation("percent_off must be 1..100".into()));
    }
    if req.max_uses.is_some_and(|m| m < 1) {
        return Err(AppError::Validation("max_uses must be >= 1".into()));
    }
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM promo_codes WHERE event_id = $1 AND lower(code) = lower($2))")
            .bind(event_id)
            .bind(code)
            .fetch_one(&st.db)
            .await?;
    if exists {
        return Err(AppError::Conflict("такий код уже існує".into()));
    }
    let promo: PromoCode = sqlx::query_as(
        "INSERT INTO promo_codes (event_id, code, percent_off, max_uses, expires_at) \
         VALUES ($1, $2, $3, $4, $5) \
         RETURNING id, event_id, code, percent_off, max_uses, used_count, expires_at, active, created_at",
    )
    .bind(event_id)
    .bind(code)
    .bind(req.percent_off)
    .bind(req.max_uses)
    .bind(req.expires_at)
    .fetch_one(&st.db)
    .await?;
    Ok((StatusCode::CREATED, Json(promo)))
}

/// `GET /api/events/:id/promos` — owner lists their event's codes.
pub async fn list_promos(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(event_id): Path<Uuid>,
) -> Result<Json<Vec<PromoCode>>, AppError> {
    require_event_owner(&st.db, event_id, user.id).await?;
    let promos: Vec<PromoCode> = sqlx::query_as(
        "SELECT id, event_id, code, percent_off, max_uses, used_count, expires_at, active, created_at \
         FROM promo_codes WHERE event_id = $1 ORDER BY created_at DESC",
    )
    .bind(event_id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(promos))
}

/// `DELETE /api/promos/:id` — owner deactivates a code.
pub async fn delete_promo(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let deleted = sqlx::query(
        "DELETE FROM promo_codes p USING events e \
         WHERE p.id = $1 AND e.id = p.event_id AND e.organizer_id = $2",
    )
    .bind(id)
    .bind(user.id)
    .execute(&st.db)
    .await?;
    if deleted.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

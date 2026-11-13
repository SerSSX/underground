//! Event reviews: 1..5 rating + optional text, one per user per event (F-13).

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::state::AppState;

/// Review text cap, in characters (TASK-305 F10).
const MAX_TEXT_LEN: usize = 2000;

/// SQL (reviews aliased `r`): the review counts — its author still holds an active or used place
/// at that event (TASK-305 R4a). A review whose place was returned, cancelled or handed on stays
/// stored but leaves the event's list, its average and the organiser's rating.
pub const COUNTED_REVIEW_SQL: &str = "EXISTS (SELECT 1 FROM tickets ct \
    WHERE ct.event_id = r.event_id AND ct.owner_id = r.author_id AND ct.status IN ('active', 'used'))";

#[derive(Serialize, sqlx::FromRow)]
pub struct Review {
    pub id: Uuid,
    pub event_id: Uuid,
    pub author_id: Uuid,
    pub author_name: String,
    pub rating: i32,
    pub text: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct EventReviews {
    pub avg_rating: Option<f64>,
    pub count: i64,
    pub reviews: Vec<Review>,
}

#[derive(Deserialize)]
pub struct PostReviewReq {
    pub event_id: Uuid,
    pub rating: i32,
    pub text: Option<String>,
}

/// Only a guest reviews (TASK-305 F9, orchestrator reading in docs/decisions.md): the caller holds
/// a place at this event that was not returned or cancelled, and is not its organiser.
async fn ensure_guest(
    st: &AppState,
    event_id: Uuid,
    organizer_id: Option<Uuid>,
    user_id: Uuid,
) -> Result<(), AppError> {
    if organizer_id == Some(user_id) {
        return Err(AppError::Validation(
            "організатор не може оцінювати власну подію".into(),
        ));
    }
    let guest: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM tickets WHERE event_id = $1 AND owner_id = $2 \
            AND status IN ('active', 'used', 'pending_claim'))",
    )
    .bind(event_id)
    .bind(user_id)
    .fetch_one(&st.db)
    .await?;
    if !guest {
        return Err(AppError::Validation("відгук може залишити лише гість події".into()));
    }
    Ok(())
}

/// `POST /api/reviews` — create or update (upsert) the caller's review of an event.
pub async fn post_review(
    user: CurrentUser,
    State(st): State<AppState>,
    Json(req): Json<PostReviewReq>,
) -> Result<(StatusCode, Json<Review>), AppError> {
    if !(1..=5).contains(&req.rating) {
        return Err(AppError::Validation("rating must be between 1 and 5".into()));
    }
    // Controls and bidi controls out, newlines kept; nothing visible means no text (TASK-305 R5).
    let text = req.text.as_deref().map(crate::text::clean_multiline);
    let text = text.as_deref().filter(|t| !crate::text::is_blank(t));
    if text.is_some_and(|t| t.chars().count() > MAX_TEXT_LEN) {
        return Err(AppError::Validation(format!(
            "text must be at most {MAX_TEXT_LEN} chars"
        )));
    }
    let row: Option<(Option<DateTime<Utc>>, Option<Uuid>)> =
        sqlx::query_as("SELECT starts_at, organizer_id FROM events WHERE id = $1 AND status = 'published'")
            .bind(req.event_id)
            .fetch_optional(&st.db)
            .await?;
    let (starts_at, organizer_id) = row.ok_or(AppError::NotFound)?;
    // Reviews open once the event has started — a rating you cannot fake ahead of time. An event
    // without a start date never opens them (TASK-305 R4b).
    let Some(starts_at) = starts_at else {
        return Err(AppError::Validation(
            "у події немає дати початку — відгуки недоступні".into(),
        ));
    };
    if starts_at > Utc::now() {
        return Err(AppError::Validation("відгуки відкриються після початку події".into()));
    }
    ensure_guest(&st, req.event_id, organizer_id, user.id).await?;
    let review: Review = sqlx::query_as(concat!(
        "WITH upserted AS ( \
            INSERT INTO reviews (event_id, author_id, rating, text) VALUES ($1, $2, $3, $4) \
            ON CONFLICT (event_id, author_id) DO UPDATE SET rating = $3, text = $4, created_at = now() \
            RETURNING id, event_id, author_id, rating, text, created_at \
         ) \
         SELECT r.id, r.event_id, r.author_id, \
            ",
        crate::display_name_sql!(),
        " AS author_name, \
            r.rating, r.text, r.created_at \
         FROM upserted r JOIN users u ON u.id = r.author_id"
    ))
    .bind(req.event_id)
    .bind(user.id)
    .bind(req.rating)
    .bind(text)
    .fetch_one(&st.db)
    .await?;
    // The review also lands on the author's wall (replacing an older review post).
    if let Err(e) = crate::posts::insert_auto_post(
        &st.db,
        user.id,
        "review",
        review.text.as_deref(),
        Some(review.event_id),
        Some(review.rating),
    )
    .await
    {
        tracing::warn!(error = %e, review_id = %review.id, "review wall post failed");
    }
    Ok((StatusCode::CREATED, Json(review)))
}

/// `GET /api/events/:id/reviews` — newest first, with the rating aggregate (public).
pub async fn event_reviews(
    State(st): State<AppState>,
    Path(event_id): Path<Uuid>,
) -> Result<Json<EventReviews>, AppError> {
    let sql = format!(
        "SELECT r.id, r.event_id, r.author_id, {name} AS author_name, r.rating, r.text, r.created_at \
         FROM reviews r JOIN users u ON u.id = r.author_id \
         WHERE r.event_id = $1 AND {counted} ORDER BY r.created_at DESC",
        name = crate::users::DISPLAY_NAME_SQL,
        counted = COUNTED_REVIEW_SQL,
    );
    let reviews: Vec<Review> = sqlx::query_as(&sql).bind(event_id).fetch_all(&st.db).await?;
    let count = reviews.len() as i64;
    let avg_rating = (count > 0).then(|| reviews.iter().map(|r| f64::from(r.rating)).sum::<f64>() / count as f64);
    Ok(Json(EventReviews {
        avg_rating,
        count,
        reviews,
    }))
}

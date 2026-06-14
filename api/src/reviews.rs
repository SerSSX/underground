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

const MAX_TEXT_LEN: usize = 2000;

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

/// `POST /api/reviews` — create or update (upsert) the caller's review of an event.
pub async fn post_review(
    user: CurrentUser,
    State(st): State<AppState>,
    Json(req): Json<PostReviewReq>,
) -> Result<(StatusCode, Json<Review>), AppError> {
    if !(1..=5).contains(&req.rating) {
        return Err(AppError::Validation("rating must be between 1 and 5".into()));
    }
    let text = req.text.as_deref().map(str::trim).filter(|t| !t.is_empty());
    if text.is_some_and(|t| t.len() > MAX_TEXT_LEN) {
        return Err(AppError::Validation(format!(
            "text must be at most {MAX_TEXT_LEN} chars"
        )));
    }
    let row: Option<Option<chrono::DateTime<chrono::Utc>>> =
        sqlx::query_scalar("SELECT starts_at FROM events WHERE id = $1 AND status = 'published'")
            .bind(req.event_id)
            .fetch_optional(&st.db)
            .await?;
    let starts_at = row.ok_or(AppError::NotFound)?;
    // Reviews open once the event has started — a rating you cannot fake ahead of time.
    if starts_at.is_some_and(|t| t > chrono::Utc::now()) {
        return Err(AppError::Validation("відгуки відкриються після початку події".into()));
    }
    let review: Review = sqlx::query_as(
        "WITH upserted AS ( \
            INSERT INTO reviews (event_id, author_id, rating, text) VALUES ($1, $2, $3, $4) \
            ON CONFLICT (event_id, author_id) DO UPDATE SET rating = $3, text = $4, created_at = now() \
            RETURNING id, event_id, author_id, rating, text, created_at \
         ) \
         SELECT r.id, r.event_id, r.author_id, u.name AS author_name, r.rating, r.text, r.created_at \
         FROM upserted r JOIN users u ON u.id = r.author_id",
    )
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
    let reviews: Vec<Review> = sqlx::query_as(
        "SELECT r.id, r.event_id, r.author_id, u.name AS author_name, r.rating, r.text, r.created_at \
         FROM reviews r JOIN users u ON u.id = r.author_id \
         WHERE r.event_id = $1 ORDER BY r.created_at DESC",
    )
    .bind(event_id)
    .fetch_all(&st.db)
    .await?;
    let count = reviews.len() as i64;
    let avg_rating = (count > 0).then(|| reviews.iter().map(|r| f64::from(r.rating)).sum::<f64>() / count as f64);
    Ok(Json(EventReviews {
        avg_rating,
        count,
        reviews,
    }))
}

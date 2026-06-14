//! Event social proof + similar events (TASK-017 M3).

use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::events::{Event, EVENT_COLS};
use crate::social::UserBrief;
use crate::state::AppState;

#[derive(Serialize)]
pub struct EventSocial {
    /// People who saved/are interested in the event.
    pub interested_count: i64,
    /// People with an active or used ticket (i.e. going).
    pub going_count: i64,
    /// Followed users who are interested (empty for anonymous callers).
    pub friends_interested: Vec<UserBrief>,
    /// Followed users who are going.
    pub friends_going: Vec<UserBrief>,
}

/// `GET /api/events/:id/social` — counts + which of your friends are interested/going.
pub async fn social(
    viewer: Option<CurrentUser>,
    State(st): State<AppState>,
    Path(event_id): Path<Uuid>,
) -> Result<Json<EventSocial>, AppError> {
    let viewer_id = viewer.map(|v| v.id);

    let interested_count: i64 = sqlx::query_scalar("SELECT count(*) FROM event_saves WHERE event_id = $1")
        .bind(event_id)
        .fetch_one(&st.db)
        .await?;
    let going_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM tickets WHERE event_id = $1 AND status IN ('active','used')")
            .bind(event_id)
            .fetch_one(&st.db)
            .await?;

    let friends_interested: Vec<UserBrief> = sqlx::query_as(
        "SELECT u.id, u.name, u.avatar_url FROM event_saves s \
         JOIN follows f ON f.followee_id = s.user_id AND f.follower_id = $2 \
         JOIN users u ON u.id = s.user_id AND u.blocked = false \
         WHERE s.event_id = $1 ORDER BY u.name LIMIT 12",
    )
    .bind(event_id)
    .bind(viewer_id)
    .fetch_all(&st.db)
    .await?;

    let friends_going: Vec<UserBrief> = sqlx::query_as(
        "SELECT DISTINCT u.id, u.name, u.avatar_url FROM tickets t \
         JOIN follows f ON f.followee_id = t.owner_id AND f.follower_id = $2 \
         JOIN users u ON u.id = t.owner_id AND u.blocked = false \
         WHERE t.event_id = $1 AND t.status IN ('active','used') ORDER BY u.name LIMIT 12",
    )
    .bind(event_id)
    .bind(viewer_id)
    .fetch_all(&st.db)
    .await?;

    Ok(Json(EventSocial {
        interested_count,
        going_count,
        friends_interested,
        friends_going,
    }))
}

/// `GET /api/events/:id/similar` — upcoming events in the same category or city (public).
pub async fn similar(State(st): State<AppState>, Path(event_id): Path<Uuid>) -> Result<Json<Vec<Event>>, AppError> {
    let cols = EVENT_COLS
        .split(", ")
        .map(|c| format!("e.{c}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "WITH src AS (SELECT category, city FROM events WHERE id = $1) \
         SELECT {cols} FROM events e, src \
         WHERE e.id <> $1 AND e.status = 'published' \
           AND (e.starts_at IS NULL OR e.starts_at >= now()) \
           AND (e.category = src.category OR e.city = src.city) \
         ORDER BY ( \
              (CASE WHEN e.category = src.category THEN 2 ELSE 0 END) \
            + (CASE WHEN e.city = src.city THEN 1 ELSE 0 END) \
            + (SELECT count(*) FROM tickets t WHERE t.event_id = e.id) * 0.3 \
         ) DESC, e.starts_at ASC NULLS LAST \
         LIMIT 8"
    );
    let events: Vec<Event> = sqlx::query_as(&sql).bind(event_id).fetch_all(&st.db).await?;
    Ok(Json(events))
}

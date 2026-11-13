//! Social graph: public profiles + follows (F-11).

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::events::{self, Event};
use crate::notifications;
use crate::state::AppState;

/// Public profile shape — no email (privacy), unlike `users::User` returned to the owner.
#[derive(Serialize, sqlx::FromRow)]
pub struct PublicProfile {
    pub id: Uuid,
    pub name: String,
    pub role: String,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
    pub is_organizer: bool,
    /// Verified organizer badge (TASK-017 M6, admin-granted).
    pub verified: bool,
    pub followers_count: i64,
    pub following_count: i64,
    pub events_count: i64,
    /// Average review rating across this user's events (F-13), if any.
    pub rating_avg: Option<f64>,
    pub rating_count: i64,
    /// Whether the (optional) caller follows this user; `false` for anonymous callers.
    pub is_following: bool,
}

/// The profile as anyone else sees it. TASK-305: the name is the display name (a placeholder that
/// repeats the email shows as «Гість»), and reviews of cancelled events do not count (F9).
async fn fetch_profile(db: &PgPool, id: Uuid, viewer: Option<Uuid>) -> Result<Option<PublicProfile>, sqlx::Error> {
    let sql = format!(
        "SELECT u.id, {name} AS name, u.role, u.avatar_url, u.bio, u.is_organizer, u.verified, \
            (SELECT count(*) FROM follows WHERE followee_id = u.id) AS followers_count, \
            (SELECT count(*) FROM follows WHERE follower_id = u.id) AS following_count, \
            (SELECT count(*) FROM events WHERE organizer_id = u.id AND status = 'published') AS events_count, \
            (SELECT avg(r.rating)::float8 FROM reviews r JOIN events e ON e.id = r.event_id \
                WHERE e.organizer_id = u.id AND e.status <> 'cancelled') AS rating_avg, \
            (SELECT count(*) FROM reviews r JOIN events e ON e.id = r.event_id \
                WHERE e.organizer_id = u.id AND e.status <> 'cancelled') AS rating_count, \
            EXISTS(SELECT 1 FROM follows WHERE follower_id = $2 AND followee_id = u.id) AS is_following \
         FROM users u WHERE u.id = $1 AND u.blocked = false",
        name = crate::users::DISPLAY_NAME_SQL
    );
    sqlx::query_as(&sql).bind(id).bind(viewer).fetch_optional(db).await
}

/// `GET /api/users/:id` — public profile with counters. Auth is optional
/// (`is_following` is only meaningful for authenticated callers).
pub async fn get_profile(
    viewer: Option<CurrentUser>,
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<PublicProfile>, AppError> {
    fetch_profile(&st.db, id, viewer.map(|v| v.id))
        .await?
        .map(Json)
        .ok_or(AppError::NotFound)
}

/// `GET /api/users/:id/events` — the user's published events (public).
pub async fn user_events(State(st): State<AppState>, Path(id): Path<Uuid>) -> Result<Json<Vec<Event>>, AppError> {
    let all = events::fetch_by_organizer(&st.db, id).await?;
    Ok(Json(all.into_iter().filter(|e| e.status == "published").collect()))
}

/// `POST /api/users/:id/follow` — idempotent follow.
pub async fn follow(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    if user.id == id {
        return Err(AppError::Validation("cannot follow yourself".into()));
    }
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1 AND blocked = false)")
        .bind(id)
        .fetch_one(&st.db)
        .await?;
    if !exists {
        return Err(AppError::NotFound);
    }
    let res = sqlx::query("INSERT INTO follows (follower_id, followee_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(user.id)
        .bind(id)
        .execute(&st.db)
        .await?;
    // Only notify on a fresh follow (not a repeat).
    if res.rows_affected() == 1 {
        notifications::notify_actor(
            &st.db,
            id,
            user.id,
            "new_follower",
            ("підписався(лась) на вас", "followed you"),
            None,
            None,
        )
        .await?;
    }
    crate::telemetry::log(&st.db, Some(user.id), "follow", serde_json::json!({ "target": id })).await;
    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /api/users/:id/follow` — idempotent unfollow.
pub async fn unfollow(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    sqlx::query("DELETE FROM follows WHERE follower_id = $1 AND followee_id = $2")
        .bind(user.id)
        .bind(id)
        .execute(&st.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Brief user shape for follower/following lists.
#[derive(Serialize, sqlx::FromRow)]
pub struct UserBrief {
    pub id: Uuid,
    pub name: String,
    pub avatar_url: Option<String>,
}

/// `sql` selects `{name}` for the display name (TASK-305 P), filled in here.
async fn fetch_briefs(db: &PgPool, sql: &str, id: Uuid) -> Result<Vec<UserBrief>, sqlx::Error> {
    let sql = sql.replace("{name}", crate::users::DISPLAY_NAME_SQL);
    sqlx::query_as(&sql).bind(id).fetch_all(db).await
}

/// `GET /api/users/:id/followers`
pub async fn followers(State(st): State<AppState>, Path(id): Path<Uuid>) -> Result<Json<Vec<UserBrief>>, AppError> {
    let users = fetch_briefs(
        &st.db,
        "SELECT u.id, {name} AS name, u.avatar_url FROM follows f \
         JOIN users u ON u.id = f.follower_id AND u.blocked = false \
         WHERE f.followee_id = $1 ORDER BY f.created_at DESC",
        id,
    )
    .await?;
    Ok(Json(users))
}

/// `GET /api/users/:id/following`
pub async fn following(State(st): State<AppState>, Path(id): Path<Uuid>) -> Result<Json<Vec<UserBrief>>, AppError> {
    let users = fetch_briefs(
        &st.db,
        "SELECT u.id, {name} AS name, u.avatar_url FROM follows f \
         JOIN users u ON u.id = f.followee_id AND u.blocked = false \
         WHERE f.follower_id = $1 ORDER BY f.created_at DESC",
        id,
    )
    .await?;
    Ok(Json(users))
}

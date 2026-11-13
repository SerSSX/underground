//! In-app notifications (F-14): generated when a new event matches user prefs
//! (interest categories + city geo-match).

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::events::Event;
use crate::state::AppState;

#[derive(Serialize, sqlx::FromRow)]
pub struct Notification {
    pub id: Uuid,
    pub kind: String,
    pub title: String,
    pub body: Option<String>,
    pub event_id: Option<Uuid>,
    /// Who triggered a social notification (liker/commenter/follower) — for profile deep-links.
    pub actor_id: Option<Uuid>,
    pub read: bool,
    pub created_at: DateTime<Utc>,
}

/// Emit a social notification to `target` triggered by `actor`. The title is
/// `"<actor name> <suffix>"`, with the suffix chosen by the recipient's locale
/// (TASK-017 M5). Self-triggered actions are a no-op (you don't get notified about
/// your own like/comment/follow).
pub async fn notify_actor(
    db: &PgPool,
    target: Uuid,
    actor: Uuid,
    kind: &str,
    // `(uk, en)` title suffixes, chosen by the recipient's locale.
    suffix: (&str, &str),
    body: Option<&str>,
    event_id: Option<Uuid>,
) -> Result<u64, sqlx::Error> {
    if target == actor {
        return Ok(0);
    }
    let result = sqlx::query(
        "INSERT INTO notifications (user_id, kind, title, body, event_id, actor_id) \
         SELECT $1, $2, (SELECT CASE WHEN u.name_is_placeholder IS NOT FALSE THEN 'Гість' ELSE u.name END \
                FROM users u WHERE u.id = $3) || ' ' || \
            CASE WHEN COALESCE((SELECT locale FROM user_prefs WHERE user_id = $1), 'uk') <> 'uk' \
                 THEN $5 ELSE $4 END, \
            $6, $7, $3",
    )
    .bind(target)
    .bind(kind)
    .bind(actor)
    .bind(suffix.0)
    .bind(suffix.1)
    .bind(body)
    .bind(event_id)
    .execute(db)
    .await?;
    Ok(result.rows_affected())
}

/// Most characters of an event title copied into a notification (TASK-305 F1).
pub const NOTIFY_TITLE_MAX: usize = 120;

/// Fan out an `event_match` notification to every user whose prefs match the
/// event's category and city (a missing city on either side matches anything).
/// The organizer and the organizer's followers are skipped — followers get the
/// richer `organizer_event` notification instead (no duplicate).
pub async fn notify_event_match(db: &PgPool, event: &Event) -> Result<u64, sqlx::Error> {
    let body = match &event.city {
        Some(city) => format!("{} · {city}", event.category),
        None => event.category.clone(),
    };
    let result = sqlx::query(
        "INSERT INTO notifications (user_id, kind, title, body, event_id) \
         SELECT p.user_id, 'event_match', \
            CASE WHEN p.locale <> 'uk' THEN 'New event: ' ELSE 'Нова подія: ' END || $1, $2, $3 \
         FROM user_prefs p \
         WHERE $4 = ANY(p.categories) \
           AND (p.city IS NULL OR $5::text IS NULL OR lower(p.city) = lower($5)) \
           AND ($6::uuid IS NULL OR p.user_id <> $6) \
           AND NOT EXISTS (SELECT 1 FROM follows f WHERE f.follower_id = p.user_id AND f.followee_id = $6)",
    )
    .bind(crate::text::truncate_chars(&event.title, NOTIFY_TITLE_MAX))
    .bind(body)
    .bind(event.id)
    .bind(&event.category)
    .bind(event.city.as_deref())
    .bind(event.organizer_id)
    .execute(db)
    .await?;
    Ok(result.rows_affected())
}

/// Notify every follower of the organizer that they announced a new event — the core
/// virality loop. A null organizer matches no followers, so it's a natural no-op.
pub async fn notify_followers_new_event(db: &PgPool, event: &Event) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO notifications (user_id, kind, title, body, event_id) \
         SELECT f.follower_id, 'organizer_event', \
            (SELECT CASE WHEN u.name_is_placeholder IS NOT FALSE THEN 'Гість' ELSE u.name END \
                FROM users u WHERE u.id = $1) || \
            CASE WHEN COALESCE(up.locale, 'uk') <> 'uk' THEN ' announced an event' ELSE ' оголосив(ла) подію' END, \
            $2, $3 \
         FROM follows f \
         LEFT JOIN user_prefs up ON up.user_id = f.follower_id \
         WHERE f.followee_id = $1",
    )
    .bind(event.organizer_id)
    .bind(crate::text::truncate_chars(&event.title, NOTIFY_TITLE_MAX))
    .bind(event.id)
    .execute(db)
    .await?;
    Ok(result.rows_affected())
}

/// `GET /api/me/notifications` — newest first (capped at 100).
pub async fn my_notifications(
    user: CurrentUser,
    State(st): State<AppState>,
) -> Result<Json<Vec<Notification>>, AppError> {
    let items: Vec<Notification> = sqlx::query_as(
        "SELECT id, kind, title, body, event_id, actor_id, read, created_at \
         FROM notifications WHERE user_id = $1 ORDER BY created_at DESC LIMIT 100",
    )
    .bind(user.id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(items))
}

#[derive(Serialize)]
pub struct UnreadCounts {
    pub notifications: i64,
    pub messages: i64,
}

/// `GET /api/me/unread` — badge counters for the Inbox tab.
pub async fn unread_counts(user: CurrentUser, State(st): State<AppState>) -> Result<Json<UnreadCounts>, AppError> {
    let notifications: i64 =
        sqlx::query_scalar("SELECT count(*) FROM notifications WHERE user_id = $1 AND read = false")
            .bind(user.id)
            .fetch_one(&st.db)
            .await?;
    let messages: i64 = sqlx::query_scalar("SELECT count(*) FROM messages WHERE recipient_id = $1 AND read = false")
        .bind(user.id)
        .fetch_one(&st.db)
        .await?;
    Ok(Json(UnreadCounts {
        notifications,
        messages,
    }))
}

/// `POST /api/me/notifications/read` — mark all of the caller's notifications read.
pub async fn mark_all_read(user: CurrentUser, State(st): State<AppState>) -> Result<StatusCode, AppError> {
    sqlx::query("UPDATE notifications SET read = true WHERE user_id = $1 AND read = false")
        .bind(user.id)
        .execute(&st.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

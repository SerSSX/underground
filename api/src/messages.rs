//! Direct messages: user ↔ organizer chat (conversations, thread, unread).

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
pub struct MessageView {
    pub id: Uuid,
    pub sender_id: Uuid,
    pub recipient_id: Uuid,
    pub text: String,
    pub read: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct SendMessageReq {
    pub recipient_id: Uuid,
    pub text: String,
}

/// `POST /api/messages` — send a direct message.
pub async fn send_message(
    user: CurrentUser,
    State(st): State<AppState>,
    Json(req): Json<SendMessageReq>,
) -> Result<(StatusCode, Json<MessageView>), AppError> {
    let text = req.text.trim();
    if text.is_empty() {
        return Err(AppError::Validation("text is required".into()));
    }
    if text.len() > MAX_TEXT_LEN {
        return Err(AppError::Validation(format!(
            "text must be at most {MAX_TEXT_LEN} chars"
        )));
    }
    if req.recipient_id == user.id {
        return Err(AppError::Validation("cannot message yourself".into()));
    }
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1 AND blocked = false)")
        .bind(req.recipient_id)
        .fetch_one(&st.db)
        .await?;
    if !exists {
        return Err(AppError::NotFound);
    }
    let message: MessageView = sqlx::query_as(
        "INSERT INTO messages (sender_id, recipient_id, text) VALUES ($1, $2, $3) \
         RETURNING id, sender_id, recipient_id, text, read, created_at",
    )
    .bind(user.id)
    .bind(req.recipient_id)
    .bind(text)
    .fetch_one(&st.db)
    .await?;
    Ok((StatusCode::CREATED, Json(message)))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct Conversation {
    pub peer_id: Uuid,
    pub peer_name: String,
    pub peer_avatar: Option<String>,
    pub last_text: String,
    pub last_at: DateTime<Utc>,
    pub last_from_me: bool,
    pub unread: i64,
}

/// `GET /api/me/conversations` — one row per peer, most recent first.
pub async fn conversations(user: CurrentUser, State(st): State<AppState>) -> Result<Json<Vec<Conversation>>, AppError> {
    let items: Vec<Conversation> = sqlx::query_as(
        "SELECT c.peer AS peer_id, u.name AS peer_name, u.avatar_url AS peer_avatar, \
            c.text AS last_text, c.created_at AS last_at, c.from_me AS last_from_me, \
            (SELECT count(*) FROM messages \
                WHERE recipient_id = $1 AND sender_id = c.peer AND read = false) AS unread \
         FROM ( \
            SELECT DISTINCT ON (peer) \
                CASE WHEN m.sender_id = $1 THEN m.recipient_id ELSE m.sender_id END AS peer, \
                m.text, m.created_at, (m.sender_id = $1) AS from_me \
            FROM messages m \
            WHERE m.sender_id = $1 OR m.recipient_id = $1 \
            ORDER BY peer, m.created_at DESC \
         ) c \
         JOIN users u ON u.id = c.peer \
         ORDER BY c.created_at DESC",
    )
    .bind(user.id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(items))
}

/// `GET /api/messages/:user_id` — the thread with one peer, oldest first.
/// Reading the thread marks the peer's messages as read.
pub async fn thread(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(peer_id): Path<Uuid>,
) -> Result<Json<Vec<MessageView>>, AppError> {
    let items: Vec<MessageView> = sqlx::query_as(
        "SELECT id, sender_id, recipient_id, text, read, created_at FROM messages \
         WHERE (sender_id = $1 AND recipient_id = $2) OR (sender_id = $2 AND recipient_id = $1) \
         ORDER BY created_at ASC LIMIT 500",
    )
    .bind(user.id)
    .bind(peer_id)
    .fetch_all(&st.db)
    .await?;
    sqlx::query("UPDATE messages SET read = true WHERE sender_id = $2 AND recipient_id = $1 AND read = false")
        .bind(user.id)
        .bind(peer_id)
        .execute(&st.db)
        .await?;
    Ok(Json(items))
}

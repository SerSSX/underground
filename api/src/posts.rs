//! Wall posts: manual posts, auto event announcements, auto review reposts;
//! likes and comments.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::notifications;
use crate::state::AppState;

/// Post and comment cap, in characters (TASK-305 F10).
const MAX_TEXT_LEN: usize = 2000;

/// A wall post with author, optional attached event, and social counters.
#[derive(Serialize, sqlx::FromRow)]
pub struct PostView {
    pub id: Uuid,
    pub author_id: Uuid,
    pub author_name: String,
    pub author_avatar: Option<String>,
    pub kind: String,
    pub text: Option<String>,
    pub rating: Option<i32>,
    pub event_id: Option<Uuid>,
    pub event_title: Option<String>,
    pub event_poster: Option<String>,
    pub event_category: Option<String>,
    pub event_starts_at: Option<DateTime<Utc>>,
    pub likes_count: i64,
    pub comments_count: i64,
    pub liked_by_me: bool,
    pub created_at: DateTime<Utc>,
    /// `following` | `interest` — why the post is in the caller's feed (feed only).
    #[sqlx(default)]
    pub reason: Option<String>,
}

/// The author's display name comes from `display_name_sql!` (the one copy of the rule, TASK-305
/// R1): a const cannot `format!`, but it can `concat!` a macro literal.
const POST_VIEW_COLS: &str = concat!(
    "p.id, p.author_id, \
        ",
    crate::display_name_sql!(),
    " AS author_name, \
        u.avatar_url AS author_avatar, \
        p.kind, p.text, p.rating, p.event_id, \
        e.title AS event_title, e.poster_url AS event_poster, e.category AS event_category, \
        e.starts_at AS event_starts_at, \
        (SELECT count(*) FROM post_likes pl WHERE pl.post_id = p.id) AS likes_count, \
        (SELECT count(*) FROM post_comments pc WHERE pc.post_id = p.id) AS comments_count, \
        EXISTS(SELECT 1 FROM post_likes pl WHERE pl.post_id = p.id AND pl.user_id = $2) AS liked_by_me, \
        p.created_at"
);

const POST_VIEW_FROM: &str = "FROM posts p \
     JOIN users u ON u.id = p.author_id \
     LEFT JOIN events e ON e.id = p.event_id";

/// Insert an automatic post (event announcement / review repost).
/// Review posts replace the author's previous review post of the same event.
pub async fn insert_auto_post(
    db: &PgPool,
    author_id: Uuid,
    kind: &str,
    text: Option<&str>,
    event_id: Option<Uuid>,
    rating: Option<i32>,
) -> Result<(), sqlx::Error> {
    if kind == "review" {
        sqlx::query("DELETE FROM posts WHERE author_id = $1 AND event_id = $2 AND kind = 'review'")
            .bind(author_id)
            .bind(event_id)
            .execute(db)
            .await?;
    }
    sqlx::query("INSERT INTO posts (author_id, kind, text, event_id, rating) VALUES ($1, $2, $3, $4, $5)")
        .bind(author_id)
        .bind(kind)
        .bind(text)
        .bind(event_id)
        .bind(rating)
        .execute(db)
        .await?;
    Ok(())
}

#[derive(Deserialize)]
pub struct CreatePostReq {
    pub text: Option<String>,
    pub event_id: Option<Uuid>,
}

/// `POST /api/posts` — manual post; an attached event must be the author's own
/// (sharing someone else's event happens via reviews).
pub async fn create_post(
    user: CurrentUser,
    State(st): State<AppState>,
    Json(req): Json<CreatePostReq>,
) -> Result<(StatusCode, Json<PostView>), AppError> {
    // Controls and bidi controls out, newlines kept; nothing visible means no text (TASK-305 R5).
    let text = req.text.as_deref().map(crate::text::clean_multiline);
    let text = text.as_deref().filter(|t| !crate::text::is_blank(t));
    if text.is_none() && req.event_id.is_none() {
        return Err(AppError::Validation("text or event_id is required".into()));
    }
    if text.is_some_and(|t| t.chars().count() > MAX_TEXT_LEN) {
        return Err(AppError::Validation(format!(
            "text must be at most {MAX_TEXT_LEN} chars"
        )));
    }
    let kind = if req.event_id.is_some() { "event_share" } else { "text" };
    if let Some(event_id) = req.event_id {
        let owns: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM events WHERE id = $1 AND organizer_id = $2)")
            .bind(event_id)
            .bind(user.id)
            .fetch_one(&st.db)
            .await?;
        owns.then_some(()).ok_or(AppError::NotFound)?;
    }
    let id: Uuid =
        sqlx::query_scalar("INSERT INTO posts (author_id, kind, text, event_id) VALUES ($1, $2, $3, $4) RETURNING id")
            .bind(user.id)
            .bind(kind)
            .bind(text)
            .bind(req.event_id)
            .fetch_one(&st.db)
            .await?;
    let post = fetch_post(&st.db, id, Some(user.id)).await?.ok_or(AppError::Internal)?;
    Ok((StatusCode::CREATED, Json(post)))
}

async fn fetch_post(db: &PgPool, id: Uuid, viewer: Option<Uuid>) -> Result<Option<PostView>, sqlx::Error> {
    let sql = format!("SELECT {POST_VIEW_COLS} {POST_VIEW_FROM} WHERE p.id = $1");
    sqlx::query_as(&sql).bind(id).bind(viewer).fetch_optional(db).await
}

/// `GET /api/users/:id/posts` — a user's wall, newest first (public).
pub async fn user_posts(
    viewer: Option<CurrentUser>,
    State(st): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<Vec<PostView>>, AppError> {
    let sql =
        format!("SELECT {POST_VIEW_COLS} {POST_VIEW_FROM} WHERE p.author_id = $1 ORDER BY p.created_at DESC LIMIT 100");
    let posts: Vec<PostView> = sqlx::query_as(&sql)
        .bind(user_id)
        .bind(viewer.map(|v| v.id))
        .fetch_all(&st.db)
        .await?;
    Ok(Json(posts))
}

/// `GET /api/feed` — activity stream for the caller: every post by people they
/// follow (makers and regular users alike), plus posts attached to events in
/// their interest categories (onboarding prefs). Own posts are excluded; when a
/// post qualifies both ways, `following` wins as the reason.
pub async fn feed(user: CurrentUser, State(st): State<AppState>) -> Result<Json<Vec<PostView>>, AppError> {
    let sql = format!(
        "SELECT {POST_VIEW_COLS}, \
            CASE WHEN f.follower_id IS NOT NULL THEN 'following' ELSE 'interest' END AS reason \
         {POST_VIEW_FROM} \
         LEFT JOIN follows f ON f.follower_id = $1 AND f.followee_id = p.author_id \
         LEFT JOIN user_prefs up ON up.user_id = $1 \
         WHERE p.author_id <> $1 \
           AND u.blocked = false \
           AND (f.follower_id IS NOT NULL OR e.category = ANY(up.categories)) \
         ORDER BY p.created_at DESC LIMIT 50"
    );
    let posts: Vec<PostView> = sqlx::query_as(&sql)
        .bind(user.id)
        .bind(user.id)
        .fetch_all(&st.db)
        .await?;
    Ok(Json(posts))
}

/// `DELETE /api/posts/:id` — author-only.
pub async fn delete_post(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let deleted = sqlx::query("DELETE FROM posts WHERE id = $1 AND author_id = $2")
        .bind(id)
        .bind(user.id)
        .execute(&st.db)
        .await?;
    if deleted.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /api/posts/:id/like` — idempotent.
pub async fn like(user: CurrentUser, State(st): State<AppState>, Path(id): Path<Uuid>) -> Result<StatusCode, AppError> {
    // Fetch the author up front: this is both the existence check (404) and the
    // notification target, so no post-insert lookup can race a concurrent deletion.
    let owner: Uuid = sqlx::query_scalar("SELECT author_id FROM posts WHERE id = $1")
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let res = sqlx::query("INSERT INTO post_likes (post_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(id)
        .bind(user.id)
        .execute(&st.db)
        .await?;
    // Only notify on a fresh like (not a repeat) — the post's author.
    if res.rows_affected() == 1 {
        notify_post_owner(
            &st.db,
            owner,
            user.id,
            "post_like",
            ("вподобав(ла) ваш допис", "liked your post"),
            None,
        )
        .await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Notify a post's author about a social action. The suffix is localized to the
/// recipient (TASK-017 M5): `suffix_uk` / `suffix_en`.
async fn notify_post_owner(
    db: &PgPool,
    owner: Uuid,
    actor: Uuid,
    kind: &str,
    suffix: (&str, &str),
    body: Option<&str>,
) -> Result<(), AppError> {
    notifications::notify_actor(db, owner, actor, kind, suffix, body, None).await?;
    Ok(())
}

/// `DELETE /api/posts/:id/like` — idempotent.
pub async fn unlike(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    sqlx::query("DELETE FROM post_likes WHERE post_id = $1 AND user_id = $2")
        .bind(id)
        .bind(user.id)
        .execute(&st.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, sqlx::FromRow)]
pub struct CommentView {
    pub id: Uuid,
    pub post_id: Uuid,
    pub author_id: Uuid,
    pub author_name: String,
    pub text: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct CreateCommentReq {
    pub text: String,
}

/// `POST /api/posts/:id/comments`
pub async fn create_comment(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<CreateCommentReq>,
) -> Result<(StatusCode, Json<CommentView>), AppError> {
    // TASK-305 R5: cleaned like a post; whitespace or zero-width only is no comment.
    let text = crate::text::clean_multiline(&req.text);
    let text = text.as_str();
    if crate::text::is_blank(text) {
        return Err(AppError::Validation("text is required".into()));
    }
    if text.chars().count() > MAX_TEXT_LEN {
        return Err(AppError::Validation(format!(
            "text must be at most {MAX_TEXT_LEN} chars"
        )));
    }
    // Fetch the author up front: existence check (404) + notification target in one,
    // so the comment insert and notification never race a concurrent deletion.
    let owner: Uuid = sqlx::query_scalar("SELECT author_id FROM posts WHERE id = $1")
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let comment: CommentView = sqlx::query_as(concat!(
        "WITH c AS ( \
            INSERT INTO post_comments (post_id, author_id, text) VALUES ($1, $2, $3) \
            RETURNING id, post_id, author_id, text, created_at \
         ) \
         SELECT c.id, c.post_id, c.author_id, \
            ",
        crate::display_name_sql!(),
        " AS author_name, \
            c.text, c.created_at \
         FROM c JOIN users u ON u.id = c.author_id"
    ))
    .bind(id)
    .bind(user.id)
    .bind(text)
    .fetch_one(&st.db)
    .await?;
    notify_post_owner(
        &st.db,
        owner,
        user.id,
        "post_comment",
        ("прокоментував(ла) ваш допис", "commented on your post"),
        Some(text),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(comment)))
}

/// `GET /api/posts/:id/comments` — oldest first (public).
pub async fn comments(State(st): State<AppState>, Path(id): Path<Uuid>) -> Result<Json<Vec<CommentView>>, AppError> {
    let items: Vec<CommentView> = sqlx::query_as(concat!(
        "SELECT c.id, c.post_id, c.author_id, \
            ",
        crate::display_name_sql!(),
        " AS author_name, \
            c.text, c.created_at \
         FROM post_comments c JOIN users u ON u.id = c.author_id \
         WHERE c.post_id = $1 ORDER BY c.created_at ASC LIMIT 200"
    ))
    .bind(id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(items))
}

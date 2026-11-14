//! Person-to-person blocks (TASK-306). A blocks B: no new message, follow, like or comment between
//! them either way; nothing B does reaches A as an alert; A's lists leave B out. Not the admin ban
//! (`users.blocked`). Abuse cases: `docs/backlog/TASK-306-abuse-cases.md`.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::state::AppState;

/// Most people one account can block (D1).
pub const MAX_BLOCKS: i64 = 1000;

/// Advisory-lock namespace for one pair of people ("BLKP"): a block and any write it refuses between
/// the same two people take this lock, so no such write commits after the block did (D11).
const PAIR_LOCK_NAMESPACE: i32 = 0x424c_4b50;

/// Advisory-lock namespace for one blocker's count ("BLKC"), so two blocks at the cap cannot both pass.
const COUNT_LOCK_NAMESPACE: i32 = 0x424c_4b43;

fn per_minute_limit() -> i64 {
    crate::ratelimit::env_limit("BLOCK_LIMIT_PER_MINUTE", 30)
}

/// SQL: `viewer` (an expression, may be NULL) has blocked the user in `col`. False for a NULL viewer
/// or a NULL `col`. Both arguments are trusted SQL fragments written in this crate, never input.
pub fn blocked_by(viewer: &str, col: &str) -> String {
    format!("EXISTS(SELECT 1 FROM user_blocks ub WHERE ub.blocker_id = {viewer} AND ub.blocked_id = {col})")
}

/// Take the pair lock for `a` and `b` (the same lock whichever comes first), for the rest of the
/// transaction.
pub async fn lock_pair(conn: &mut sqlx::PgConnection, a: Uuid, b: Uuid) -> Result<(), sqlx::Error> {
    let (lo, hi) = if a < b { (a, b) } else { (b, a) };
    sqlx::query("SELECT pg_advisory_xact_lock($1, hashtext($2))")
        .bind(PAIR_LOCK_NAMESPACE)
        .bind(format!("{lo}:{hi}"))
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// Inside a transaction: take the pair lock, then refuse with 403 when either has blocked the other.
/// The 403 body is the same whichever side blocked (D6).
pub async fn ensure_not_blocked(conn: &mut sqlx::PgConnection, a: Uuid, b: Uuid) -> Result<(), AppError> {
    lock_pair(conn, a, b).await?;
    let blocked: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM user_blocks \
            WHERE (blocker_id = $1 AND blocked_id = $2) OR (blocker_id = $2 AND blocked_id = $1))",
    )
    .bind(a)
    .bind(b)
    .fetch_one(&mut *conn)
    .await?;
    if blocked {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

/// An active account: exists, not admin-banned (an erased account is banned too).
async fn is_active(conn: &mut sqlx::PgConnection, id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1 AND blocked = false)")
        .bind(id)
        .fetch_one(&mut *conn)
        .await
}

/// `POST /api/users/:id/block` — idempotent; 204. Self 422, no such active account 404, the 1001st
/// block 409 (checked before anything changes). Removes follows both ways; alerts from the blocked
/// person still waiting for push are never pushed (D8).
pub async fn block(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    if id == user.id {
        return Err(AppError::Validation("cannot block yourself".into()));
    }
    crate::ratelimit::hit(&st.db, &format!("block:{}", user.id), per_minute_limit(), 60).await?;
    let mut tx = st.db.begin().await?;
    // Count lock first, pair lock second; nothing takes them the other way round.
    sqlx::query("SELECT pg_advisory_xact_lock($1, hashtext($2))")
        .bind(COUNT_LOCK_NAMESPACE)
        .bind(user.id.to_string())
        .execute(&mut *tx)
        .await?;
    lock_pair(&mut tx, user.id, id).await?;
    let already: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM user_blocks WHERE blocker_id = $1 AND blocked_id = $2)")
            .bind(user.id)
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
    if already {
        tx.commit().await?;
        return Ok(StatusCode::NO_CONTENT);
    }
    if !is_active(&mut tx, id).await? {
        return Err(AppError::NotFound);
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM user_blocks WHERE blocker_id = $1")
        .bind(user.id)
        .fetch_one(&mut *tx)
        .await?;
    if count >= MAX_BLOCKS {
        return Err(AppError::Conflict(format!("at most {MAX_BLOCKS} blocked users")));
    }
    sqlx::query("INSERT INTO user_blocks (blocker_id, blocked_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(user.id)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "DELETE FROM follows WHERE (follower_id = $1 AND followee_id = $2) OR (follower_id = $2 AND followee_id = $1)",
    )
    .bind(user.id)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE notifications SET pushed_at = now() WHERE user_id = $1 AND actor_id = $2 AND pushed_at IS NULL",
    )
    .bind(user.id)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /api/users/:id/block` — idempotent; 204. A block the caller holds is always removed, whatever
/// became of the other account (D2); with none, 404 when no such active account exists.
pub async fn unblock(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    if id == user.id {
        return Err(AppError::Validation("cannot unblock yourself".into()));
    }
    crate::ratelimit::hit(&st.db, &format!("block:{}", user.id), per_minute_limit(), 60).await?;
    let mut tx = st.db.begin().await?;
    lock_pair(&mut tx, user.id, id).await?;
    let removed = sqlx::query("DELETE FROM user_blocks WHERE blocker_id = $1 AND blocked_id = $2")
        .bind(user.id)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    if removed.rows_affected() == 0 && !is_active(&mut tx, id).await? {
        return Err(AppError::NotFound);
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, sqlx::FromRow)]
pub struct BlockedUser {
    pub id: Uuid,
    pub name: String,
    pub avatar_url: Option<String>,
    pub blocked_at: DateTime<Utc>,
}

/// `GET /api/me/blocks` — everyone the caller blocked, newest first; all of them (at most [`MAX_BLOCKS`]).
pub async fn my_blocks(user: CurrentUser, State(st): State<AppState>) -> Result<Json<Vec<BlockedUser>>, AppError> {
    let items: Vec<BlockedUser> = sqlx::query_as(concat!(
        "SELECT u.id, ",
        crate::display_name_sql!(),
        " AS name, u.avatar_url, b.created_at AS blocked_at \
         FROM user_blocks b JOIN users u ON u.id = b.blocked_id \
         WHERE b.blocker_id = $1 ORDER BY b.created_at DESC, u.id"
    ))
    .bind(user.id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(items))
}

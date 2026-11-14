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
    /// Where the alert leads in the web app, when that is not the event page (TASK-305 UX7): the
    /// claim page `/g/<token>` of a place offered to the caller while the offer is open, or the
    /// manage page `/organizer/<event id>` of a new booking. A relative path, or null.
    pub link: Option<String>,
}

/// A notification as stored, before the claim link of an open offer is rebuilt.
#[derive(sqlx::FromRow)]
struct NotificationRow {
    id: Uuid,
    kind: String,
    title: String,
    body: Option<String>,
    event_id: Option<Uuid>,
    actor_id: Option<Uuid>,
    read: bool,
    created_at: DateTime<Utc>,
    link: Option<String>,
    /// The offer's gift, only while it is pending and not expired.
    open_gift: Option<Uuid>,
}

/// Emit a social notification to `target` triggered by `actor`. The title is
/// `"<actor name> <suffix>"`, with the suffix chosen by the recipient's locale
/// (TASK-017 M5). Self-triggered actions are a no-op (you don't get notified about
/// your own like/comment/follow), and so is an action by someone the target blocked (TASK-306 S4).
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
    let result = sqlx::query(concat!(
        "INSERT INTO notifications (user_id, kind, title, body, event_id, actor_id) \
         SELECT $1, $2, (SELECT ",
        crate::display_name_sql!(),
        " \
                FROM users u WHERE u.id = $3) || ' ' || \
            CASE WHEN COALESCE((SELECT locale FROM user_prefs WHERE user_id = $1), 'uk') <> 'uk' \
                 THEN $5 ELSE $4 END, \
            $6, $7, $3 \
         WHERE NOT EXISTS (SELECT 1 FROM user_blocks ub WHERE ub.blocker_id = $1 AND ub.blocked_id = $3)"
    ))
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
        "INSERT INTO notifications (user_id, kind, title, body, event_id, actor_id) \
         SELECT p.user_id, 'event_match', \
            CASE WHEN p.locale <> 'uk' THEN 'New event: ' ELSE 'Нова подія: ' END || $1, $2, $3, $6 \
         FROM user_prefs p \
         WHERE $4 = ANY(p.categories) \
           AND (p.city IS NULL OR $5::text IS NULL OR lower(p.city) = lower($5)) \
           AND ($6::uuid IS NULL OR p.user_id <> $6) \
           AND NOT EXISTS (SELECT 1 FROM follows f WHERE f.follower_id = p.user_id AND f.followee_id = $6) \
           AND NOT EXISTS (SELECT 1 FROM user_blocks ub WHERE ub.blocker_id = p.user_id AND ub.blocked_id = $6)",
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
    let result = sqlx::query(concat!(
        "INSERT INTO notifications (user_id, kind, title, body, event_id, actor_id) \
         SELECT f.follower_id, 'organizer_event', \
            (SELECT ",
        crate::display_name_sql!(),
        " \
                FROM users u WHERE u.id = $1) || \
            CASE WHEN COALESCE(up.locale, 'uk') <> 'uk' THEN ' announced an event' ELSE ' оголосив(ла) подію' END, \
            $2, $3, $1 \
         FROM follows f \
         LEFT JOIN user_prefs up ON up.user_id = f.follower_id \
         WHERE f.followee_id = $1 \
           AND NOT EXISTS (SELECT 1 FROM user_blocks ub WHERE ub.blocker_id = f.follower_id AND ub.blocked_id = $1)"
    ))
    .bind(event.organizer_id)
    .bind(crate::text::truncate_chars(&event.title, NOTIFY_TITLE_MAX))
    .bind(event.id)
    .execute(db)
    .await?;
    Ok(result.rows_affected())
}

/// SQL (events aliased `e`): the event title cut to [`NOTIFY_TITLE_MAX`] characters, the same
/// cut as `text::truncate_chars`.
fn event_title_cut_sql() -> String {
    format!(
        "CASE WHEN char_length(e.title) > {max} THEN left(e.title, {keep}) || '…' ELSE e.title END",
        max = NOTIFY_TITLE_MAX,
        keep = NOTIFY_TITLE_MAX - 1
    )
}

/// UX7: `recipient` is offered a place on `ticket_id` by `sender` (a gift to their email, or a
/// transfer). The alert stores the gift, not its token; [`my_notifications`] shows the claim link
/// while the offer is open. Runs in the caller's transaction. No alert when `recipient` blocked `sender`
/// (TASK-306 S6): the offer then goes on as one to an unknown email.
pub async fn notify_place_offer<'e, E: sqlx::PgExecutor<'e>>(
    db: E,
    recipient: Uuid,
    sender: Uuid,
    ticket_id: Uuid,
    gift_id: Uuid,
) -> Result<(), sqlx::Error> {
    let sql = format!(
        "INSERT INTO notifications (user_id, kind, title, body, event_id, actor_id, gift_id) \
         SELECT $1, 'place_offer', \
            (SELECT {name} FROM users u WHERE u.id = $2) || \
            CASE WHEN COALESCE((SELECT locale FROM user_prefs WHERE user_id = $1), 'uk') <> 'uk' \
                 THEN ' offers you a place' ELSE ' пропонує вам місце' END, \
            {title}, e.id, $2, $3 \
         FROM tickets t JOIN events e ON e.id = t.event_id WHERE t.id = $4 \
           AND NOT EXISTS (SELECT 1 FROM user_blocks ub WHERE ub.blocker_id = $1 AND ub.blocked_id = $2)",
        name = crate::users::DISPLAY_NAME_SQL,
        title = event_title_cut_sql(),
    );
    sqlx::query(&sql)
        .bind(recipient)
        .bind(sender)
        .bind(gift_id)
        .bind(ticket_id)
        .execute(db)
        .await?;
    Ok(())
}

/// UX7: the organiser of `event_id` is told that `booker` booked `places` places, with a link to
/// the event's manage page, with the booker as the actor (TASK-306 D9). Nothing when the organiser booked
/// their own event or blocked the booker. Runs in the
/// caller's transaction (reserve, paid-order fulfilment).
pub async fn notify_booking<'e, E: sqlx::PgExecutor<'e>>(
    db: E,
    event_id: Uuid,
    booker: Uuid,
    places: i64,
) -> Result<(), sqlx::Error> {
    let sql = format!(
        "INSERT INTO notifications (user_id, kind, title, body, event_id, link, actor_id) \
         SELECT e.organizer_id, 'booking', \
            CASE WHEN COALESCE(p.locale, 'uk') <> 'uk' THEN 'New booking: ' ELSE 'Нове бронювання: ' END || {title}, \
            CASE WHEN COALESCE(p.locale, 'uk') <> 'uk' THEN 'Places: ' ELSE 'Місць: ' END || $3, \
            e.id, '/organizer/' || e.id, $2 \
         FROM events e LEFT JOIN user_prefs p ON p.user_id = e.organizer_id \
         WHERE e.id = $1 AND e.organizer_id IS NOT NULL AND e.organizer_id <> $2 \
           AND NOT EXISTS (SELECT 1 FROM user_blocks ub WHERE ub.blocker_id = e.organizer_id AND ub.blocked_id = $2)",
        title = event_title_cut_sql(),
    );
    sqlx::query(&sql)
        .bind(event_id)
        .bind(booker)
        .bind(places.to_string())
        .execute(db)
        .await?;
    Ok(())
}

/// `GET /api/me/notifications` — newest first: the newest 100, plus every place offer to the caller
/// that is still open, however old (TASK-305 round 3: its claim link is only reachable here).
pub async fn my_notifications(
    user: CurrentUser,
    State(st): State<AppState>,
) -> Result<Json<Vec<Notification>>, AppError> {
    // TASK-306 S4: alerts whose actor the caller blocked are left out of both parts, before the limit.
    let rows: Vec<NotificationRow> = sqlx::query_as(&format!(
        "SELECT n.id, n.kind, n.title, n.body, n.event_id, n.actor_id, n.read, n.created_at, n.link, \
            g.id AS open_gift \
         FROM ( \
            (SELECT a.id, a.kind, a.title, a.body, a.event_id, a.actor_id, a.read, a.created_at, a.link, a.gift_id \
             FROM notifications a WHERE a.user_id = $1 AND NOT {hide_a} ORDER BY a.created_at DESC LIMIT 100) \
            UNION \
            (SELECT x.id, x.kind, x.title, x.body, x.event_id, x.actor_id, x.read, x.created_at, x.link, x.gift_id \
             FROM notifications x JOIN ticket_gifts og ON og.id = x.gift_id \
             WHERE x.user_id = $1 AND og.status = 'pending' AND og.expires_at > now() AND NOT {hide_x}) \
         ) n \
         LEFT JOIN ticket_gifts g ON g.id = n.gift_id AND g.status = 'pending' AND g.expires_at > now() \
         ORDER BY n.created_at DESC",
        hide_a = crate::blocks::blocked_by("$1", "a.actor_id"),
        hide_x = crate::blocks::blocked_by("$1", "x.actor_id"),
    ))
    .bind(user.id)
    .fetch_all(&st.db)
    .await?;
    let items = rows
        .into_iter()
        .map(|r| Notification {
            id: r.id,
            kind: r.kind,
            title: r.title,
            body: r.body,
            event_id: r.event_id,
            actor_id: r.actor_id,
            read: r.read,
            created_at: r.created_at,
            link: r
                .open_gift
                .map(|g| format!("/g/{}", crate::tickets::claim_token(&st.jwt_secret, g)))
                .or(r.link),
        })
        .collect();
    Ok(Json(items))
}

#[derive(Serialize)]
pub struct UnreadCounts {
    pub notifications: i64,
    pub messages: i64,
}

/// `GET /api/me/unread` — badge counters for the Inbox tab.
pub async fn unread_counts(user: CurrentUser, State(st): State<AppState>) -> Result<Json<UnreadCounts>, AppError> {
    let notifications: i64 = sqlx::query_scalar(&format!(
        "SELECT count(*) FROM notifications n WHERE n.user_id = $1 AND n.read = false AND NOT {}",
        crate::blocks::blocked_by("$1", "n.actor_id")
    ))
    .bind(user.id)
    .fetch_one(&st.db)
    .await?;
    let messages: i64 = sqlx::query_scalar(&format!(
        "SELECT count(*) FROM messages m WHERE m.recipient_id = $1 AND m.read = false AND NOT {}",
        crate::blocks::blocked_by("$1", "m.sender_id")
    ))
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

//! Discovery engine (TASK-017 M1): personalized "For You" ranking + saved/interested.
//!
//! Ranking blends personal relevance (interests, home city, followed organizers) with
//! event momentum (tickets sold + saves + reviews) and recency. Anonymous callers get
//! a pure trending feed. Each result carries a `reason` so the UI can explain itself.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::events::{Event, EVENT_COLS};
use crate::state::AppState;

/// An event ranked for discovery, with social-proof counters and the reason it surfaced.
#[derive(Serialize, sqlx::FromRow)]
pub struct DiscoverEvent {
    #[sqlx(flatten)]
    #[serde(flatten)]
    pub event: Event,
    pub saves_count: i64,
    pub saved_by_me: bool,
    /// `followed` | `near` | `interest` | `city` | `trending`.
    pub reason: String,
    /// Great-circle km from the viewer's city to the event (when both have coordinates).
    pub distance_km: Option<f64>,
}

/// A titled group of discovery results (TASK-200, BR-024).
#[derive(Serialize)]
pub struct DiscoverSection {
    pub key: String,
    pub title: String,
    pub events: Vec<DiscoverEvent>,
}

/// Great-circle distance (km) between the viewer's city `vc` and the event `e`.
const HAVERSINE: &str = "6371 * acos(least(1.0, greatest(-1.0, \
    sin(radians(vc.lat)) * sin(radians(e.lat)) \
  + cos(radians(vc.lat)) * cos(radians(e.lat)) * cos(radians(e.lng - vc.lng)))))";

/// Social-proof momentum: tickets + saves + reviews (shared by "For You" and "Popular").
const MOMENTUM: &str = "(SELECT count(*) FROM tickets t WHERE t.event_id = e.id) * 0.5 \
    + (SELECT count(*) FROM event_saves s WHERE s.event_id = e.id) * 0.5 \
    + (SELECT count(*) FROM reviews r WHERE r.event_id = e.id) * 0.3";

/// Personalized ranking (relevance + geo + momentum + recency), used by `/discover` and the
/// `for_you` section so both rank identically.
fn personal_order() -> String {
    format!(
        "( (CASE WHEN f.follower_id IS NOT NULL THEN 4 ELSE 0 END) \
        + (CASE WHEN up.categories IS NOT NULL AND e.category = ANY(up.categories) THEN 3 ELSE 0 END) \
        + (CASE WHEN up.city IS NOT NULL AND e.city IS NOT NULL AND lower(up.city) = lower(e.city) THEN 2 ELSE 0 END) \
        + (CASE WHEN COALESCE(up.country, vc.country) IS NOT NULL AND COALESCE(up.country, vc.country) = e.country THEN 1.5 ELSE 0 END) \
        + (CASE WHEN vc.lat IS NOT NULL AND e.lat IS NOT NULL THEN greatest(0, 5 - ({HAVERSINE}) / 100) ELSE 0 END) \
        + {MOMENTUM} \
        + (CASE WHEN e.starts_at IS NOT NULL \
                THEN greatest(0, 30 - EXTRACT(EPOCH FROM (e.starts_at - now())) / 86400) * 0.1 ELSE 0 END) \
        ) DESC, e.starts_at ASC NULLS LAST"
    )
}

/// Run a discovery query with a section-specific extra `WHERE` and `ORDER BY`. Always scoped to
/// published events the viewer doesn't organize (SEC-2/SEC-3). Personalization binds `$1`.
async fn section_events(
    db: &sqlx::PgPool,
    viewer_id: Option<Uuid>,
    extra_where: &str,
    order: &str,
    limit: i64,
) -> Result<Vec<DiscoverEvent>, AppError> {
    let cols = EVENT_COLS
        .split(", ")
        .map(|c| format!("e.{c}"))
        .collect::<Vec<_>>()
        .join(", ");
    let dist = format!("CASE WHEN vc.lat IS NOT NULL AND e.lat IS NOT NULL THEN {HAVERSINE} ELSE NULL END");
    let sql = format!(
        "SELECT {cols}, \
            (SELECT count(*) FROM event_saves s WHERE s.event_id = e.id) AS saves_count, \
            EXISTS(SELECT 1 FROM event_saves s WHERE s.event_id = e.id AND s.user_id = $1) AS saved_by_me, \
            CASE \
                WHEN f.follower_id IS NOT NULL THEN 'followed' \
                WHEN vc.lat IS NOT NULL AND e.lat IS NOT NULL AND ({HAVERSINE}) < 100 THEN 'near' \
                WHEN up.categories IS NOT NULL AND e.category = ANY(up.categories) THEN 'interest' \
                WHEN up.city IS NOT NULL AND e.city IS NOT NULL AND lower(up.city) = lower(e.city) THEN 'city' \
                ELSE 'trending' \
            END AS reason, \
            {dist} AS distance_km \
         FROM events e \
         LEFT JOIN follows f ON f.follower_id = $1 AND f.followee_id = e.organizer_id \
         LEFT JOIN user_prefs up ON up.user_id = $1 \
         LEFT JOIN cities vc ON lower(vc.name) = lower(up.city) \
         WHERE e.status = 'published' \
           AND ($1::uuid IS NULL OR e.organizer_id IS NULL OR e.organizer_id <> $1) \
           {extra_where} \
         ORDER BY {order} LIMIT {limit}"
    );
    Ok(sqlx::query_as(&sql).bind(viewer_id).fetch_all(db).await?)
}

/// `GET /api/discover` — ranked "For You". Auth optional (personalizes when present).
/// Geo-aware: closer events (and same-country) rank higher when locations are known.
pub async fn discover(
    viewer: Option<CurrentUser>,
    State(st): State<AppState>,
) -> Result<Json<Vec<DiscoverEvent>>, AppError> {
    let viewer_id = viewer.map(|v| v.id);
    let events = section_events(
        &st.db,
        viewer_id,
        "AND (e.starts_at IS NULL OR e.starts_at >= now())",
        &personal_order(),
        50,
    )
    .await?;
    Ok(Json(events))
}

/// True when a signed-in viewer has any personalization signal (prefs, saves, or follows).
async fn has_signals(db: &sqlx::PgPool, viewer_id: Uuid) -> Result<bool, AppError> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM user_prefs WHERE user_id = $1) \
             OR EXISTS(SELECT 1 FROM event_saves WHERE user_id = $1) \
             OR EXISTS(SELECT 1 FROM follows WHERE follower_id = $1)",
    )
    .bind(viewer_id)
    .fetch_one(db)
    .await?)
}

const SECTION_LIMIT: i64 = 12;
const FUTURE: &str = "AND (e.starts_at IS NULL OR e.starts_at >= now())";

/// `GET /api/discover/sections` — time-aware sectioned Афіша (BR-024). Additive; the flat
/// `/api/discover` is unchanged. Empty sections are omitted (fallback chain).
pub async fn discover_sections(
    viewer: Option<CurrentUser>,
    State(st): State<AppState>,
) -> Result<Json<Vec<DiscoverSection>>, AppError> {
    let viewer_id = viewer.map(|v| v.id);
    let mut sections: Vec<DiscoverSection> = Vec::new();

    // "For You" — only for a signed-in viewer who has a personalization signal.
    if let Some(vid) = viewer_id {
        if has_signals(&st.db, vid).await? {
            let events = section_events(&st.db, viewer_id, FUTURE, &personal_order(), SECTION_LIMIT).await?;
            if !events.is_empty() {
                sections.push(DiscoverSection {
                    key: "for_you".into(),
                    title: "Для тебе".into(),
                    events,
                });
            }
        }
    }

    let soon = section_events(
        &st.db,
        viewer_id,
        "AND e.starts_at IS NOT NULL AND e.starts_at >= now() AND e.starts_at < now() + interval '48 hours'",
        "e.starts_at ASC",
        SECTION_LIMIT,
    )
    .await?;
    if !soon.is_empty() {
        sections.push(DiscoverSection {
            key: "soon".into(),
            title: "Незабаром".into(),
            events: soon,
        });
    }

    let weekend = section_events(
        &st.db,
        viewer_id,
        "AND e.starts_at >= date_trunc('week', now()) + interval '5 days' \
         AND e.starts_at < date_trunc('week', now()) + interval '7 days'",
        "e.starts_at ASC",
        SECTION_LIMIT,
    )
    .await?;
    if !weekend.is_empty() {
        sections.push(DiscoverSection {
            key: "weekend".into(),
            title: "Цими вихідними".into(),
            events: weekend,
        });
    }

    let popular = section_events(
        &st.db,
        viewer_id,
        FUTURE,
        &format!("({MOMENTUM}) DESC, e.starts_at ASC NULLS LAST"),
        SECTION_LIMIT,
    )
    .await?;
    if !popular.is_empty() {
        sections.push(DiscoverSection {
            key: "popular".into(),
            title: "Популярне".into(),
            events: popular,
        });
    }

    Ok(Json(sections))
}

async fn event_published(db: &sqlx::PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM events WHERE id = $1 AND status = 'published')")
        .bind(id)
        .fetch_one(db)
        .await
}

/// `POST /api/events/:id/save` — mark interested (idempotent).
pub async fn save_event(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    if !event_published(&st.db, id).await? {
        return Err(AppError::NotFound);
    }
    sqlx::query("INSERT INTO event_saves (user_id, event_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(user.id)
        .bind(id)
        .execute(&st.db)
        .await?;
    crate::telemetry::log(&st.db, Some(user.id), "save", serde_json::json!({ "event_id": id })).await;
    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /api/events/:id/save` — un-save (idempotent).
pub async fn unsave_event(
    user: CurrentUser,
    State(st): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    sqlx::query("DELETE FROM event_saves WHERE user_id = $1 AND event_id = $2")
        .bind(user.id)
        .bind(id)
        .execute(&st.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /api/me/saves` — the caller's saved events, newest first (for the star state + a saved list).
pub async fn my_saves(user: CurrentUser, State(st): State<AppState>) -> Result<Json<Vec<Event>>, AppError> {
    let cols = EVENT_COLS
        .split(", ")
        .map(|c| format!("e.{c}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT {cols} FROM event_saves s JOIN events e ON e.id = s.event_id \
         WHERE s.user_id = $1 ORDER BY s.created_at DESC"
    );
    let events: Vec<Event> = sqlx::query_as(&sql).bind(user.id).fetch_all(&st.db).await?;
    Ok(Json(events))
}

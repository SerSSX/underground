//! Event catalog (афіша): read + create.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::state::AppState;

pub const EVENT_COLS: &str = "id, organizer_id, title, description, poster_url, category, city, venue_address, \
     starts_at, ends_at, price, currency, tickets_left, status, cost_type, \
     video_url, gallery_urls, links, sales_closed, country, lat, lng, age_rating, timezone";

/// Allowed minimum-age ratings (empty/NULL = no restriction).
const AGE_RATINGS: [&str; 4] = ["13", "16", "18", "21"];

fn validate_age_rating(age_rating: Option<&str>) -> Result<(), AppError> {
    if let Some(a) = age_rating.filter(|s| !s.is_empty()) {
        if !AGE_RATINGS.contains(&a) {
            return Err(AppError::Validation("age_rating must be 13|16|18|21".into()));
        }
    }
    Ok(())
}

/// Default timezone for events whose organizer doesn't specify one (launch market).
const DEFAULT_TIMEZONE: &str = "Europe/Kyiv";

/// Validate an optional IANA timezone name against the `chrono-tz` database (BR-020).
fn validate_timezone(timezone: Option<&str>) -> Result<(), AppError> {
    if let Some(t) = timezone.filter(|s| !s.is_empty()) {
        if t.parse::<chrono_tz::Tz>().is_err() {
            return Err(AppError::Validation("timezone must be a valid IANA name".into()));
        }
    }
    Ok(())
}

#[derive(Serialize, sqlx::FromRow)]
pub struct Event {
    pub id: Uuid,
    pub organizer_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub poster_url: Option<String>,
    pub category: String,
    pub city: Option<String>,
    pub venue_address: Option<String>,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub price: Decimal,
    pub currency: String,
    pub tickets_left: i32,
    pub status: String,
    pub cost_type: String,
    pub video_url: Option<String>,
    pub gallery_urls: Vec<String>,
    pub links: Vec<String>,
    pub sales_closed: bool,
    /// Geo (TASK-017 M2), resolved from the city gazetteer.
    pub country: Option<String>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    /// Minimum-age rating (P1 #77): one of 13|16|18|21, or null for unrestricted.
    pub age_rating: Option<String>,
    /// IANA timezone the event's local time is shown in (TASK-102 / BR-020). Times are stored UTC.
    pub timezone: String,
}

#[derive(Deserialize)]
pub struct CreateEventReq {
    pub title: String,
    pub description: Option<String>,
    pub category: String,
    pub city: Option<String>,
    pub venue_address: Option<String>,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub price: Decimal,
    pub currency: Option<String>,
    pub tickets_total: i32,
    pub cost_type: Option<String>,
    pub poster_url: Option<String>,
    pub video_url: Option<String>,
    #[serde(default)]
    pub gallery_urls: Vec<String>,
    #[serde(default)]
    pub links: Vec<String>,
    /// `published` (default) or `draft` — drafts are hidden until the organizer publishes.
    pub status: Option<String>,
    /// Minimum-age rating: 13|16|18|21 (empty/absent = unrestricted).
    pub age_rating: Option<String>,
    /// IANA timezone for the event's local time (default `Europe/Kyiv` when absent). TASK-102.
    pub timezone: Option<String>,
}

/// Published events, soonest first. Parameterized query (no SQL string-building from input).
pub async fn fetch_published(db: &PgPool) -> Result<Vec<Event>, sqlx::Error> {
    let sql = format!("SELECT {EVENT_COLS} FROM events WHERE status = 'published' ORDER BY starts_at ASC NULLS LAST");
    sqlx::query_as::<_, Event>(&sql).fetch_all(db).await
}

pub async fn fetch_by_organizer(db: &PgPool, organizer_id: Uuid) -> Result<Vec<Event>, sqlx::Error> {
    let sql = format!("SELECT {EVENT_COLS} FROM events WHERE organizer_id = $1 ORDER BY starts_at ASC NULLS LAST");
    sqlx::query_as::<_, Event>(&sql).bind(organizer_id).fetch_all(db).await
}

pub async fn insert_event(db: &PgPool, organizer_id: Uuid, req: &CreateEventReq) -> Result<Event, sqlx::Error> {
    // Resolve country + coordinates from the city gazetteer (no external geocoding).
    let geo = crate::cities::resolve(db, req.city.as_deref()).await?;
    let sql = format!(
        "INSERT INTO events (organizer_id, title, description, category, city, venue_address, \
            starts_at, ends_at, price, currency, tickets_total, tickets_left, cost_type, status, \
            poster_url, video_url, gallery_urls, links, country, lat, lng, age_rating, timezone) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $11, $12, $20, \
            $13, $14, $15, $16, $17, $18, $19, $21, $22) \
         RETURNING {EVENT_COLS}"
    );
    sqlx::query_as::<_, Event>(&sql)
        .bind(organizer_id)
        .bind(req.title.trim())
        .bind(req.description.as_deref())
        .bind(req.category.trim())
        .bind(req.city.as_deref())
        .bind(req.venue_address.as_deref())
        .bind(req.starts_at)
        .bind(req.ends_at)
        .bind(req.price)
        .bind(
            req.currency
                .as_deref()
                .filter(|c| !c.is_empty())
                .unwrap_or_else(|| crate::currency::for_country(geo.country.as_deref())),
        )
        .bind(req.tickets_total)
        .bind(req.cost_type.as_deref().unwrap_or("paid"))
        .bind(req.poster_url.as_deref())
        .bind(req.video_url.as_deref())
        .bind(&req.gallery_urls)
        .bind(&req.links)
        .bind(geo.country)
        .bind(geo.lat)
        .bind(geo.lng)
        .bind(req.status.as_deref().unwrap_or("published"))
        .bind(req.age_rating.as_deref().filter(|s| !s.is_empty()))
        .bind(
            req.timezone
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or(DEFAULT_TIMEZONE),
        )
        .fetch_one(db)
        .await
}

#[derive(Deserialize)]
pub struct EventQuery {
    pub q: Option<String>,
    pub city: Option<String>,
    pub category: Option<String>,
    pub free: Option<bool>,
    pub sort: Option<String>,
    /// `true` includes past events (hidden from the афіша by default).
    pub past: Option<bool>,
}

/// `GET /api/events` with optional filters (`q`, `city`, `category`, `free`, `sort`).
/// All filters are parameterized; the sort key comes from a fixed allow-list (no injection).
pub async fn list_events(
    State(st): State<AppState>,
    Query(query): Query<EventQuery>,
) -> Result<Json<Vec<Event>>, AppError> {
    let order = match query.sort.as_deref() {
        Some("price") => "price ASC NULLS LAST",
        Some("price_desc") => "price DESC NULLS LAST",
        _ => "starts_at ASC NULLS LAST",
    };
    let sql = format!(
        "SELECT {EVENT_COLS} FROM events \
         WHERE status = 'published' \
           AND ($1::text IS NULL OR city = $1) \
           AND ($2::text IS NULL OR category = $2) \
           AND ($3::bool IS NULL OR ((cost_type = 'free') = $3)) \
           AND ($4::text IS NULL OR title ILIKE '%' || $4 || '%') \
           AND ($5::bool IS TRUE OR starts_at IS NULL OR starts_at >= now()) \
         ORDER BY {order}"
    );
    let none_if_empty = |s: Option<String>| s.filter(|v| !v.trim().is_empty());
    // LIKE metacharacters in the search term are matched literally.
    let escape_like = |s: String| s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    let events = sqlx::query_as::<_, Event>(&sql)
        .bind(none_if_empty(query.city))
        .bind(none_if_empty(query.category))
        .bind(query.free)
        .bind(none_if_empty(query.q).map(escape_like))
        .bind(query.past)
        .fetch_all(&st.db)
        .await?;
    Ok(Json(events))
}

/// `GET /api/events/:id` — public details of one published event (deep links,
/// refreshing the detail screen).
pub async fn get_event(
    State(st): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Result<Json<Event>, AppError> {
    let sql = format!("SELECT {EVENT_COLS} FROM events WHERE id = $1 AND status = 'published'");
    let event = sqlx::query_as::<_, Event>(&sql)
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?;
    // Top-of-funnel: a detail open (UC-015). Anonymous views carry a null user.
    crate::telemetry::log(&st.db, None, "event_view", serde_json::json!({ "event_id": id })).await;
    Ok(Json(event))
}

/// `GET /api/me/events` — the caller's own events.
pub async fn my_events(user: CurrentUser, State(st): State<AppState>) -> Result<Json<Vec<Event>>, AppError> {
    Ok(Json(fetch_by_organizer(&st.db, user.id).await?))
}

/// `POST /api/events` — organizer = the authenticated caller (never from the body).
pub async fn create_event(
    user: CurrentUser,
    State(st): State<AppState>,
    Json(req): Json<CreateEventReq>,
) -> Result<(StatusCode, Json<Event>), AppError> {
    if req.title.trim().is_empty() {
        return Err(AppError::Validation("title is required".into()));
    }
    if req.category.trim().is_empty() {
        return Err(AppError::Validation("category is required".into()));
    }
    if req.tickets_total < 0 {
        return Err(AppError::Validation("tickets must be >= 0".into()));
    }
    if req.price < Decimal::ZERO {
        return Err(AppError::Validation("price must be >= 0".into()));
    }
    if let Some(c) = req.currency.as_deref().filter(|c| !c.is_empty()) {
        if !crate::currency::is_supported(c) {
            return Err(AppError::Validation("unsupported currency".into()));
        }
    }
    if req.status.as_deref().is_some_and(|s| s != "draft" && s != "published") {
        return Err(AppError::Validation("status must be draft|published".into()));
    }
    validate_age_rating(req.age_rating.as_deref())?;
    validate_timezone(req.timezone.as_deref())?;
    // A free event needs no payment setup at all; the wall appears only where money would move.
    //
    // Both defaults matter here and both are "the money-taking one": an omitted `cost_type` is
    // `paid`, and an omitted `status` is PUBLISHED. Reading the gate off `Some("published")`
    // alone let the commonest call — a body with neither field — create a published paid event
    // with no way to be paid for it.
    let will_be_paid = req.cost_type.as_deref().unwrap_or("paid") != "free";
    let will_be_published = req.status.as_deref().unwrap_or("published") == "published";
    if will_be_published && will_be_paid {
        require_can_take_money(&st.db, user.id).await?;
    }
    let event = insert_event(&st.db, user.id, &req).await?;
    // Drafts stay private; only a published event announces itself.
    if event.status == "published" {
        announce(&st.db, user.id, &event).await;
    }
    Ok((StatusCode::CREATED, Json(event)))
}

/// Best-effort announce on publish: notify followers + matching prefs + post to the wall.
/// A failure here never fails the create/publish.
async fn announce(db: &PgPool, organizer_id: Uuid, event: &Event) {
    if let Err(e) = crate::notifications::notify_followers_new_event(db, event).await {
        tracing::warn!(error = %e, event_id = %event.id, "follower fan-out failed");
    }
    if let Err(e) = crate::notifications::notify_event_match(db, event).await {
        tracing::warn!(error = %e, event_id = %event.id, "notification fan-out failed");
    }
    if let Err(e) = crate::posts::insert_auto_post(
        db,
        organizer_id,
        "event_share",
        event.description.as_deref(),
        Some(event.id),
        None,
    )
    .await
    {
        tracing::warn!(error = %e, event_id = %event.id, "wall announcement failed");
    }
}

#[derive(Deserialize)]
pub struct UpdateEventReq {
    pub title: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub city: Option<String>,
    pub venue_address: Option<String>,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub price: Option<Decimal>,
    pub currency: Option<String>,
    pub poster_url: Option<String>,
    pub video_url: Option<String>,
    pub gallery_urls: Option<Vec<String>>,
    pub links: Option<Vec<String>>,
    /// `published` | `hidden` — the organizer's visibility switch.
    pub status: Option<String>,
    /// Pause/resume ticket sales without hiding the event.
    pub sales_closed: Option<bool>,
    /// Minimum-age rating: 13|16|18|21 (empty = unrestricted).
    pub age_rating: Option<String>,
    /// IANA timezone for the event's local time (TASK-102). Null leaves the current value.
    pub timezone: Option<String>,
}

async fn require_owner(db: &PgPool, event_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
    let organizer: Option<Option<Uuid>> = sqlx::query_scalar("SELECT organizer_id FROM events WHERE id = $1")
        .bind(event_id)
        .fetch_optional(db)
        .await?;
    match organizer {
        None => Err(AppError::NotFound),
        Some(o) if o == Some(user_id) => Ok(()),
        Some(_) => Err(AppError::Forbidden),
    }
}

/// A PUBLISHED, non-free event can take a buyer's money — so that money must have somewhere to
/// land. Under direct charges the destination is the ORGANISER's own connected account at the
/// payment provider, and until the provider says that account can accept charges, publishing
/// such an event would sell tickets whose payment has no destination (PAY-5).
///
/// Checked on every path that can PRODUCE that state, not only on creation: a draft published
/// later, or a cancelled event republished, arrive at the same place by another route, and a
/// gate on one path only is the classic hole.
async fn require_can_take_money(db: &PgPool, organizer_id: Uuid) -> Result<(), AppError> {
    // Only where money would actually move. With a provider that moves none — the test and noop
    // providers a demo stand runs on — there is no destination to be missing, and demanding a
    // connected account would put a payment wall in front of every non-payment feature. The
    // load-bearing check is the one in `orders::checkout`, which refuses a REAL charge with
    // nowhere to land; this one exists so the organiser learns about it before they advertise
    // the event rather than when the first buyer cannot pay.
    let settings = crate::payments::load_settings(db).await?;
    match crate::provider::resolve(&settings) {
        Ok(p) if !p.moves_real_money() => return Ok(()),
        // A provider that will not resolve cannot take money either; that is the operator's
        // problem to fix, and checkout says so plainly.
        Err(_) => return Ok(()),
        Ok(_) => {}
    }
    let row: Option<(bool, bool)> = sqlx::query_as("SELECT psp_charges_enabled, blocked FROM users WHERE id = $1")
        .bind(organizer_id)
        .fetch_optional(db)
        .await?;
    let (charges_enabled, blocked) = row.ok_or(AppError::NotFound)?;
    // Blocked wins over the provider's opinion: the provider knows whether the account is valid,
    // not whether WE have suspended this organiser.
    if blocked || !charges_enabled {
        return Err(AppError::Conflict(
            "щоб продавати платні квитки, підключіть виплати в кабінеті організатора".into(),
        ));
    }
    Ok(())
}

/// `PUT /api/events/:id` — owner-only partial update (mobile organizer admin).
pub async fn update_event(
    user: CurrentUser,
    State(st): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
    Json(req): Json<UpdateEventReq>,
) -> Result<Json<Event>, AppError> {
    require_owner(&st.db, id, user.id).await?;
    if req.title.as_deref().is_some_and(|t| t.trim().is_empty()) {
        return Err(AppError::Validation("title must not be blank".into()));
    }
    if req.price.is_some_and(|p| p < Decimal::ZERO) {
        return Err(AppError::Validation("price must be >= 0".into()));
    }
    // A draft can be published or hidden, but never reverted to draft — that would let an
    // owner re-trigger the publish announcement (follower fan-out + wall post) repeatedly.
    if req.status.as_deref().is_some_and(|s| s != "published" && s != "hidden") {
        return Err(AppError::Validation("status must be published|hidden".into()));
    }
    if let Some(c) = req.currency.as_deref().filter(|c| !c.is_empty()) {
        if !crate::currency::is_supported(c) {
            return Err(AppError::Validation("unsupported currency".into()));
        }
    }
    validate_age_rating(req.age_rating.as_deref())?;
    validate_timezone(req.timezone.as_deref())?;
    // Track a draft→published transition so publishing a draft announces it (once), and read the
    // event's cost type: `cost_type` is not updatable here, so the stored value is what this
    // event will still be after the update.
    let (was_draft, is_paid): (bool, bool) =
        sqlx::query_as("SELECT status = 'draft', cost_type <> 'free' FROM events WHERE id = $1")
            .bind(id)
            .fetch_one(&st.db)
            .await?;
    if req.status.as_deref() == Some("published") && is_paid {
        require_can_take_money(&st.db, user.id).await?;
    }
    // Re-resolve geo when the city changes; null fields leave the old geo untouched (COALESCE).
    let geo = match req.city.as_deref() {
        Some(_) => crate::cities::resolve(&st.db, req.city.as_deref()).await?,
        None => crate::cities::Resolved {
            country: None,
            lat: None,
            lng: None,
        },
    };
    let sql = format!(
        "UPDATE events SET \
            title = COALESCE($2, title), \
            description = COALESCE($3, description), \
            category = COALESCE($4, category), \
            city = COALESCE($5, city), \
            venue_address = COALESCE($6, venue_address), \
            starts_at = COALESCE($7, starts_at), \
            ends_at = COALESCE($8, ends_at), \
            price = COALESCE($9, price), \
            poster_url = COALESCE($10, poster_url), \
            video_url = COALESCE($11, video_url), \
            gallery_urls = COALESCE($12, gallery_urls), \
            links = COALESCE($13, links), \
            status = COALESCE($14, status), \
            sales_closed = COALESCE($15, sales_closed), \
            country = COALESCE($16, country), \
            lat = COALESCE($17, lat), \
            lng = COALESCE($18, lng), \
            currency = COALESCE($19, currency), \
            age_rating = COALESCE($20, age_rating), \
            timezone = COALESCE($21, timezone) \
         WHERE id = $1 \
         RETURNING {EVENT_COLS}"
    );
    let event = sqlx::query_as::<_, Event>(&sql)
        .bind(id)
        .bind(req.title.as_deref().map(str::trim))
        .bind(req.description.as_deref())
        .bind(req.category.as_deref().map(str::trim))
        .bind(req.city.as_deref())
        .bind(req.venue_address.as_deref())
        .bind(req.starts_at)
        .bind(req.ends_at)
        .bind(req.price)
        .bind(req.poster_url.as_deref())
        .bind(req.video_url.as_deref())
        .bind(req.gallery_urls.as_deref())
        .bind(req.links.as_deref())
        .bind(req.status.as_deref())
        .bind(req.sales_closed)
        .bind(geo.country)
        .bind(geo.lat)
        .bind(geo.lng)
        .bind(req.currency.as_deref().filter(|c| !c.is_empty()))
        .bind(req.age_rating.as_deref().filter(|s| !s.is_empty()))
        .bind(req.timezone.as_deref().filter(|s| !s.is_empty()))
        .fetch_one(&st.db)
        .await?;
    if was_draft && event.status == "published" {
        announce(&st.db, user.id, &event).await;
    }
    Ok(Json(event))
}

/// What a cancellation did, so the organiser sees the size of it rather than a silent 204.
#[derive(Serialize)]
pub struct CancelResp {
    pub tickets_returned: i64,
    pub refunds_queued: i64,
}

/// `POST /api/events/:id/cancel` — owner-only cancellation, and the mass refund that has to go
/// with it.
///
/// Cancelling used to be one `UPDATE events SET status='cancelled'` and nothing else: the
/// tickets stayed valid-looking, the orders stayed `paid`, and the escrow sweep went on walking
/// that money toward a payout for an event that never happened. Now the money comes back, and
/// it does so through the same queue as every other refund.
///
/// One transaction, and idempotent: cancelling twice queues nothing the second time, because
/// only orders still in a refundable state are picked up.
pub async fn cancel_event(
    user: CurrentUser,
    State(st): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Result<Json<CancelResp>, AppError> {
    require_owner(&st.db, id, user.id).await?;
    let mut tx = st.db.begin().await?;
    sqlx::query("UPDATE events SET status = 'cancelled', sales_closed = true WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    // Every order that actually took money. `FOR UPDATE` so a payment landing in this instant
    // cannot slip between the read and the queueing and end up with a ticket to a dead event.
    let paid: Vec<(Uuid, Option<Uuid>, rust_decimal::Decimal)> =
        sqlx::query_as("SELECT id, ticket_id, amount FROM orders WHERE event_id = $1 AND status = 'paid' FOR UPDATE")
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;

    let mut tickets_returned = 0i64;
    let mut refunds_queued = 0i64;
    for (order_id, ticket_id, amount) in paid {
        if let Some(ticket_id) = ticket_id {
            let returned =
                sqlx::query("UPDATE tickets SET status = 'returned' WHERE id = $1 AND status IN ('active','used')")
                    .bind(ticket_id)
                    .execute(&mut *tx)
                    .await?;
            tickets_returned += returned.rows_affected() as i64;
        }
        match crate::refunds::enqueue(&mut tx, order_id, amount, "подію скасовано організатором").await
        {
            Ok(true) => refunds_queued += 1,
            Ok(false) => {}
            // This order's money is already fully spoken for by an earlier refund. Skip it —
            // one settled order must not fail the cancellation of the other two hundred.
            Err(AppError::Conflict(why)) => {
                tracing::info!(%order_id, why, "cancel: nothing left to refund on this order");
            }
            Err(e) => return Err(e),
        }
        // `refund_due` on BOTH, and this is what makes a second cancellation a no-op: the order
        // stops being `paid`, so the query above no longer finds it. It also stops the money
        // counting as the organiser's revenue for an event that is not happening, and stops the
        // escrow sweep advancing it toward a payout (BR-014, BR-028).
        sqlx::query("UPDATE orders SET status = 'refund_due', escrow_status = 'refund_due' WHERE id = $1")
            .bind(order_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    tracing::info!(event_id = %id, tickets_returned, refunds_queued, "event cancelled, refunds queued");
    Ok(Json(CancelResp {
        tickets_returned,
        refunds_queued,
    }))
}

/// `DELETE /api/events/:id` — the older spelling of cancellation. Kept so existing clients do
/// not break, and routed through the same code so it can never drift into the bare status flip
/// it used to be.
pub async fn delete_event(
    user: CurrentUser,
    State(st): State<AppState>,
    path: axum::extract::Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let _ = cancel_event(user, State(st), path).await?;
    Ok(StatusCode::NO_CONTENT)
}

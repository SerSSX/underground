//! City gazetteer (TASK-017 M2): worldwide name → country + coordinates.
//! Powers geo-aware discovery. Since TASK-303 G1 it also learns cities on demand from the
//! geocoder answers cached by `geo.rs`, and carries ISO country codes and IANA zones.

use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::state::AppState;

#[derive(Serialize, sqlx::FromRow)]
pub struct City {
    pub name: String,
    pub country: String,
    pub lat: f64,
    pub lng: f64,
    pub country_code: Option<String>,
    pub timezone: Option<String>,
    /// `seed` (shipped with the product) or `osm` (learned from a geocoder answer).
    pub source: String,
}

/// Resolved location for an event/user, derived from the gazetteer by city name.
#[derive(Default, Clone, sqlx::FromRow)]
pub struct Resolved {
    pub country: Option<String>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    /// ISO code and IANA zone (TASK-303 G1); NULL on rows that predate them.
    pub country_code: Option<String>,
    pub timezone: Option<String>,
    #[sqlx(default)]
    pub source: Option<String>,
}

/// Look up the gazetteer row a client's city name refers to; all-`None` when there is none.
///
/// A seeded row answers a bare name, as always. A learned (`osm`) row answers only a client whose
/// `country_code` matches it, or whose `point` lies within 100 km of it: whoever learned a name
/// first does not decide what that name means for everyone else (TASK-303 G1 fix batch, item 2).
pub async fn resolve(
    db: &PgPool,
    city: Option<&str>,
    country_code: Option<&str>,
    point: Option<(f64, f64)>,
) -> Result<Resolved, sqlx::Error> {
    let Some(city) = city.map(str::trim).filter(|c| !c.is_empty()) else {
        return Ok(Resolved::default());
    };
    let rows: Vec<Resolved> = sqlx::query_as(
        "SELECT country, lat, lng, country_code, timezone, source FROM cities WHERE lower(name) = lower($1) LIMIT 50",
    )
    .bind(city)
    .fetch_all(db)
    .await?;
    Ok(pick_row(rows, country_code, point).unwrap_or_default())
}

fn pick_row(rows: Vec<Resolved>, country_code: Option<&str>, point: Option<(f64, f64)>) -> Option<Resolved> {
    let cc = country_code
        .map(|c| c.trim().to_ascii_uppercase())
        .filter(|c| !c.is_empty());
    let distance = |r: &Resolved| match (point, r.lat.zip(r.lng)) {
        (Some((a, b)), Some((c, d))) => crate::tz::haversine_km(a, b, c, d),
        _ => f64::INFINITY,
    };
    let seeded = |r: &Resolved| r.source.as_deref() == Some("seed");
    rows.into_iter()
        .filter(|r| match &cc {
            Some(cc) => r.country_code.as_deref() == Some(cc.as_str()),
            None => seeded(r) || distance(r) <= SAME_PLACE_KM,
        })
        .min_by(|a, b| {
            let rank = |r: &Resolved| (if seeded(r) { 0.0_f64 } else { 1.0 }, distance(r));
            let (ra, rb) = (rank(a), rank(b));
            ra.1.total_cmp(&rb.1).then(ra.0.total_cmp(&rb.0))
        })
}

/// What a client said about a place: the city text and, from a chosen geocoder suggestion,
/// a point, a country name and an ISO code. All optional; values are already validated.
/// `learner` is the signed-in user an unknown city may be learned for (none: never learned).
#[derive(Default)]
pub struct PlaceInput<'a> {
    pub city: Option<&'a str>,
    pub point: Option<(f64, f64)>,
    pub country: Option<&'a str>,
    pub country_code: Option<&'a str>,
    pub learner: Option<Uuid>,
}

/// Where an event is: what [`locate`] settled on. `timezone` is `None` when it cannot be told.
#[derive(Default, Debug, PartialEq)]
pub struct Located {
    pub country: Option<String>,
    pub country_code: Option<String>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub timezone: Option<String>,
}

/// A client point farther than this from the named city's gazetteer row does not inherit that
/// row's country or zone, and does not adopt a learned row.
const SAME_PLACE_KM: f64 = 100.0;
/// The nearest gazetteer city lends its zone to a point only within this distance.
const ZONE_NEIGHBOUR_KM: f64 = 300.0;

/// Settle an event's country, coordinates and time zone (TASK-303 G1). A point the client sent
/// wins over the name lookup; a city missing from the gazetteer is learned only from a city the
/// geocoder really returned ([`crate::geo::cached_city`]), never from the free text alone.
pub async fn locate(db: &PgPool, input: &PlaceInput<'_>) -> Result<Located, sqlx::Error> {
    let mut row = resolve(db, input.city, input.country_code, input.point).await?;
    if row.lat.is_none() {
        if let Some(learned) = learn(db, input).await? {
            row = learned;
        }
    }
    let row_point = row.lat.zip(row.lng);
    // The row describes the client's point only when it is the same place.
    let row_fits = match (input.point, row_point) {
        (Some((a, b)), Some((c, d))) => crate::tz::haversine_km(a, b, c, d) <= SAME_PLACE_KM,
        (None, _) => true,
        (Some(_), None) => false,
    };
    let point = input.point.or(row_point);
    let country_code = input
        .country_code
        .map(str::to_ascii_uppercase)
        .or(row.country_code.filter(|_| row_fits));
    let timezone = match point {
        Some(_) if input.point.is_none() && row.timezone.is_some() => row.timezone,
        Some((lat, lng)) => zone_at(db, country_code.as_deref(), lat, lng).await?,
        None => None,
    };
    Ok(Located {
        country: input.country.map(String::from).or(row.country.filter(|_| row_fits)),
        country_code,
        lat: point.map(|p| p.0),
        lng: point.map(|p| p.1),
        timezone,
    })
}

/// Learn the client's city from a geocoder answer, for a signed-in user under a daily cap, and
/// only with a point or a country code to tie the name to (a bare name never qualifies).
async fn learn(db: &PgPool, input: &PlaceInput<'_>) -> Result<Option<Resolved>, sqlx::Error> {
    let name = input.city.map(str::trim).filter(|c| c.chars().count() >= 2);
    let (Some(name), Some(learner)) = (name, input.learner) else {
        return Ok(None);
    };
    if input.point.is_none() && input.country_code.is_none() {
        return Ok(None);
    }
    let Some(place) = crate::geo::cached_city(db, name, input.country_code, input.point).await? else {
        return Ok(None);
    };
    if learned_today(db, learner).await? >= crate::ratelimit::env_limit("CITY_LEARN_PER_DAY", 20) {
        tracing::info!(user_id = %learner, "city learning cap reached");
        return Ok(None);
    }
    add_from_place(db, &place, learner).await?;
    let found = resolve(db, place.name_default.as_deref(), place.country_code.as_deref(), None).await?;
    Ok(found.lat.is_some().then_some(found))
}

async fn learned_today(db: &PgPool, user: Uuid) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT count(*) FROM cities WHERE learned_by = $1 AND learned_at > now() - interval '1 day'")
        .bind(user)
        .fetch_one(db)
        .await
}

/// The zone an event gets when nothing else is known (the launch market).
pub const DEFAULT_TIMEZONE: &str = "Europe/Kyiv";

/// The zone an event is stored with: the organiser's explicit zone, else the place's, else the
/// default. The one rule `POST /api/events` and `GET /api/geo/timezone` share (TASK-303 G2).
pub fn event_zone(explicit: Option<&str>, located: Option<String>) -> String {
    explicit
        .map(str::trim)
        .filter(|z| !z.is_empty())
        .map(String::from)
        .or(located)
        .unwrap_or_else(|| DEFAULT_TIMEZONE.to_string())
}

/// The zone a new event at this point (and country) gets when it sends no zone: the same
/// [`locate`] + [`event_zone`] path as `POST /api/events` with no city text.
pub async fn zone_for_point(db: &PgPool, point: (f64, f64), country_code: Option<&str>) -> Result<String, sqlx::Error> {
    let place = PlaceInput {
        point: Some(point),
        country_code,
        ..PlaceInput::default()
    };
    Ok(event_zone(None, locate(db, &place).await?.timezone))
}

/// The zone at a point: the boundary polygons, else the country's zone table, else the nearest
/// gazetteer city with a zone (within 300 km), else `None`.
pub async fn zone_at(
    db: &PgPool,
    country_code: Option<&str>,
    lat: f64,
    lng: f64,
) -> Result<Option<String>, sqlx::Error> {
    if let Some(zone) = crate::tz::zone_for_point(country_code, lat, lng) {
        return Ok(Some(zone.to_string()));
    }
    let near: Vec<(String, f64, f64)> = sqlx::query_as(
        "SELECT timezone, lat, lng FROM cities \
         WHERE timezone IS NOT NULL AND lat BETWEEN $1 - 3 AND $1 + 3 AND lng BETWEEN $2 - 6 AND $2 + 6",
    )
    .bind(lat)
    .bind(lng)
    .fetch_all(db)
    .await?;
    Ok(near
        .into_iter()
        .map(|(zone, c_lat, c_lng)| (crate::tz::haversine_km(lat, lng, c_lat, c_lng), zone))
        .filter(|(d, _)| *d <= ZONE_NEIGHBOUR_KM)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, zone)| zone))
}

/// Add a geocoder `city` result to the gazetteer (source `osm`) under its default OSM name,
/// recording who learned it. One row per (name, country): an existing one is left alone.
pub async fn add_from_place(db: &PgPool, place: &crate::geo::GeoPlace, learner: Uuid) -> Result<bool, sqlx::Error> {
    let name = place.name_default.as_deref().map(str::trim).unwrap_or("");
    let country = place.country.as_deref().map(str::trim).unwrap_or("");
    let Some(cc) = place.country_code.as_deref() else {
        return Ok(false);
    };
    if place.kind != "city" || !(2..=100).contains(&name.chars().count()) || country.is_empty() {
        return Ok(false);
    }
    let zone = crate::tz::zone_for_point(Some(cc), place.lat, place.lng);
    let done = sqlx::query(
        "INSERT INTO cities (name, country, lat, lng, country_code, timezone, source, learned_by, learned_at) \
         VALUES ($1, $2, $3, $4, $5, $6, 'osm', $7, now()) ON CONFLICT DO NOTHING",
    )
    .bind(name)
    .bind(country)
    .bind(place.lat)
    .bind(place.lng)
    .bind(cc)
    .bind(zone)
    .bind(learner)
    .execute(db)
    .await?;
    Ok(done.rows_affected() == 1)
}

#[derive(Deserialize)]
pub struct CityQuery {
    pub q: Option<String>,
}

/// `GET /api/cities?q=` — autocomplete for the city picker (public).
pub async fn search(State(st): State<AppState>, Query(query): Query<CityQuery>) -> Result<Json<Vec<City>>, AppError> {
    let q = query.q.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let cities: Vec<City> = sqlx::query_as(
        "SELECT name, country, lat, lng, country_code, timezone, source FROM cities \
         WHERE $1::text IS NULL OR name ILIKE $1 || '%' \
         ORDER BY name LIMIT 20",
    )
    .bind(q)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(cities))
}

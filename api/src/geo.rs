//! Address and city search (TASK-303 G1) through OpenStreetMap's Photon geocoder.
//!
//! The browser never talks to Photon itself. `GET /api/geo/search` and `GET /api/geo/reverse`
//! validate the input, build the outbound URL with a query encoder (user text never reaches the
//! host or the path), call the upstream with a hard timeout, a body size cap and no redirects,
//! normalise the answer and cache it for 7 days. A per-client rate limit and a global cap on
//! upstream calls keep us inside Photon's fair-use policy. Upstream failures are a 503 with a
//! short message; the upstream body is never passed on.

use std::sync::OnceLock;
use std::time::Duration;

use axum::extract::{Query, State};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;

use crate::error::AppError;
use crate::ratelimit::{self, ClientIp};
use crate::state::AppState;

pub const DEFAULT_GEOCODER_URL: &str = "https://photon.komoot.io";
/// Identifies us to the upstream (fair use asks for it); no personal contact data.
pub const USER_AGENT: &str = "Underground/1.0 (+https://underground-dev.65-108-87-247.sslip.io)";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
/// Largest upstream body read; a limit=8 answer is a few KB.
pub const MAX_BODY_BYTES: usize = 256 * 1024;
/// Cached answers older than this are never served, and are pruned.
pub const CACHE_TTL_SECS: i32 = 7 * 24 * 3600;
/// A cached city may name an event's city only within this distance of the event's point.
const CITY_RADIUS_KM: f64 = 50.0;
const MAX_FIELD_CHARS: usize = 200;
const UNAVAILABLE: &str = "пошук адрес тимчасово недоступний";
const CITY_TAGS: [&str; 4] = ["place:city", "place:town", "place:village", "place:hamlet"];

/// Where the geocoder lives. Production reads `GEOCODER_URL` once; tests inject a local stub
/// with an `Extension<GeoConfig>` layer on the router.
#[derive(Clone, Debug)]
pub struct GeoConfig {
    base: reqwest::Url,
    timeout: Duration,
}

impl GeoConfig {
    /// `None` unless `base` is an absolute http(s) URL with a host and no credentials, query
    /// or fragment.
    pub fn new(base: &str, timeout: Duration) -> Option<Self> {
        let url = reqwest::Url::parse(base.trim()).ok()?;
        let ok = matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none();
        ok.then_some(Self { base: url, timeout })
    }

    fn from_env() -> Self {
        let raw = std::env::var("GEOCODER_URL").unwrap_or_else(|_| DEFAULT_GEOCODER_URL.into());
        Self::new(&raw, DEFAULT_TIMEOUT).unwrap_or_else(|| {
            tracing::error!("GEOCODER_URL is not an absolute http(s) URL; using the default geocoder");
            Self::new(DEFAULT_GEOCODER_URL, DEFAULT_TIMEOUT).expect("the default geocoder URL is valid")
        })
    }

    /// The upstream endpoint `path` (a constant: "api" or "reverse") under the base URL.
    fn endpoint(&self, path: &str) -> reqwest::Url {
        let mut url = self.base.clone();
        url.path_segments_mut()
            .expect("an http(s) URL has a path")
            .pop_if_empty()
            .push(path);
        url
    }
}

fn config(ext: Option<Extension<GeoConfig>>) -> GeoConfig {
    static FROM_ENV: OnceLock<GeoConfig> = OnceLock::new();
    ext.map(|Extension(c)| c)
        .unwrap_or_else(|| FROM_ENV.get_or_init(GeoConfig::from_env).clone())
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(DEFAULT_TIMEOUT)
            .timeout(DEFAULT_TIMEOUT)
            .build()
            .expect("static HTTP client configuration")
    })
}

/// One normalised geocoder result, as the web client receives it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GeoPlace {
    pub label: String,
    pub name: Option<String>,
    pub street: Option<String>,
    pub housenumber: Option<String>,
    /// The locality: for a `city` result its own name, otherwise the city it lies in.
    pub city: Option<String>,
    pub state: Option<String>,
    pub country: Option<String>,
    /// ISO 3166-1 alpha-2, upper case.
    pub country_code: Option<String>,
    pub lat: f64,
    pub lng: f64,
    /// `city` (city/town/village/hamlet) | `address` (street or house) | `place` (anything else).
    pub kind: String,
}

#[derive(Deserialize)]
pub struct SearchParams {
    pub q: Option<String>,
    pub kind: Option<String>,
    pub lang: Option<String>,
    pub lat: Option<String>,
    pub lng: Option<String>,
}

#[derive(Deserialize)]
pub struct ReverseParams {
    pub lat: Option<String>,
    pub lng: Option<String>,
    pub lang: Option<String>,
}

/// `GET /api/geo/search?q=&kind=city|address&lang=xx&lat=&lng=` (public, rate-limited).
pub async fn search(
    State(st): State<AppState>,
    ip: ClientIp,
    cfg: Option<Extension<GeoConfig>>,
    Query(p): Query<SearchParams>,
) -> Result<Json<Vec<GeoPlace>>, AppError> {
    let q = parse_q(p.q.as_deref())?;
    let city_only = parse_kind(p.kind.as_deref())?;
    let lang = photon_lang(p.lang.as_deref())?;
    // The bias is rounded (~11 km) before it is used or sent: the exact position never leaves us.
    let bias = parse_point(p.lat.as_deref(), p.lng.as_deref())?.map(|(a, b)| (round_to(a, 1), round_to(b, 1)));
    hit_client(&st.db, &ip).await?;
    let kind = if city_only { "city" } else { "address" };
    let bias_key = bias.map_or("-".to_string(), |(a, b)| format!("{a:.1},{b:.1}"));
    let key = format!("s|{kind}|{lang}|{bias_key}|{}", q.to_lowercase());
    let cfg = config(cfg);
    let url = search_url(&cfg, &q, lang, bias, city_only);
    Ok(Json(cached_or_fetch(&st.db, &cfg, &key, url).await?))
}

/// The Photon search URL. Every user value goes through the query encoder.
pub fn search_url(cfg: &GeoConfig, q: &str, lang: &str, bias: Option<(f64, f64)>, city_only: bool) -> reqwest::Url {
    let mut url = cfg.endpoint("api");
    {
        let mut qp = url.query_pairs_mut();
        qp.append_pair("q", q)
            .append_pair("limit", "8")
            .append_pair("lang", lang);
        if let Some((lat, lng)) = bias {
            qp.append_pair("lat", &lat.to_string())
                .append_pair("lon", &lng.to_string());
        }
        if city_only {
            for tag in CITY_TAGS {
                qp.append_pair("osm_tag", tag);
            }
        }
    }
    url
}

/// `GET /api/geo/reverse?lat=&lng=&lang=xx` — names the place at a point ("near me").
pub async fn reverse(
    State(st): State<AppState>,
    ip: ClientIp,
    cfg: Option<Extension<GeoConfig>>,
    Query(p): Query<ReverseParams>,
) -> Result<Json<Vec<GeoPlace>>, AppError> {
    let (lat, lng) = parse_point(p.lat.as_deref(), p.lng.as_deref())?
        .ok_or_else(|| AppError::Validation("lat and lng are required".into()))?;
    let lang = photon_lang(p.lang.as_deref())?;
    // ~110 m: enough to name the street, and what is cached and sent upstream.
    let (lat, lng) = (round_to(lat, 3), round_to(lng, 3));
    hit_client(&st.db, &ip).await?;
    let key = format!("r|{lang}|{lat:.3},{lng:.3}");
    let cfg = config(cfg);
    let mut url = cfg.endpoint("reverse");
    url.query_pairs_mut()
        .append_pair("lat", &lat.to_string())
        .append_pair("lon", &lng.to_string())
        .append_pair("lang", lang)
        .append_pair("limit", "1");
    Ok(Json(cached_or_fetch(&st.db, &cfg, &key, url).await?))
}

async fn hit_client(db: &PgPool, ip: &ClientIp) -> Result<(), AppError> {
    let max = ratelimit::env_limit("GEO_LIMIT_PER_MIN", 60);
    ratelimit::hit(db, &format!("geo-ip:{}", ip.0), max, 60).await
}

fn parse_q(raw: Option<&str>) -> Result<String, AppError> {
    let q = raw.unwrap_or("").split_whitespace().collect::<Vec<_>>().join(" ");
    let n = q.chars().count();
    if !(2..=120).contains(&n) || q.chars().any(char::is_control) {
        return Err(AppError::Validation("q must be 2 to 120 characters".into()));
    }
    Ok(q)
}

/// `true` for `kind=city`; `address` (the default) searches everything.
fn parse_kind(raw: Option<&str>) -> Result<bool, AppError> {
    match raw.map(str::trim).filter(|s| !s.is_empty()) {
        None | Some("address") => Ok(false),
        Some("city") => Ok(true),
        Some(_) => Err(AppError::Validation("kind must be city or address".into())),
    }
}

/// Our two-letter language → the upstream's: Photon knows `en`, `de`, `fr`; anything else
/// gets `default` (names in the local language of the place).
pub fn photon_lang(raw: Option<&str>) -> Result<&'static str, AppError> {
    let Some(lang) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok("default");
    };
    if lang.len() != 2 || !lang.bytes().all(|b| b.is_ascii_lowercase()) {
        return Err(AppError::Validation("lang must be two lowercase letters".into()));
    }
    Ok(match lang {
        "en" => "en",
        "de" => "de",
        "fr" => "fr",
        _ => "default",
    })
}

fn parse_coord(raw: Option<&str>, max: f64, name: &str) -> Result<Option<f64>, AppError> {
    let Some(s) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    match s.parse::<f64>() {
        Ok(v) if v.is_finite() && v.abs() <= max => Ok(Some(v)),
        _ => Err(AppError::Validation(format!("{name} must be a number within ±{max}"))),
    }
}

fn parse_point(lat: Option<&str>, lng: Option<&str>) -> Result<Option<(f64, f64)>, AppError> {
    match (parse_coord(lat, 90.0, "lat")?, parse_coord(lng, 180.0, "lng")?) {
        (Some(a), Some(b)) => Ok(Some((a, b))),
        (None, None) => Ok(None),
        _ => Err(AppError::Validation("lat and lng go together".into())),
    }
}

fn round_to(v: f64, decimals: i32) -> f64 {
    let f = 10f64.powi(decimals);
    let r = (v * f).round() / f;
    if r == 0.0 {
        0.0 // no "-0.0" in cache keys or URLs
    } else {
        r
    }
}

async fn cached_or_fetch(
    db: &PgPool,
    cfg: &GeoConfig,
    key: &str,
    url: reqwest::Url,
) -> Result<Vec<GeoPlace>, AppError> {
    if let Some(hit) = cache_get(db, key).await? {
        return Ok(hit);
    }
    // A global cap on upstream calls; when it is reached the service is "busy" for everyone.
    let max = ratelimit::env_limit("GEO_UPSTREAM_LIMIT_PER_MIN", 300);
    ratelimit::hit(db, "geo-upstream", max, 60).await.map_err(|e| match e {
        AppError::RateLimited => AppError::Unavailable(UNAVAILABLE.into()),
        other => other,
    })?;
    let places = match tokio::time::timeout(cfg.timeout, fetch(cfg, url)).await {
        Ok(Ok(body)) => normalise(&body).ok_or_else(|| "unexpected answer shape".to_string()),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("timed out".to_string()),
    }
    .map_err(|e| {
        tracing::warn!(error = %e, "geocoder request failed");
        AppError::Unavailable(UNAVAILABLE.into())
    })?;
    cache_put(db, key, &places).await?;
    Ok(places)
}

/// GET the URL; the error is a short description that never contains the URL (user text).
async fn fetch(cfg: &GeoConfig, url: reqwest::Url) -> Result<Value, String> {
    let fail = |e: reqwest::Error| e.without_url().to_string();
    let mut resp = client().get(url).timeout(cfg.timeout).send().await.map_err(fail)?;
    if !resp.status().is_success() {
        // Includes 3xx: redirects are never followed.
        return Err(format!("status {}", resp.status().as_u16()));
    }
    if resp.content_length().is_some_and(|n| n > MAX_BODY_BYTES as u64) {
        return Err("body too large".into());
    }
    let mut body = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(fail)? {
        if body.len() + chunk.len() > MAX_BODY_BYTES {
            return Err("body too large".into());
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_| "body is not JSON".to_string())
}

/// Photon's GeoJSON → our places. `None` when the answer is not a feature collection.
pub fn normalise(body: &Value) -> Option<Vec<GeoPlace>> {
    let features = body.get("features")?.as_array()?;
    Some(features.iter().filter_map(place_from).take(8).collect())
}

fn place_from(feature: &Value) -> Option<GeoPlace> {
    let p = feature.get("properties")?;
    let coords = feature.pointer("/geometry/coordinates")?.as_array()?;
    let (lng, lat) = (coords.first()?.as_f64()?, coords.get(1)?.as_f64()?);
    if !(lat.is_finite() && lng.is_finite() && lat.abs() <= 90.0 && lng.abs() <= 180.0) {
        return None;
    }
    let text = |k: &str| p.get(k).and_then(Value::as_str).map(clean).filter(|s| !s.is_empty());
    let kind = kind_of(p);
    let name = text("name");
    let country_code = text("countrycode")
        .filter(|c| c.len() == 2 && c.bytes().all(|b| b.is_ascii_alphabetic()))
        .map(|c| c.to_ascii_uppercase());
    let mut place = GeoPlace {
        label: String::new(),
        city: if kind == "city" { name.clone() } else { text("city") },
        name,
        street: text("street"),
        housenumber: text("housenumber"),
        state: text("state"),
        country: text("country"),
        country_code,
        lat,
        lng,
        kind: kind.into(),
    };
    place.label = label_of(&place);
    (!place.label.is_empty()).then_some(place)
}

fn clean(s: &str) -> String {
    let s: String = s.chars().filter(|c| !c.is_control()).take(MAX_FIELD_CHARS).collect();
    s.trim().to_string()
}

fn kind_of(p: &Value) -> &'static str {
    let s = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or("");
    if s("osm_key") == "place" && matches!(s("osm_value"), "city" | "town" | "village" | "hamlet") {
        "city"
    } else if !s("housenumber").is_empty() || !s("street").is_empty() || matches!(s("type"), "house" | "street") {
        "address"
    } else {
        "place"
    }
}

/// "Name, Street 1, City, State, Country", skipping empty and repeated parts.
fn label_of(p: &GeoPlace) -> String {
    let street = match (&p.street, &p.housenumber) {
        (Some(s), Some(n)) => Some(format!("{s} {n}")),
        (Some(s), None) => Some(s.clone()),
        _ => None,
    };
    let candidates = [
        p.name.as_deref(),
        street.as_deref(),
        p.city.as_deref(),
        p.state.as_deref(),
        p.country.as_deref(),
    ];
    let mut parts: Vec<&str> = Vec::new();
    for part in candidates.into_iter().flatten() {
        if !parts.iter().any(|x| x.to_lowercase() == part.to_lowercase()) {
            parts.push(part);
        }
    }
    parts.join(", ")
}

async fn cache_get(db: &PgPool, key: &str) -> Result<Option<Vec<GeoPlace>>, AppError> {
    let value: Option<Value> = sqlx::query_scalar(
        "SELECT value FROM geo_cache WHERE key = $1 AND created_at > now() - ($2 * interval '1 second')",
    )
    .bind(key)
    .bind(CACHE_TTL_SECS)
    .fetch_optional(db)
    .await?;
    Ok(value.and_then(|v| serde_json::from_value(v).ok()))
}

async fn cache_put(db: &PgPool, key: &str, places: &[GeoPlace]) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO geo_cache (key, value) VALUES ($1, $2) \
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, created_at = now()",
    )
    .bind(key)
    .bind(sqlx::types::Json(places))
    .execute(db)
    .await?;
    if uuid::Uuid::new_v4().as_bytes()[0] < 4 {
        // ~1 in 64 writes, off the request path, like the rate-limit pruning; failure is harmless.
        let db = db.clone();
        tokio::spawn(async move {
            let _ = prune_cache(&db).await;
        });
    }
    Ok(())
}

/// Delete cached answers past the TTL. Returns the number of rows deleted.
pub async fn prune_cache(db: &PgPool) -> Result<u64, sqlx::Error> {
    let done = sqlx::query("DELETE FROM geo_cache WHERE created_at <= now() - ($1 * interval '1 second')")
        .bind(CACHE_TTL_SECS)
        .execute(db)
        .await?;
    Ok(done.rows_affected())
}

/// A `city` result with this name from a cached geocoder answer (one the geocoder really
/// returned within the TTL): the only source the gazetteer learns new cities from. With a
/// `near` point the nearest match within 50 km wins; without one the match must be
/// unambiguous (one country), narrowed by `country_code` when given.
pub async fn cached_city(
    db: &PgPool,
    name: &str,
    country_code: Option<&str>,
    near: Option<(f64, f64)>,
) -> Result<Option<GeoPlace>, sqlx::Error> {
    let rows: Vec<Value> = sqlx::query_scalar(
        "SELECT r FROM geo_cache c CROSS JOIN LATERAL jsonb_array_elements(c.value) AS r \
         WHERE c.created_at > now() - ($1 * interval '1 second') \
           AND r->>'kind' = 'city' AND lower(r->>'name') = lower($2) \
           AND ($3::text IS NULL OR r->>'country_code' = upper($3)) \
         LIMIT 100",
    )
    .bind(CACHE_TTL_SECS)
    .bind(name.trim())
    .bind(country_code.map(str::trim).filter(|c| !c.is_empty()))
    .fetch_all(db)
    .await?;
    let places: Vec<GeoPlace> = rows
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();
    Ok(pick_city(places, near))
}

fn pick_city(places: Vec<GeoPlace>, near: Option<(f64, f64)>) -> Option<GeoPlace> {
    if let Some((lat, lng)) = near {
        return places
            .into_iter()
            .map(|p| (crate::tz::haversine_km(lat, lng, p.lat, p.lng), p))
            .filter(|(d, _)| *d <= CITY_RADIUS_KM)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, p)| p);
    }
    let first = places.first()?;
    let one_place = places.iter().all(|p| {
        p.country_code == first.country_code
            && crate::tz::haversine_km(p.lat, p.lng, first.lat, first.lng) <= CITY_RADIUS_KM
    });
    one_place.then(|| first.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cfg(base: &str) -> GeoConfig {
        GeoConfig::new(base, DEFAULT_TIMEOUT).unwrap()
    }

    #[test]
    fn config_accepts_only_plain_http_urls() {
        assert!(GeoConfig::new("https://photon.komoot.io", DEFAULT_TIMEOUT).is_some());
        assert!(GeoConfig::new("http://127.0.0.1:9", DEFAULT_TIMEOUT).is_some());
        for bad in [
            "ftp://x.io",
            "photon.komoot.io",
            "https://u:p@x.io",
            "https://x.io/?a=1",
            "https://x.io/#f",
            "",
        ] {
            assert!(GeoConfig::new(bad, DEFAULT_TIMEOUT).is_none(), "{bad}");
        }
    }

    #[test]
    fn user_text_stays_inside_one_query_parameter() {
        let q = "a&lang=xx#frag/../../evil?x=1 Київ ü";
        let url = search_url(&cfg("https://photon.komoot.io"), q, "en", Some((50.4, 30.5)), true);
        assert_eq!(url.host_str(), Some("photon.komoot.io"));
        assert_eq!(url.path(), "/api");
        assert_eq!(url.fragment(), None);
        let pairs: Vec<(String, String)> = url.query_pairs().into_owned().collect();
        let qs: Vec<_> = pairs.iter().filter(|(k, _)| k == "q").collect();
        assert_eq!(qs.len(), 1);
        assert_eq!(qs[0].1, q);
        let langs: Vec<_> = pairs
            .iter()
            .filter(|(k, _)| k == "lang")
            .map(|(_, v)| v.as_str())
            .collect();
        assert_eq!(langs, ["en"], "[SEC] no parameter smuggled through q");
        assert_eq!(pairs.iter().filter(|(k, _)| k == "osm_tag").count(), 4);
    }

    #[test]
    fn endpoint_keeps_a_base_path() {
        let url = search_url(&cfg("http://127.0.0.1:9/photon/"), "kyiv", "default", None, false);
        assert_eq!(url.path(), "/photon/api");
        assert!(!url.query().unwrap().contains("osm_tag"));
    }

    #[test]
    fn languages_map_to_the_upstream_set() {
        assert_eq!(photon_lang(Some("en")).unwrap(), "en");
        assert_eq!(photon_lang(Some("fr")).unwrap(), "fr");
        assert_eq!(photon_lang(Some("uk")).unwrap(), "default");
        assert_eq!(photon_lang(Some("it")).unwrap(), "default", "Photon rejects it");
        assert_eq!(photon_lang(None).unwrap(), "default");
        for bad in ["EN", "eng", "e", "1x", "е́"] {
            assert!(photon_lang(Some(bad)).is_err(), "{bad}");
        }
    }

    #[test]
    fn normalises_photon_features() {
        let body = json!({"type": "FeatureCollection", "features": [
            {"properties": {"osm_key": "place", "osm_value": "city", "type": "city", "name": "Lisbon",
                "country": "Portugal", "countrycode": "pt"}, "geometry": {"coordinates": [-9.1366, 38.7078]}},
            {"properties": {"osm_key": "building", "type": "house", "street": "вулиця Бориса Грінченка",
                "housenumber": "3-А", "name": "АТП", "city": "Київ", "country": "Україна", "countrycode": "UA"},
             "geometry": {"coordinates": [30.5203, 50.4502]}},
            {"properties": {"osm_key": "tourism", "name": "Museum\u{0007}", "country": "X"},
             "geometry": {"coordinates": [1.0, 2.0]}},
            {"properties": {"name": "Broken"}, "geometry": {"coordinates": [500.0, 2.0]}},
            {"properties": {"name": "No geometry"}}
        ]});
        let places = normalise(&body).unwrap();
        assert_eq!(places.len(), 3, "invalid coordinates are dropped");
        assert_eq!(places[0].kind, "city");
        assert_eq!(places[0].city.as_deref(), Some("Lisbon"));
        assert_eq!(places[0].country_code.as_deref(), Some("PT"));
        assert_eq!((places[0].lat, places[0].lng), (38.7078, -9.1366));
        assert_eq!(places[0].label, "Lisbon, Portugal");
        assert_eq!(places[1].kind, "address");
        assert_eq!(places[1].label, "АТП, вулиця Бориса Грінченка 3-А, Київ, Україна");
        assert_eq!(places[2].kind, "place");
        assert_eq!(places[2].name.as_deref(), Some("Museum"), "control characters stripped");
        assert!(normalise(&json!({"error": "x"})).is_none());
    }

    fn city(cc: &str, lat: f64, lng: f64) -> GeoPlace {
        GeoPlace {
            label: "Lisbon".into(),
            name: Some("Lisbon".into()),
            street: None,
            housenumber: None,
            city: Some("Lisbon".into()),
            state: None,
            country: None,
            country_code: Some(cc.into()),
            lat,
            lng,
            kind: "city".into(),
        }
    }

    #[test]
    fn a_city_is_picked_only_when_unambiguous_or_near() {
        let pt = city("PT", 38.71, -9.14);
        let us = city("US", 41.92, -91.39);
        assert_eq!(
            pick_city(vec![pt.clone(), us.clone()], None),
            None,
            "two countries, no point"
        );
        assert_eq!(pick_city(vec![pt.clone()], None), Some(pt.clone()));
        assert_eq!(pick_city(vec![us.clone(), pt.clone()], Some((38.75, -9.2))), Some(pt));
        assert_eq!(pick_city(vec![us], Some((50.0, 30.0))), None, "too far from the point");
        assert_eq!(pick_city(vec![], None), None);
    }
}

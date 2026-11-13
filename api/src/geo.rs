//! Address and city search (TASK-303 G1) through OpenStreetMap's Photon geocoder.
//!
//! The browser never talks to Photon itself. `GET /api/geo/search` and `GET /api/geo/reverse`
//! validate the input, build the outbound URL with a query encoder (user text never reaches the
//! host or the path), call the upstream with a hard timeout, a body size cap and no redirects,
//! normalise the answer and cache it (searches 7 days, reverse 24 h, empty answers 1 h). Per-client
//! limits (all requests, and cache misses) and a global cap on upstream calls keep us inside
//! Photon's fair-use policy. Upstream failures are a 503 with a short message; the upstream body is
//! never passed on.
//!
//! Every place carries its default OSM name (`name_default`, `city_default`), the one spelling
//! the web stores and the gazetteer learns, whatever language the visitor searched in.

use std::net::IpAddr;
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
/// Fallback for the User-Agent's contact URL when `PUBLIC_BASE_URL` is unset or not http(s).
const DEFAULT_PUBLIC_URL: &str = "https://underground-dev.65-108-87-247.sslip.io";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
/// Largest upstream body read; an answer of 12 results is a few KB.
pub const MAX_BODY_BYTES: usize = 256 * 1024;
/// How long answers are served from the cache (F5/F8).
pub const SEARCH_TTL_SECS: i32 = 7 * 24 * 3600;
pub const REVERSE_TTL_SECS: i32 = 24 * 3600;
pub const EMPTY_TTL_SECS: i32 = 3600;
/// A city the geocoder returned can be learned for this long.
pub const CITY_SEEN_TTL_SECS: i32 = 7 * 24 * 3600;
/// A seen city may name an event's city only within this distance of the event's point.
const CITY_RADIUS_KM: f64 = 50.0;
const MAX_FIELD_CHARS: usize = 200;
const RESULTS: usize = 8;
const UNAVAILABLE: &str = "пошук адрес тимчасово недоступний";
/// Cache key format version: bumped whenever the cached shape or the upstream query changes.
/// v3 (2026-11-13): city mode also asks for `place:province`, so cached city answers from before
/// (Tokyo came back without Tokyo) are not served again.
const KEY_VERSION: &str = "v3";
const CITY_TAGS: [&str; 6] = [
    "place:city",
    "place:town",
    "place:village",
    "place:hamlet",
    // Both only for the allow-listed city-states below; every other state or province is dropped.
    "place:state",
    "place:province",
];
/// OSM objects tagged `place=state` or `place=province` that people know as a city (OSM type, OSM
/// id). Tokyo-to is a prefecture-level metropolis with no `place=city` object of its own, and Photon
/// tags it `place=province` (checked live 2026-11-13). Singapore and Hong Kong are `place=city`
/// already (checked live 2026-11-13); Macau is not checked.
const CITY_STATES: [(&str, i64); 1] = [("R", 1_543_125)];

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

/// `Underground/1.0 (+<public URL>)` — identifies us to the upstream (fair use asks for it),
/// from `PUBLIC_BASE_URL` (F14); never personal contact data.
pub fn user_agent(public_base_url: Option<&str>) -> String {
    let url = public_base_url
        .and_then(|u| reqwest::Url::parse(u.trim()).ok())
        .filter(|u| matches!(u.scheme(), "http" | "https") && u.host_str().is_some() && u.username().is_empty())
        .map(|u| u.as_str().trim_end_matches('/').to_string())
        .unwrap_or_else(|| DEFAULT_PUBLIC_URL.to_string());
    format!("Underground/1.0 (+{url})")
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(user_agent(std::env::var("PUBLIC_BASE_URL").ok().as_deref()))
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
    /// Display text in the visitor's language where the geocoder has it.
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
    /// `city` (city/town/village/hamlet, and allow-listed states) | `address` | `place`.
    pub kind: String,
    /// The OSM object: `N` | `W` | `R` and its id.
    #[serde(default)]
    pub osm_type: Option<String>,
    #[serde(default)]
    pub osm_id: Option<i64>,
    /// The OSM `name` (local language), the same whatever `lang` was asked for. Null only when the
    /// geocoder's default-language answer did not include this object.
    #[serde(default)]
    pub name_default: Option<String>,
    /// The locality's OSM `name`: what to store as an event's or a profile's city.
    #[serde(default)]
    pub city_default: Option<String>,
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
    let key = format!("{KEY_VERSION}|s|{kind}|{lang}|{bias_key}|{}", q.to_lowercase());
    let cfg = config(cfg);
    let lookup = Lookup {
        key: &key,
        lang,
        ttl: SEARCH_TTL_SECS,
        city_only,
    };
    let url = |l: &str| search_url(&cfg, &q, l, bias, city_only);
    Ok(Json(cached_or_fetch(&st.db, &cfg, &ip, &lookup, url).await?))
}

/// The Photon search URL. Every user value goes through the query encoder.
pub fn search_url(cfg: &GeoConfig, q: &str, lang: &str, bias: Option<(f64, f64)>, city_only: bool) -> reqwest::Url {
    let mut url = cfg.endpoint("api");
    {
        let mut qp = url.query_pairs_mut();
        // City mode asks for more: states that are not allow-listed are dropped afterwards.
        let limit = if city_only { "12" } else { "8" };
        qp.append_pair("q", q)
            .append_pair("limit", limit)
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
    // ~1 km (F5): enough to name the city, and all that is cached or sent upstream.
    let (lat, lng) = (round_to(lat, 2), round_to(lng, 2));
    hit_client(&st.db, &ip).await?;
    let key = format!("{KEY_VERSION}|r|{lang}|{lat:.2},{lng:.2}");
    let cfg = config(cfg);
    let lookup = Lookup {
        key: &key,
        lang,
        ttl: REVERSE_TTL_SECS,
        city_only: false,
    };
    let url = |l: &str| {
        let mut url = cfg.endpoint("reverse");
        url.query_pairs_mut()
            .append_pair("lat", &lat.to_string())
            .append_pair("lon", &lng.to_string())
            .append_pair("lang", l)
            .append_pair("limit", "1");
        url
    };
    Ok(Json(cached_or_fetch(&st.db, &cfg, &ip, &lookup, url).await?))
}

#[derive(Deserialize)]
pub struct TimezoneParams {
    pub lat: Option<String>,
    pub lng: Option<String>,
    pub country_code: Option<String>,
}

#[derive(Serialize)]
pub struct TimezoneAnswer {
    pub timezone: String,
}

/// `GET /api/geo/timezone?lat=&lng=&country_code=XX` (TASK-303 G2) — the zone a new event at this
/// point gets, so the form converts its wall-clock times in the zone the event is stored with.
/// Same rule as `POST /api/events` without a zone ([`crate::cities::zone_for_point`]); no upstream
/// call, same per-client bucket as search and reverse.
pub async fn timezone(
    State(st): State<AppState>,
    ip: ClientIp,
    Query(p): Query<TimezoneParams>,
) -> Result<Json<TimezoneAnswer>, AppError> {
    let point = parse_point(p.lat.as_deref(), p.lng.as_deref())?
        .ok_or_else(|| AppError::Validation("lat and lng are required".into()))?;
    let cc = parse_country_code(p.country_code.as_deref())?;
    hit_client(&st.db, &ip).await?;
    let timezone = crate::cities::zone_for_point(&st.db, point, cc.as_deref()).await?;
    Ok(Json(TimezoneAnswer { timezone }))
}

/// ISO 3166-1 alpha-2 in any case, upper-cased; empty means "not given".
fn parse_country_code(raw: Option<&str>) -> Result<Option<String>, AppError> {
    match raw.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(c) if c.len() == 2 && c.bytes().all(|b| b.is_ascii_alphabetic()) => Ok(Some(c.to_ascii_uppercase())),
        Some(_) => Err(AppError::Validation("country_code must be two letters".into())),
    }
}

/// The rate-limit key for a client (F7): an IPv6 client is keyed by its /64, the block one
/// subscriber gets, so rotating addresses inside it buys no fresh bucket.
pub fn ip_key(ip: &ClientIp) -> String {
    match ip.0.parse::<IpAddr>() {
        Ok(IpAddr::V6(v6)) => match v6.to_ipv4_mapped() {
            Some(v4) => v4.to_string(),
            None => {
                let s = v6.segments();
                format!("{:x}:{:x}:{:x}:{:x}::/64", s[0], s[1], s[2], s[3])
            }
        },
        _ => ip.0.clone(),
    }
}

async fn hit_client(db: &PgPool, ip: &ClientIp) -> Result<(), AppError> {
    let max = ratelimit::env_limit("GEO_LIMIT_PER_MIN", 60);
    ratelimit::hit(db, &format!("geo-ip:{}", ip_key(ip)), max, 60).await
}

fn parse_q(raw: Option<&str>) -> Result<String, AppError> {
    let q = strip_invisible(raw.unwrap_or(""))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
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

/// One logical lookup: its cache key, the visitor's upstream language and the answer's TTL.
struct Lookup<'a> {
    key: &'a str,
    lang: &'static str,
    ttl: i32,
    city_only: bool,
}

/// Serve from the cache, else ask the upstream: once in `default` (the OSM names), and when the
/// visitor's language is one Photon knows, once more in it for the labels.
async fn cached_or_fetch(
    db: &PgPool,
    cfg: &GeoConfig,
    ip: &ClientIp,
    lookup: &Lookup<'_>,
    url: impl Fn(&str) -> reqwest::Url,
) -> Result<Vec<GeoPlace>, AppError> {
    if let Some(hit) = cache_get(db, lookup.key).await? {
        return Ok(hit);
    }
    // F7: a client may cause at most this many upstream lookups a minute, inside its overall budget.
    let misses = ratelimit::env_limit("GEO_MISS_LIMIT_PER_MIN", 20);
    ratelimit::hit(db, &format!("geo-miss:{}", ip_key(ip)), misses, 60).await?;
    let localized = lookup.lang != "default";
    upstream_slot(db).await?;
    if localized {
        upstream_slot(db).await?;
    }
    let default_call = call(cfg, url("default"));
    let (defaults, labels) = if localized {
        let (d, l) = tokio::join!(default_call, call(cfg, url(lookup.lang)));
        (d.ok(), Some(l?))
    } else {
        (Some(default_call.await?), None)
    };
    let complete = defaults.is_some();
    let mut places = match labels {
        Some(labels) => with_default_names(labels, defaults.as_deref().unwrap_or(&[])),
        None => defaults.unwrap_or_default(),
    };
    if lookup.city_only {
        places.retain(|p| p.kind == "city");
    }
    places.truncate(RESULTS);
    // Without the default names the answer is still shown, but not kept.
    if complete {
        cache_put(db, lookup.key, &places, lookup.ttl).await?;
    }
    Ok(places)
}

/// One slot under the global cap on upstream calls; when it is reached the service is "busy".
async fn upstream_slot(db: &PgPool) -> Result<(), AppError> {
    let max = ratelimit::env_limit("GEO_UPSTREAM_LIMIT_PER_MIN", 120);
    ratelimit::hit(db, "geo-upstream", max, 60).await.map_err(|e| match e {
        AppError::RateLimited => AppError::Unavailable(UNAVAILABLE.into()),
        other => other,
    })
}

/// One upstream request, normalised; any failure is a 503 (logged without the URL).
async fn call(cfg: &GeoConfig, url: reqwest::Url) -> Result<Vec<GeoPlace>, AppError> {
    let default_names = url.query_pairs().any(|(k, v)| k == "lang" && v == "default");
    let result = match tokio::time::timeout(cfg.timeout, fetch(cfg, url)).await {
        Ok(Ok(body)) => normalise(&body, default_names).ok_or_else(|| "unexpected answer shape".to_string()),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("timed out".to_string()),
    };
    result.map_err(|e| {
        tracing::warn!(error = %e, "geocoder request failed");
        AppError::Unavailable(UNAVAILABLE.into())
    })
}

/// The visitor-language answer, with each object's default names taken from the default answer.
fn with_default_names(labels: Vec<GeoPlace>, defaults: &[GeoPlace]) -> Vec<GeoPlace> {
    labels
        .into_iter()
        .map(|mut p| {
            let same = defaults
                .iter()
                .find(|d| d.osm_id.is_some() && d.osm_id == p.osm_id && d.osm_type == p.osm_type);
            p.name_default = same.and_then(|d| d.name.clone());
            p.city_default = same.and_then(|d| d.city.clone());
            p
        })
        .collect()
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

/// Photon's GeoJSON → our places. `None` when the answer is not a feature collection. When
/// `default_names` (a `lang=default` answer), the names are the OSM defaults themselves.
pub fn normalise(body: &Value, default_names: bool) -> Option<Vec<GeoPlace>> {
    let features = body.get("features")?.as_array()?;
    let places = features.iter().filter_map(place_from).map(|mut p| {
        if default_names {
            p.name_default = p.name.clone();
            p.city_default = p.city.clone();
        }
        p
    });
    Some(places.take(12).collect())
}

fn place_from(feature: &Value) -> Option<GeoPlace> {
    let p = feature.get("properties")?;
    let coords = feature.pointer("/geometry/coordinates")?.as_array()?;
    let (lng, lat) = (coords.first()?.as_f64()?, coords.get(1)?.as_f64()?);
    if !(lat.is_finite() && lng.is_finite() && lat.abs() <= 90.0 && lng.abs() <= 180.0) {
        return None;
    }
    let text = |k: &str| p.get(k).and_then(Value::as_str).map(clean).filter(|s| !s.is_empty());
    let osm_type = text("osm_type").filter(|t| matches!(t.as_str(), "N" | "W" | "R"));
    let osm_id = p.get("osm_id").and_then(Value::as_i64);
    let kind = kind_of(p, osm_type.as_deref(), osm_id);
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
        osm_type,
        osm_id,
        name_default: None,
        city_default: None,
    };
    place.label = label_of(&place);
    (!place.label.is_empty()).then_some(place)
}

/// Remove the zero-width and bidirectional-control characters that make two equal-looking
/// names differ or reorder text on screen (F11): U+200B–200F, U+202A–202E, U+2066–2069, U+FEFF.
pub fn strip_invisible(s: &str) -> String {
    s.chars()
        .filter(
            |c| !matches!(*c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}'),
        )
        .collect()
}

fn clean(s: &str) -> String {
    let s: String = strip_invisible(s)
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_FIELD_CHARS)
        .collect();
    s.trim().to_string()
}

fn kind_of(p: &Value, osm_type: Option<&str>, osm_id: Option<i64>) -> &'static str {
    let s = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or("");
    let allow_listed_state = matches!(s("osm_value"), "state" | "province")
        && CITY_STATES
            .iter()
            .any(|(t, id)| Some(*t) == osm_type && Some(*id) == osm_id);
    if s("osm_key") == "place"
        && (matches!(s("osm_value"), "city" | "town" | "village" | "hamlet") || allow_listed_state)
    {
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
    let value: Option<Value> = sqlx::query_scalar("SELECT value FROM geo_cache WHERE key = $1 AND expires_at > now()")
        .bind(key)
        .fetch_optional(db)
        .await?;
    Ok(value.and_then(|v| serde_json::from_value(v).ok()))
}

/// Keep the answer (an empty one for an hour only) and remember every city in it for learning.
async fn cache_put(db: &PgPool, key: &str, places: &[GeoPlace], ttl: i32) -> Result<(), AppError> {
    let ttl = if places.is_empty() { EMPTY_TTL_SECS } else { ttl };
    sqlx::query(
        "INSERT INTO geo_cache (key, value, created_at, expires_at) \
         VALUES ($1, $2, now(), now() + ($3 * interval '1 second')) \
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, created_at = now(), expires_at = EXCLUDED.expires_at",
    )
    .bind(key)
    .bind(sqlx::types::Json(places))
    .bind(ttl)
    .execute(db)
    .await?;
    for place in places.iter().filter(|p| p.kind == "city") {
        remember_city(db, place).await?;
    }
    if uuid::Uuid::new_v4().as_bytes()[0] < 4 {
        // ~1 in 64 writes, off the request path, like the rate-limit pruning; failure is harmless.
        let db = db.clone();
        tokio::spawn(async move {
            let _ = prune_cache(&db).await;
        });
    }
    Ok(())
}

/// One row per (default name, country, ~1 km point) in `geo_city_seen` (F6).
async fn remember_city(db: &PgPool, place: &GeoPlace) -> Result<(), AppError> {
    let (Some(name), Some(cc)) = (place.name_default.as_deref(), place.country_code.as_deref()) else {
        return Ok(());
    };
    sqlx::query(
        "INSERT INTO geo_city_seen (name_key, country_code, lat_key, lng_key, place) VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (name_key, country_code, lat_key, lng_key) DO UPDATE SET place = EXCLUDED.place, seen_at = now()",
    )
    .bind(name.to_lowercase())
    .bind(cc)
    .bind((place.lat * 100.0).round() as i32)
    .bind((place.lng * 100.0).round() as i32)
    .bind(sqlx::types::Json(place))
    .execute(db)
    .await?;
    Ok(())
}

/// Delete expired answers and cities seen too long ago. Returns the number of rows deleted.
pub async fn prune_cache(db: &PgPool) -> Result<u64, sqlx::Error> {
    let answers = sqlx::query("DELETE FROM geo_cache WHERE expires_at <= now()")
        .execute(db)
        .await?;
    let cities = sqlx::query("DELETE FROM geo_city_seen WHERE seen_at <= now() - ($1 * interval '1 second')")
        .bind(CITY_SEEN_TTL_SECS)
        .execute(db)
        .await?;
    Ok(answers.rows_affected() + cities.rows_affected())
}

/// A `city` result with this default name that the geocoder really returned within the TTL:
/// the only source the gazetteer learns new cities from. A name alone never qualifies: with a
/// `near` point the nearest match within 50 km wins; with only a `country_code` the match must
/// be unambiguous inside that country.
pub async fn cached_city(
    db: &PgPool,
    name: &str,
    country_code: Option<&str>,
    near: Option<(f64, f64)>,
) -> Result<Option<GeoPlace>, sqlx::Error> {
    let cc = country_code
        .map(|c| c.trim().to_ascii_uppercase())
        .filter(|c| !c.is_empty());
    if near.is_none() && cc.is_none() {
        return Ok(None);
    }
    let rows: Vec<Value> = sqlx::query_scalar(
        "SELECT place FROM geo_city_seen \
         WHERE name_key = $1 AND ($2::text IS NULL OR country_code = $2) \
           AND seen_at > now() - ($3 * interval '1 second') \
         LIMIT 50",
    )
    .bind(strip_invisible(name.trim()).to_lowercase())
    .bind(cc)
    .bind(CITY_SEEN_TTL_SECS)
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
    fn user_agent_names_the_public_url_only() {
        assert_eq!(
            user_agent(Some("https://underground.example/")),
            "Underground/1.0 (+https://underground.example)"
        );
        let fallback = format!("Underground/1.0 (+{DEFAULT_PUBLIC_URL})");
        for bad in [
            None,
            Some(""),
            Some("ftp://x.io"),
            Some("not a url"),
            Some("https://me:pw@x.io"),
        ] {
            assert_eq!(user_agent(bad), fallback, "{bad:?}");
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
        // 6 since 2026-11-13: place:province added for Tokyo-to (was 5).
        assert_eq!(pairs.iter().filter(|(k, _)| k == "osm_tag").count(), 6);
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
    fn ipv6_clients_share_a_bucket_per_64() {
        let key = |s: &str| ip_key(&ClientIp(s.into()));
        assert_eq!(key("2001:db8:1:2:aaaa::1"), "2001:db8:1:2::/64");
        assert_eq!(key("2001:db8:1:2:ffff:1:2:3"), key("2001:db8:1:2::9"));
        assert_ne!(key("2001:db8:1:3::1"), key("2001:db8:1:2::1"));
        assert_eq!(key("::ffff:198.51.100.7"), "198.51.100.7");
        assert_eq!(key("198.51.100.7"), "198.51.100.7");
        assert_eq!(key("unknown"), "unknown");
    }

    #[test]
    fn invisible_characters_are_stripped() {
        assert_eq!(strip_invisible("Ky\u{200B}iv\u{202E}\u{2066}\u{FEFF}"), "Kyiv");
        assert_eq!(strip_invisible("Київ"), "Київ");
        assert_eq!(clean("  Lis\u{200D}bon\u{0007} "), "Lisbon");
    }

    #[test]
    fn normalises_photon_features() {
        let body = json!({"type": "FeatureCollection", "features": [
            {"properties": {"osm_type": "R", "osm_id": 5400890, "osm_key": "place", "osm_value": "city",
                "type": "city", "name": "Lisbon", "country": "Portugal", "countrycode": "pt"},
             "geometry": {"coordinates": [-9.1366, 38.7078]}},
            {"properties": {"osm_key": "building", "type": "house", "street": "вулиця Бориса Грінченка",
                "housenumber": "3-А", "name": "АТП", "city": "Київ", "country": "Україна", "countrycode": "UA"},
             "geometry": {"coordinates": [30.5203, 50.4502]}},
            {"properties": {"osm_key": "tourism", "name": "Museum\u{0007}", "country": "X"},
             "geometry": {"coordinates": [1.0, 2.0]}},
            {"properties": {"name": "Broken"}, "geometry": {"coordinates": [500.0, 2.0]}},
            {"properties": {"name": "No geometry"}}
        ]});
        let places = normalise(&body, false).unwrap();
        assert_eq!(places.len(), 3, "invalid coordinates are dropped");
        assert_eq!(places[0].kind, "city");
        assert_eq!(places[0].city.as_deref(), Some("Lisbon"));
        assert_eq!(places[0].country_code.as_deref(), Some("PT"));
        assert_eq!(
            (places[0].osm_type.as_deref(), places[0].osm_id),
            (Some("R"), Some(5_400_890))
        );
        assert_eq!(places[0].name_default, None, "a localized answer has no default names");
        assert_eq!((places[0].lat, places[0].lng), (38.7078, -9.1366));
        assert_eq!(places[0].label, "Lisbon, Portugal");
        assert_eq!(places[1].kind, "address");
        assert_eq!(places[1].label, "АТП, вулиця Бориса Грінченка 3-А, Київ, Україна");
        assert_eq!(places[2].kind, "place");
        assert_eq!(places[2].name.as_deref(), Some("Museum"), "control characters stripped");
        assert!(normalise(&json!({"error": "x"}), false).is_none());
        let defaults = normalise(&body, true).unwrap();
        assert_eq!(defaults[1].city_default.as_deref(), Some("Київ"));
        assert_eq!(defaults[0].name_default.as_deref(), Some("Lisbon"));
    }

    #[test]
    fn only_allow_listed_states_are_cities() {
        let state = |osm_id: i64, name: &str| {
            json!({"properties": {"osm_type": "R", "osm_id": osm_id, "osm_key": "place", "osm_value": "state",
                "type": "state", "name": name, "countrycode": "XX"}, "geometry": {"coordinates": [1.0, 1.0]}})
        };
        let body = json!({"features": [state(1_543_125, "東京都"), state(165_475, "California")]});
        let places = normalise(&body, true).unwrap();
        assert_eq!(places[0].kind, "city");
        assert_eq!(places[0].city.as_deref(), Some("東京都"));
        assert_eq!(places[1].kind, "place", "an ordinary state is not a city");
        // Photon tags Tokyo-to `place=province` (live, 2026-11-13): the same allow-list applies.
        let mut tokyo = state(1_543_125, "東京都");
        tokyo["properties"]["osm_value"] = json!("province");
        let mut other = state(3_795_658, "Hokkaido");
        other["properties"]["osm_value"] = json!("province");
        let places = normalise(&json!({"features": [tokyo, other]}), true).unwrap();
        assert_eq!(places[0].kind, "city", "the allow-listed province is a city");
        assert_eq!(places[1].kind, "place", "any other province is not");
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
            osm_type: Some("R".into()),
            osm_id: Some(1),
            name_default: Some("Lisboa".into()),
            city_default: Some("Lisboa".into()),
        }
    }

    #[test]
    fn default_names_come_from_the_same_osm_object() {
        let mut other = city("PT", 1.0, 1.0);
        other.osm_id = Some(2);
        other.name = Some("Elsewhere".into());
        let mut label = city("PT", 38.7, -9.1);
        label.name_default = None;
        label.city_default = None;
        let mut default = label.clone();
        default.name = Some("Lisboa".into());
        default.city = Some("Lisboa".into());
        let merged = with_default_names(vec![label.clone()], &[other, default]);
        assert_eq!(merged[0].name.as_deref(), Some("Lisbon"));
        assert_eq!(merged[0].name_default.as_deref(), Some("Lisboa"));
        let missing = with_default_names(vec![label], &[]);
        assert_eq!(missing[0].name_default, None);
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

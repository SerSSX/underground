//! TASK-303 G1: the geocoder proxy (`/api/geo/search`, `/api/geo/reverse`), its cache and rate
//! limit, cities learned on demand, and event zones from coordinates.
//!
//! The upstream is a local axum stub on 127.0.0.1, injected with an `Extension<GeoConfig>`;
//! no test reaches the real Photon. `[SEC]` marks the controls on untrusted input.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::extract::{RawQuery, State};
use axum::http::{Request, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Router};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use underground_api::geo::GeoConfig;
use underground_api::routes::router;
use underground_api::state::AppState;
use uuid::Uuid;

const SECRET: &str = "test-secret";

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Ok,
    Slow,
    Error,
    Redirect,
    Huge,
}

#[derive(Clone)]
struct Stub {
    mode: Mode,
    /// Every request the stub saw: path and raw query.
    seen: Arc<Mutex<Vec<(String, String)>>>,
}

impl Stub {
    fn calls(&self) -> Vec<(String, String)> {
        self.seen.lock().unwrap().clone()
    }
}

fn photon_answer() -> Value {
    json!({"type": "FeatureCollection", "features": [
        {"properties": {"osm_key": "place", "osm_value": "city", "type": "city", "name": "Porto",
            "state": "Porto", "country": "Portugal", "countrycode": "PT"},
         "geometry": {"type": "Point", "coordinates": [-8.6110, 41.1496]}},
        {"properties": {"osm_key": "place", "osm_value": "city", "type": "city", "name": "Tokyo",
            "country": "Japan", "countrycode": "JP"},
         "geometry": {"type": "Point", "coordinates": [139.6503, 35.6762]}},
        {"properties": {"osm_key": "building", "type": "house", "street": "Rua das Flores", "housenumber": "12",
            "city": "Porto", "country": "Portugal", "countrycode": "PT"},
         "geometry": {"type": "Point", "coordinates": [-8.6125, 41.1433]}}
    ]})
}

async fn stub_handler(State(stub): State<Stub>, uri: Uri, RawQuery(q): RawQuery) -> Response {
    stub.seen
        .lock()
        .unwrap()
        .push((uri.path().to_string(), q.unwrap_or_default()));
    if uri.path() == "/followed" {
        // Only reachable by following a redirect.
        return axum::Json(photon_answer()).into_response();
    }
    match stub.mode {
        Mode::Ok => axum::Json(photon_answer()).into_response(),
        Mode::Slow => {
            tokio::time::sleep(Duration::from_secs(3)).await;
            axum::Json(photon_answer()).into_response()
        }
        Mode::Error => (StatusCode::INTERNAL_SERVER_ERROR, "UPSTREAM-SECRET-BODY").into_response(),
        Mode::Redirect => (StatusCode::FOUND, [("location", "/followed")], "").into_response(),
        // Valid JSON, so only the size cap stands between it and a 200.
        Mode::Huge => {
            let pad = "x".repeat(underground_api::geo::MAX_BODY_BYTES);
            axum::Json(json!({"type": "FeatureCollection", "features": [], "pad": pad})).into_response()
        }
    }
}

async fn start_stub(mode: Mode) -> (Stub, String) {
    let stub = Stub {
        mode,
        seen: Arc::default(),
    };
    let app = Router::new().fallback(stub_handler).with_state(stub.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (stub, format!("http://{addr}"))
}

async fn app_with(pool: PgPool, mode: Mode) -> (Router, Stub) {
    let (stub, base) = start_stub(mode).await;
    let cfg = GeoConfig::new(&base, Duration::from_millis(400)).unwrap();
    let app = router(AppState {
        db: pool,
        jwt_secret: SECRET.into(),
    })
    .layer(Extension(cfg));
    (app, stub)
}

async fn get(app: &Router, path: &str, ip: &str) -> (StatusCode, Value) {
    let req = Request::builder()
        .uri(path)
        .header("x-forwarded-for", ip)
        .body(Body::empty())
        .unwrap();
    send(app, req).await
}

async fn send(app: &Router, req: Request<Body>) -> (StatusCode, Value) {
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::String(String::from_utf8_lossy(&bytes).into()));
    (status, body)
}

async fn cache_rows(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM geo_cache")
        .fetch_one(pool)
        .await
        .unwrap()
}

fn query_pairs(raw: &str) -> Vec<(String, String)> {
    url_pairs(&format!("http://x/?{raw}"))
}

fn url_pairs(url: &str) -> Vec<(String, String)> {
    reqwest::Url::parse(url).unwrap().query_pairs().into_owned().collect()
}

// ---------------------------------------------------------------- validation

#[sqlx::test(migrations = "./migrations")]
async fn bad_input_is_422_and_never_reaches_the_upstream(pool: PgPool) {
    let (app, stub) = app_with(pool, Mode::Ok).await;
    let long = "a".repeat(121);
    let bad = [
        "/api/geo/search".to_string(),
        "/api/geo/search?q=a".into(),
        "/api/geo/search?q=%20%20a%20%20".into(),
        format!("/api/geo/search?q={long}"),
        "/api/geo/search?q=kyiv%00x".into(),
        "/api/geo/search?q=kyiv&lang=EN".into(),
        "/api/geo/search?q=kyiv&lang=eng".into(),
        "/api/geo/search?q=kyiv&lang=u1".into(),
        "/api/geo/search?q=kyiv&kind=street".into(),
        "/api/geo/search?q=kyiv&lat=91&lng=0".into(),
        "/api/geo/search?q=kyiv&lat=10&lng=-180.5".into(),
        "/api/geo/search?q=kyiv&lat=10".into(),
        "/api/geo/search?q=kyiv&lat=abc&lng=1".into(),
        "/api/geo/search?q=kyiv&lat=NaN&lng=1".into(),
        "/api/geo/search?q=kyiv&lat=inf&lng=1".into(),
        "/api/geo/reverse".into(),
        "/api/geo/reverse?lat=50".into(),
        "/api/geo/reverse?lat=50&lng=200".into(),
        "/api/geo/reverse?lat=50&lng=30&lang=xyz".into(),
    ];
    for path in &bad {
        let (status, body) = get(&app, path, "203.0.113.1").await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{path}: {body}");
    }
    assert!(stub.calls().is_empty(), "[SEC] invalid input never goes upstream");

    let max = "%C3%A9".repeat(120);
    let (status, _) = get(&app, &format!("/api/geo/search?q={max}"), "203.0.113.1").await;
    assert_eq!(status, StatusCode::OK, "120 characters (not bytes) is the limit");
}

// ---------------------------------------------------------------- encoding

#[sqlx::test(migrations = "./migrations")]
async fn user_text_reaches_the_upstream_as_one_parameter(pool: PgPool) {
    let (app, stub) = app_with(pool, Mode::Ok).await;
    let q = "a&lang=xx#frag/../evil?x=1 Київ ü";
    let path = format!(
        "/api/geo/search?{}",
        serde_urlencode(&[
            ("q", q),
            ("kind", "city"),
            ("lang", "uk"),
            ("lat", "50.4567"),
            ("lng", "30.5234")
        ])
    );
    let (status, body) = get(&app, &path, "203.0.113.2").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let calls = stub.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "/api", "[SEC] the path is ours, not the user's");
    let pairs = query_pairs(&calls[0].1);
    let values = |k: &str| -> Vec<String> { pairs.iter().filter(|(n, _)| n == k).map(|(_, v)| v.clone()).collect() };
    assert_eq!(values("q"), [q], "[SEC] q arrives whole, as one parameter");
    assert_eq!(
        values("lang"),
        ["default"],
        "[SEC] no parameter smuggled through q; uk → default"
    );
    assert_eq!(values("limit"), ["8"]);
    assert_eq!(values("lat"), ["50.5"], "bias rounded before it leaves us");
    assert_eq!(values("lon"), ["30.5"]);
    assert_eq!(
        values("osm_tag"),
        ["place:city", "place:town", "place:village", "place:hamlet"]
    );

    let (_, places) = get(&app, "/api/geo/search?q=porto&lang=en", "203.0.113.2").await;
    let calls = stub.calls();
    assert!(!calls[1].1.contains("osm_tag"), "kind=address is unfiltered");
    assert_eq!(places[0]["kind"], "city");
    assert_eq!(places[0]["label"], "Porto, Portugal");
    assert_eq!(places[0]["country_code"], "PT");
    assert_eq!(places[0]["lat"], 41.1496);
    assert_eq!(places[0]["lng"], -8.611);
    assert_eq!(places[2]["kind"], "address");
    assert_eq!(places[2]["label"], "Rua das Flores 12, Porto, Portugal");
    assert_eq!(places[2]["city"], "Porto");
    assert!(places[2]["name"].is_null());
}

fn serde_urlencode(pairs: &[(&str, &str)]) -> String {
    let mut url = reqwest::Url::parse("http://x/").unwrap();
    url.query_pairs_mut().extend_pairs(pairs);
    url.query().unwrap().to_string()
}

// ---------------------------------------------------------------- upstream failures

#[sqlx::test(migrations = "./migrations")]
async fn upstream_timeout_is_503_and_nothing_is_cached(pool: PgPool) {
    let (app, stub) = app_with(pool.clone(), Mode::Slow).await;
    let started = std::time::Instant::now();
    let (status, body) = get(&app, "/api/geo/search?q=porto", "203.0.113.3").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "the hard timeout cut it short"
    );
    assert_eq!(body["error"], "пошук адрес тимчасово недоступний");
    assert_eq!(stub.calls().len(), 1);
    assert_eq!(cache_rows(&pool).await, 0, "a failure is never cached");
}

#[sqlx::test(migrations = "./migrations")]
async fn upstream_errors_redirects_and_huge_bodies_are_503_without_the_upstream_body(pool: PgPool) {
    for mode in [Mode::Error, Mode::Redirect, Mode::Huge] {
        let (app, stub) = app_with(pool.clone(), mode).await;
        let (status, body) = get(&app, "/api/geo/reverse?lat=41.15&lng=-8.61", "203.0.113.4").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(
            !body.to_string().contains("UPSTREAM-SECRET-BODY"),
            "[SEC] upstream body not passed on"
        );
        assert_eq!(body, json!({"error": "пошук адрес тимчасово недоступний"}));
        assert_eq!(stub.calls().len(), 1, "[SEC] a redirect is not followed");
    }
    assert_eq!(cache_rows(&pool).await, 0);
}

// ---------------------------------------------------------------- cache

#[sqlx::test(migrations = "./migrations")]
async fn a_cached_answer_does_not_call_the_upstream_again(pool: PgPool) {
    let (app, stub) = app_with(pool.clone(), Mode::Ok).await;
    let (s1, first) = get(&app, "/api/geo/search?q=Porto&lang=en", "203.0.113.5").await;
    let (s2, second) = get(&app, "/api/geo/search?q=%20porto%20%20&lang=en", "203.0.113.6").await;
    assert_eq!((s1, s2), (StatusCode::OK, StatusCode::OK));
    assert_eq!(first, second);
    assert_eq!(stub.calls().len(), 1, "same normalised query → one upstream call");
    assert_eq!(cache_rows(&pool).await, 1);

    get(&app, "/api/geo/search?q=porto&lang=de", "203.0.113.5").await;
    get(&app, "/api/geo/search?q=porto&lang=en&kind=city", "203.0.113.5").await;
    assert_eq!(stub.calls().len(), 3, "language and kind are part of the key");

    // Past the 7-day TTL: served fresh, then pruned.
    sqlx::query("UPDATE geo_cache SET created_at = now() - interval '8 days'")
        .execute(&pool)
        .await
        .unwrap();
    get(&app, "/api/geo/search?q=porto&lang=en", "203.0.113.5").await;
    assert_eq!(stub.calls().len(), 4, "an expired answer is not served");
    // (A background prune may already have run; either way only the fresh row survives.)
    underground_api::geo::prune_cache(&pool).await.unwrap();
    assert_eq!(cache_rows(&pool).await, 1);
    sqlx::query("INSERT INTO geo_cache (key, value, created_at) VALUES ('old', '[]', now() - interval '8 days')")
        .execute(&pool)
        .await
        .unwrap();
    assert!(underground_api::geo::prune_cache(&pool).await.unwrap() >= 1);
    assert_eq!(cache_rows(&pool).await, 1, "the expired row is gone");
}

#[sqlx::test(migrations = "./migrations")]
async fn reverse_rounds_the_point_and_caches(pool: PgPool) {
    let (app, stub) = app_with(pool, Mode::Ok).await;
    let (status, places) = get(
        &app,
        "/api/geo/reverse?lat=41.149612&lng=-8.611023&lang=fr",
        "203.0.113.7",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(places[0]["city"], "Porto");
    get(
        &app,
        "/api/geo/reverse?lat=41.14958&lng=-8.61099&lang=fr",
        "203.0.113.7",
    )
    .await;
    let calls = stub.calls();
    assert_eq!(calls.len(), 1, "a point ~3 m away hits the cache");
    assert_eq!(calls[0].0, "/reverse");
    let pairs = query_pairs(&calls[0].1);
    assert!(pairs.contains(&("lat".into(), "41.15".into())), "{pairs:?}");
    assert!(pairs.contains(&("lon".into(), "-8.611".into())), "{pairs:?}");
    assert!(pairs.contains(&("lang".into(), "fr".into())));
}

// ---------------------------------------------------------------- rate limit

#[sqlx::test(migrations = "./migrations")]
async fn search_is_rate_limited_per_client_ip(pool: PgPool) {
    let (app, _stub) = app_with(pool, Mode::Ok).await;
    for i in 0..60 {
        let (status, _) = get(&app, "/api/geo/search?q=porto", "198.51.100.9").await;
        assert_eq!(status, StatusCode::OK, "request {i}");
    }
    let (status, _) = get(&app, "/api/geo/reverse?lat=1&lng=1", "198.51.100.9").await;
    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "[SEC] 61st request in a minute is refused"
    );
    // The bucket is the rightmost X-Forwarded-For entry (set by our proxy), so a forged left
    // entry does not buy a fresh bucket, while another client is unaffected.
    let (status, _) = get(&app, "/api/geo/search?q=porto", "10.9.9.9, 198.51.100.9").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    let (status, _) = get(&app, "/api/geo/search?q=porto", "198.51.100.10").await;
    assert_eq!(status, StatusCode::OK);
}

// ---------------------------------------------------------------- events

async fn user(pool: &PgPool, email: &str) -> String {
    let id: Uuid = sqlx::query_scalar("INSERT INTO users (email, name) VALUES ($1, 'U') RETURNING id")
        .bind(email)
        .fetch_one(pool)
        .await
        .unwrap();
    underground_api::auth::issue_token(id, SECRET).unwrap()
}

async fn create(app: &Router, token: &str, extra: Value) -> (StatusCode, Value) {
    let mut body = json!({"title": "Gig", "category": "Rock", "price": "0", "tickets_total": 10});
    body.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    let req = Request::builder()
        .method("POST")
        .uri("/api/events")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(body.to_string()))
        .unwrap();
    send(app, req).await
}

async fn update(app: &Router, token: &str, id: &str, body: Value) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("PUT")
        .uri(format!("/api/events/{id}"))
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(body.to_string()))
        .unwrap();
    send(app, req).await
}

async fn city_rows(pool: &PgPool, name: &str) -> Vec<(String, String, Option<String>, Option<String>, String)> {
    sqlx::query_as("SELECT name, country, country_code, timezone, source FROM cities WHERE lower(name) = lower($1)")
        .bind(name)
        .fetch_all(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn a_searched_city_is_learned_once_and_free_text_never(pool: PgPool) {
    let (app, _stub) = app_with(pool.clone(), Mode::Ok).await;
    let token = user(&pool, "o@x.co").await;

    // Free text alone, even with a plausible point, does not create a gazetteer row.
    let (status, ev) = create(
        &app,
        &token,
        json!({"city": "Porto", "lat": 41.15, "lng": -8.61, "country_code": "PT"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{ev}");
    assert!(
        city_rows(&pool, "Porto").await.is_empty(),
        "[SEC] no answer cached → nothing learned"
    );
    assert_eq!(
        ev["timezone"], "Europe/Lisbon",
        "zone still follows the point and the code"
    );
    let (_, junk) = create(
        &app,
        &token,
        json!({"city": "Qwzxv Junkville", "lat": 10.5, "lng": 10.5}),
    )
    .await;
    assert!(city_rows(&pool, "Qwzxv Junkville").await.is_empty());
    assert_eq!(junk["lat"], 10.5, "the event keeps its point");

    // After a real search, the city the geocoder returned is learned, with its code and zone —
    // but not for an event whose point is far from it (2 800 km away, in Kyiv).
    get(&app, "/api/geo/search?q=porto&kind=city&lang=en", "203.0.113.8").await;
    let (_, far) = create(&app, &token, json!({"city": "Porto", "lat": 50.45, "lng": 30.52})).await;
    assert!(
        city_rows(&pool, "Porto").await.is_empty(),
        "[SEC] a far point does not confirm the name"
    );
    assert!(far["country"].is_null());
    assert_eq!(far["timezone"], "Europe/Kyiv", "zone from the nearest gazetteer city");
    let (_, ev) = create(&app, &token, json!({"city": "porto", "lat": 41.1433, "lng": -8.6125})).await;
    assert_eq!(
        city_rows(&pool, "Porto").await,
        [(
            "Porto".into(),
            "Portugal".into(),
            Some("PT".into()),
            Some("Europe/Lisbon".into()),
            "osm".into()
        )]
    );
    assert_eq!(ev["country"], "Portugal");
    assert_eq!(ev["country_code"], "PT");
    assert_eq!(ev["timezone"], "Europe/Lisbon");
    assert_eq!((ev["lat"].as_f64(), ev["lng"].as_f64()), (Some(41.1433), Some(-8.6125)));
    assert_eq!(ev["currency"], "EUR");

    // Dedupe: another spelling of the same name adds nothing; the name alone now resolves.
    let (_, again) = create(&app, &token, json!({"city": "PORTO"})).await;
    assert_eq!(city_rows(&pool, "porto").await.len(), 1);
    assert_eq!(again["lat"], 41.1496, "gazetteer coordinates");
    assert_eq!(again["timezone"], "Europe/Lisbon");
    let (_, picker) = get(&app, "/api/cities?q=Porto", "203.0.113.8").await;
    assert_eq!(picker[0]["timezone"], "Europe/Lisbon", "the city picker sees the zone");
    assert_eq!(picker[0]["country_code"], "PT");
}

#[sqlx::test(migrations = "./migrations")]
async fn an_ambiguous_name_without_a_point_is_not_learned(pool: PgPool) {
    let (app, _stub) = app_with(pool.clone(), Mode::Ok).await;
    let token = user(&pool, "o@x.co").await;
    // Two cached "Springfield" cities in different countries, no point: nothing to pick.
    let answer = json!([
        {"label": "Springfield, US", "name": "Springfield", "street": null, "housenumber": null,
         "city": "Springfield", "state": null, "country": "United States", "country_code": "US",
         "lat": 39.8, "lng": -89.64, "kind": "city"},
        {"label": "Springfield, AU", "name": "Springfield", "street": null, "housenumber": null,
         "city": "Springfield", "state": null, "country": "Australia", "country_code": "AU",
         "lat": -27.65, "lng": 152.9, "kind": "city"}
    ]);
    sqlx::query("INSERT INTO geo_cache (key, value) VALUES ('s|city|en|-|springfield', $1)")
        .bind(&answer)
        .execute(&pool)
        .await
        .unwrap();
    create(&app, &token, json!({"city": "Springfield"})).await;
    assert!(city_rows(&pool, "Springfield").await.is_empty());
    // The country code settles it.
    let (_, ev) = create(&app, &token, json!({"city": "Springfield", "country_code": "au"})).await;
    let rows = city_rows(&pool, "Springfield").await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].2.as_deref(), Some("AU"));
    assert_eq!(ev["timezone"], "Australia/Brisbane");
    assert_eq!(ev["country_code"], "AU");
}

#[sqlx::test(migrations = "./migrations")]
async fn the_event_zone_follows_its_coordinates(pool: PgPool) {
    let (app, _stub) = app_with(pool.clone(), Mode::Ok).await;
    let token = user(&pool, "o@x.co").await;
    let zone = |v: &Value| v["timezone"].as_str().unwrap().to_string();

    // By name from the gazetteer (seeded rows carry zones now).
    assert_eq!(
        zone(&create(&app, &token, json!({"city": "Київ"})).await.1),
        "Europe/Kyiv"
    );
    assert_eq!(
        zone(&create(&app, &token, json!({"city": "Tokyo"})).await.1),
        "Asia/Tokyo"
    );
    assert_eq!(
        zone(&create(&app, &token, json!({"city": "Lisbon"})).await.1),
        "Europe/Lisbon"
    );
    // By point and country code.
    let (_, tokyo) = create(
        &app,
        &token,
        json!({"lat": 35.69, "lng": 139.7, "country_code": "JP", "country": "Japan"}),
    )
    .await;
    assert_eq!(zone(&tokyo), "Asia/Tokyo");
    assert_eq!(tokyo["country"], "Japan");
    assert_eq!(tokyo["currency"], "JPY");
    // By point alone: the nearest gazetteer city with a zone (within 300 km).
    let (_, lisbon) = create(&app, &token, json!({"lat": 38.75, "lng": -9.2})).await;
    assert_eq!(zone(&lisbon), "Europe/Lisbon");
    // Nothing to go on: the old default.
    let (_, nowhere) = create(&app, &token, json!({"city": "Atlantis"})).await;
    assert_eq!(zone(&nowhere), "Europe/Kyiv");
    // An explicit zone always wins.
    let (_, explicit) = create(
        &app,
        &token,
        json!({"lat": 35.69, "lng": 139.7, "country_code": "JP", "timezone": "America/New_York"}),
    )
    .await;
    assert_eq!(zone(&explicit), "America/New_York");

    // Moving an event moves its zone; the old place's geo never lingers.
    let id = nowhere["id"].as_str().unwrap();
    let (status, moved) = update(&app, &token, id, json!({"city": "Tokyo"})).await;
    assert_eq!(status, StatusCode::OK, "{moved}");
    assert_eq!(zone(&moved), "Asia/Tokyo");
    assert_eq!(moved["country_code"], "JP");
    let (_, moved) = update(
        &app,
        &token,
        id,
        json!({"lat": 41.15, "lng": -8.61, "country_code": "PT", "country": "Portugal"}),
    )
    .await;
    assert_eq!(zone(&moved), "Europe/Lisbon");
    assert_eq!(moved["city"], "Tokyo", "the city text is the organiser's");
    assert_eq!(
        moved["country"], "Portugal",
        "not Tokyo's country: the point is 11 000 km away"
    );
    let (_, moved) = update(&app, &token, id, json!({"city": "Atlantis"})).await;
    assert!(moved["lat"].is_null(), "an unknown city clears the old coordinates");
    assert!(moved["country_code"].is_null());
    assert_eq!(zone(&moved), "Europe/Lisbon", "no new zone → the zone is kept");
    // An edit without any place leaves the geo alone.
    let (_, kyiv) = create(&app, &token, json!({"city": "Київ"})).await;
    let (_, kept) = update(&app, &token, kyiv["id"].as_str().unwrap(), json!({"title": "Renamed"})).await;
    assert_eq!(
        (kept["lat"].clone(), zone(&kept)),
        (kyiv["lat"].clone(), "Europe/Kyiv".into())
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn event_place_fields_are_validated(pool: PgPool) {
    let (app, _stub) = app_with(pool.clone(), Mode::Ok).await;
    let token = user(&pool, "o@x.co").await;
    let long = "x".repeat(81);
    let bad = [
        json!({"lat": 50.0}),
        json!({"lng": 30.0}),
        json!({"lat": 90.5, "lng": 30.0}),
        json!({"lat": 50.0, "lng": -181.0}),
        json!({"lat": 0.0, "lng": 0.0}),
        json!({"lat": "50", "lng": 30.0}),
        json!({"country_code": "UKR"}),
        json!({"country_code": "U1"}),
        json!({"country": long}),
        json!({"country": "Ukr\u{0000}aine"}),
    ];
    for extra in &bad {
        let (status, body) = create(&app, &token, extra.clone()).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{extra}: {body}");
    }
    let (_, ok) = create(&app, &token, json!({"city": "Київ"})).await;
    for extra in &bad[..5] {
        let (status, _) = update(&app, &token, ok["id"].as_str().unwrap(), extra.clone()).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "update {extra}");
    }
    let (status, _) = create(
        &app,
        &token,
        json!({"lat": -33.87, "lng": 151.21, "country_code": "", "country": ""}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "empty strings mean not given");
}

// ---------------------------------------------------------------- zone endpoint (TASK-303 G2)

/// `GET /api/geo/timezone` answers the zone an event created at that point (with no zone of its
/// own) is stored with — one rule, so the web converts times in the zone the event gets.
#[sqlx::test(migrations = "./migrations")]
async fn timezone_endpoint_matches_the_event_zone(pool: PgPool) {
    let (app, stub) = app_with(pool.clone(), Mode::Ok).await;
    let token = user(&pool, "o@x.co").await;
    let cases = [
        ("50.4501", "30.5234", "UA", "Europe/Kyiv"),
        ("35.6762", "139.6503", "jp", "Asia/Tokyo"),
        // Multi-zone countries: the nearest reference city's zone.
        ("33.4484", "-112.074", "US", "America/Phoenix"),
        ("39.7392", "-104.9903", "US", "America/Denver"),
        ("40.7128", "-74.006", "US", "America/New_York"),
        ("-31.95", "115.86", "AU", "Australia/Perth"),
        // No country: the nearest gazetteer city with a zone (Lisbon is seeded).
        ("38.75", "-9.2", "", "Europe/Lisbon"),
        // Nothing to go on (mid-Pacific, no country): the default an event gets.
        ("-30.0", "-140.0", "", "Europe/Kyiv"),
    ];
    for (i, (lat, lng, cc, want)) in cases.iter().enumerate() {
        let ip = format!("203.0.113.{}", 50 + i);
        let (status, body) = get(
            &app,
            &format!("/api/geo/timezone?lat={lat}&lng={lng}&country_code={cc}"),
            &ip,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{lat},{lng}: {body}");
        assert_eq!(body["timezone"], *want, "{lat},{lng} {cc}");
        let mut place = json!({"lat": lat.parse::<f64>().unwrap(), "lng": lng.parse::<f64>().unwrap()});
        if !cc.is_empty() {
            place["country_code"] = json!(cc);
        }
        let (status, ev) = create(&app, &token, place).await;
        assert_eq!(status, StatusCode::CREATED, "{ev}");
        assert_eq!(
            ev["timezone"], body["timezone"],
            "the event at {lat},{lng} gets the same zone"
        );
    }
    assert!(stub.calls().is_empty(), "the zone never needs the upstream geocoder");
}

#[sqlx::test(migrations = "./migrations")]
async fn timezone_endpoint_validates_and_shares_the_geo_rate_limit(pool: PgPool) {
    let (app, _stub) = app_with(pool, Mode::Ok).await;
    let bad = [
        "/api/geo/timezone",
        "/api/geo/timezone?lat=50",
        "/api/geo/timezone?lng=30",
        "/api/geo/timezone?lat=91&lng=30",
        "/api/geo/timezone?lat=50&lng=180.1",
        "/api/geo/timezone?lat=NaN&lng=30",
        "/api/geo/timezone?lat=abc&lng=30",
        "/api/geo/timezone?lat=50&lng=30&country_code=UKR",
        "/api/geo/timezone?lat=50&lng=30&country_code=U1",
        "/api/geo/timezone?lat=50&lng=30&country_code=%D0%A3%D0%90",
    ];
    for path in bad {
        let (status, body) = get(&app, path, "203.0.113.2").await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{path}: {body}");
    }
    // One bucket with search and reverse: 60 a minute per client IP in all.
    for i in 0..59 {
        let (status, _) = get(
            &app,
            "/api/geo/timezone?lat=50.45&lng=30.52&country_code=ua",
            "198.51.100.20",
        )
        .await;
        assert_eq!(status, StatusCode::OK, "request {i}");
    }
    let (status, _) = get(&app, "/api/geo/search?q=porto", "198.51.100.20").await;
    assert_eq!(status, StatusCode::OK, "the 60th request");
    let (status, _) = get(&app, "/api/geo/timezone?lat=50.45&lng=30.52", "198.51.100.20").await;
    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "[SEC] the 61st request in a minute is refused"
    );
}

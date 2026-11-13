//! TASK-303 G1 (+ fix batch): the geocoder proxy (`/api/geo/search`, `/api/geo/reverse`,
//! `/api/geo/timezone`), its cache and limits, cities learned on demand, event zones from
//! coordinates, edits that keep the place, and the sign-up language.
//!
//! The upstream is a local stub on 127.0.0.1, injected with an `Extension<GeoConfig>`; no test
//! reaches the real Photon. `[SEC]` marks the controls on untrusted input.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::extract::{RawQuery, State};
use axum::http::{Request, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Router};
use serde_json::{json, Value};
use sqlx::PgPool;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tower::ServiceExt;
use underground_api::geo::GeoConfig;
use underground_api::routes::router;
use underground_api::state::AppState;
use uuid::Uuid;

const SECRET: &str = "test-secret";
const UNAVAILABLE: &str = "пошук адрес тимчасово недоступний";

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Ok,
    Slow,
    Error,
    Redirect,
    Huge,
    Empty,
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

fn feature(osm: (&str, i64), props: Value, lng: f64, lat: f64) -> Value {
    let mut p = props;
    p["osm_type"] = json!(osm.0);
    p["osm_id"] = json!(osm.1);
    json!({"type": "Feature", "properties": p, "geometry": {"type": "Point", "coordinates": [lng, lat]}})
}

fn place_props(value: &str, name: &str, cc: &str, country: &str) -> Value {
    json!({"osm_key": "place", "osm_value": value, "type": value, "name": name, "countrycode": cc, "country": country})
}

/// What Photon answers: names follow `lang` (`default` = the OSM `name`); with `osm_tag` filters
/// only `place` objects come back, as upstream.
fn photon_answer(lang: &str, city_mode: bool) -> Value {
    let en = lang != "default";
    let mut porto = place_props("city", "Porto", "PT", "Portugal");
    porto["state"] = json!("Porto");
    let mut odessa = place_props("city", "Odessa", "US", "United States");
    odessa["state"] = json!("Texas");
    // Live Photon tags Tokyo-to `place=province` (checked 2026-11-13), not `state`.
    let tokyo = place_props(
        "province",
        if en { "Tokyo" } else { "東京都" },
        "JP",
        if en { "Japan" } else { "日本" },
    );
    let mut features = vec![
        feature(("R", 3_372_453), porto, -8.6110, 41.1496),
        feature(("R", 1_543_125), tokyo, 139.6917, 35.6895),
        feature(
            ("R", 165_475),
            place_props("state", "California", "US", "United States"),
            -119.27,
            37.27,
        ),
        // A province that is not allow-listed: never a city.
        feature(
            ("R", 3_795_658),
            place_props("province", "Hokkaido", "JP", "Japan"),
            142.79,
            43.45,
        ),
        feature(("R", 115_232), odessa, -102.3676, 31.8457),
    ];
    if !city_mode {
        let flores = json!({"osm_key": "building", "type": "house", "street": "Rua das Flores", "housenumber": "12",
            "city": if en { "Oporto" } else { "Porto" }, "country": "Portugal", "countrycode": "PT"});
        features.push(feature(("W", 4_242), flores, -8.6125, 41.1433));
    }
    json!({"type": "FeatureCollection", "features": features})
}

async fn stub_handler(State(stub): State<Stub>, uri: Uri, RawQuery(q): RawQuery) -> Response {
    let raw = q.unwrap_or_default();
    stub.seen.lock().unwrap().push((uri.path().to_string(), raw.clone()));
    let pairs = query_pairs(&raw);
    let lang = pairs
        .iter()
        .find(|(k, _)| k == "lang")
        .map_or("default".to_string(), |(_, v)| v.clone());
    let city_mode = pairs.iter().any(|(k, _)| k == "osm_tag");
    let answer = || axum::Json(photon_answer(&lang, city_mode)).into_response();
    if uri.path() == "/followed" {
        return answer(); // only reachable by following a redirect
    }
    match stub.mode {
        Mode::Ok => answer(),
        Mode::Slow => {
            tokio::time::sleep(Duration::from_secs(3)).await;
            answer()
        }
        Mode::Error => (StatusCode::INTERNAL_SERVER_ERROR, "UPSTREAM-SECRET-BODY").into_response(),
        Mode::Redirect => (StatusCode::FOUND, [("location", "/followed")], "").into_response(),
        // Valid JSON, so only the size cap stands between it and a 200.
        Mode::Huge => {
            let pad = "x".repeat(underground_api::geo::MAX_BODY_BYTES);
            axum::Json(json!({"type": "FeatureCollection", "features": [], "pad": pad})).into_response()
        }
        Mode::Empty => axum::Json(json!({"type": "FeatureCollection", "features": []})).into_response(),
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

fn app_on(pool: PgPool, base: &str) -> Router {
    let cfg = GeoConfig::new(base, Duration::from_millis(400)).unwrap();
    router(AppState {
        db: pool,
        jwt_secret: SECRET.into(),
    })
    .layer(Extension(cfg))
}

async fn app_with(pool: PgPool, mode: Mode) -> (Router, Stub) {
    let (stub, base) = start_stub(mode).await;
    (app_on(pool, &base), stub)
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

async fn json_req(app: &Router, method: &str, path: &str, token: Option<&str>, body: Value) -> (StatusCode, Value) {
    let mut b = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    send(app, b.body(Body::from(body.to_string())).unwrap()).await
}

async fn cache_rows(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM geo_cache")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn cache_ttl(pool: &PgPool) -> f64 {
    sqlx::query_scalar("SELECT EXTRACT(EPOCH FROM expires_at - created_at)::float8 FROM geo_cache")
        .fetch_one(pool)
        .await
        .unwrap()
}

fn query_pairs(raw: &str) -> Vec<(String, String)> {
    reqwest::Url::parse(&format!("http://x/?{raw}"))
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect()
}

fn urlencode(pairs: &[(&str, &str)]) -> String {
    let mut url = reqwest::Url::parse("http://x/").unwrap();
    url.query_pairs_mut().extend_pairs(pairs);
    url.query().unwrap().to_string()
}

fn lang_of(call: &(String, String)) -> String {
    query_pairs(&call.1)
        .into_iter()
        .find(|(k, _)| k == "lang")
        .map(|(_, v)| v)
        .unwrap_or_default()
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
        "/api/geo/search?q=a%E2%80%8B%E2%80%8B".into(),
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

// ---------------------------------------------------------------- encoding, default names

#[sqlx::test(migrations = "./migrations")]
async fn user_text_reaches_the_upstream_as_one_parameter(pool: PgPool) {
    let (app, stub) = app_with(pool, Mode::Ok).await;
    let q = "a&lang=xx#frag/../evil?x=1 Київ ü";
    let path = format!(
        "/api/geo/search?{}",
        urlencode(&[
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
    assert_eq!(
        calls.len(),
        1,
        "a language Photon does not know: one default-language call"
    );
    assert_eq!(calls[0].0, "/api", "[SEC] the path is ours, not the user's");
    let pairs = query_pairs(&calls[0].1);
    let values = |k: &str| -> Vec<String> { pairs.iter().filter(|(n, _)| n == k).map(|(_, v)| v.clone()).collect() };
    assert_eq!(values("q"), [q], "[SEC] q arrives whole, as one parameter");
    assert_eq!(
        values("lang"),
        ["default"],
        "[SEC] no parameter smuggled through q; uk → default"
    );
    assert_eq!(values("limit"), ["12"], "city mode asks for more, then drops states");
    assert_eq!(values("lat"), ["50.5"], "bias rounded before it leaves us");
    assert_eq!(values("lon"), ["30.5"]);
    assert_eq!(
        values("osm_tag"),
        [
            "place:city",
            "place:town",
            "place:village",
            "place:hamlet",
            "place:state",
            "place:province"
        ]
    );

    let (_, places) = get(&app, "/api/geo/search?q=porto&lang=en", "203.0.113.2").await;
    let calls = stub.calls();
    assert_eq!(calls.len(), 3, "en: one call for the labels, one for the default names");
    assert!(
        calls[1..].iter().all(|c| !c.1.contains("osm_tag")),
        "kind=address is unfiltered"
    );
    let mut langs: Vec<String> = calls[1..].iter().map(lang_of).collect();
    langs.sort();
    assert_eq!(langs, ["default", "en"]);
    assert_eq!(places[0]["kind"], "city");
    assert_eq!(places[0]["label"], "Porto, Portugal");
    assert_eq!(places[0]["country_code"], "PT");
    assert_eq!(places[0]["osm_type"], "R");
    assert_eq!(places[0]["osm_id"], 3_372_453);
    assert_eq!(places[0]["lat"], 41.1496);
    assert_eq!(places[0]["lng"], -8.611);
    let flores = places
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["kind"] == "address")
        .unwrap();
    assert_eq!(flores["label"], "Rua das Flores 12, Oporto, Portugal");
    assert_eq!(flores["city"], "Oporto", "the label language");
    assert_eq!(
        flores["city_default"], "Porto",
        "one spelling to store, whatever the language"
    );
    assert!(flores["name"].is_null());
}

#[sqlx::test(migrations = "./migrations")]
async fn allow_listed_states_are_cities_with_default_names(pool: PgPool) {
    let (app, _stub) = app_with(pool.clone(), Mode::Ok).await;
    let token = user(&pool, "o@x.co").await;
    let (status, places) = get(&app, "/api/geo/search?q=tokyo&kind=city&lang=en", "203.0.113.3").await;
    assert_eq!(status, StatusCode::OK, "{places}");
    let places = places.as_array().unwrap();
    let tokyo = places
        .iter()
        .find(|p| p["osm_id"] == 1_543_125)
        .expect("Tokyo-to is kept");
    assert_eq!(tokyo["kind"], "city");
    assert_eq!(tokyo["name"], "Tokyo");
    assert_eq!(tokyo["name_default"], "東京都");
    assert_eq!(tokyo["city_default"], "東京都");
    assert!(
        places.iter().all(|p| p["name"] != "California"),
        "an ordinary state is dropped"
    );
    assert!(
        places.iter().all(|p| p["name"] != "Hokkaido"),
        "a province that is not allow-listed is dropped"
    );
    assert!(places.iter().all(|p| p["kind"] == "city"));

    // The web stores city_default; the gazetteer learns it under that name, with the right zone.
    let (_, ev) = create(&app, &token, json!({"city": "東京都", "lat": 35.68, "lng": 139.76})).await;
    assert_eq!(ev["timezone"], "Asia/Tokyo");
    assert_eq!(ev["country_code"], "JP");
    let rows = city_rows(&pool, "東京都").await;
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].2.as_deref(), Some("JP"));
    assert_eq!(rows[0].3.as_deref(), Some("Asia/Tokyo"));
}

// ---------------------------------------------------------------- upstream failures and caps

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
    assert_eq!(body["error"], UNAVAILABLE);
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
        assert_eq!(body, json!({"error": UNAVAILABLE}));
        assert_eq!(stub.calls().len(), 1, "[SEC] a redirect is not followed");
    }
    assert_eq!(cache_rows(&pool).await, 0);
}

/// A raw HTTP server that streams a valid JSON body larger than the cap, chunked, with no
/// Content-Length: only the cap inside the read loop can stop it.
async fn start_streaming_stub() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut sock, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                let _ = sock.read(&mut buf).await;
                let head = "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\n\
                            transfer-encoding: chunked\r\nconnection: close\r\n\r\n";
                let mut out = head.as_bytes().to_vec();
                let mut chunk = |data: &[u8]| {
                    out.extend_from_slice(format!("{:x}\r\n", data.len()).as_bytes());
                    out.extend_from_slice(data);
                    out.extend_from_slice(b"\r\n");
                };
                chunk(br#"{"type":"FeatureCollection","features":[],"pad":""#);
                let pad = vec![b'x'; 32 * 1024];
                for _ in 0..(underground_api::geo::MAX_BODY_BYTES / pad.len() + 2) {
                    chunk(&pad);
                }
                chunk(br#""}"#);
                out.extend_from_slice(b"0\r\n\r\n");
                let _ = sock.write_all(&out).await;
                let _ = sock.shutdown().await;
            });
        }
    });
    format!("http://{addr}")
}

#[sqlx::test(migrations = "./migrations")]
async fn a_streamed_body_over_the_cap_is_503(pool: PgPool) {
    let base = start_streaming_stub().await;
    let app = app_on(pool.clone(), &base);
    let (status, body) = get(&app, "/api/geo/search?q=porto", "203.0.113.5").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(cache_rows(&pool).await, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn the_global_upstream_cap_answers_busy_without_calling(pool: PgPool) {
    let (app, stub) = app_with(pool.clone(), Mode::Ok).await;
    sqlx::query("INSERT INTO rate_events (bucket) SELECT 'geo-upstream' FROM generate_series(1, 120)")
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = get(&app, "/api/geo/search?q=porto", "203.0.113.6").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"], UNAVAILABLE);
    assert!(stub.calls().is_empty(), "[SEC] the cap holds before any upstream call");
}

#[sqlx::test(migrations = "./migrations")]
async fn cache_misses_have_their_own_budget_per_client_and_ipv6_64(pool: PgPool) {
    let (app, stub) = app_with(pool, Mode::Ok).await;
    for i in 0..20 {
        let ip = format!("2001:db8:7:7::{:x}", i + 1); // one /64, a fresh address each time
        let (status, _) = get(&app, &format!("/api/geo/search?q=place{i}"), &ip).await;
        assert_eq!(status, StatusCode::OK, "miss {i}");
    }
    let (status, _) = get(&app, "/api/geo/search?q=place20", "2001:db8:7:7:ffff::1").await;
    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "[SEC] the 21st upstream lookup in a minute from one /64 is refused"
    );
    assert_eq!(stub.calls().len(), 20);
    let (status, _) = get(&app, "/api/geo/search?q=place3", "2001:db8:7:7::99").await;
    assert_eq!(status, StatusCode::OK, "a cached answer is still served");
    let (status, _) = get(&app, "/api/geo/search?q=place20", "2001:db8:7:8::1").await;
    assert_eq!(status, StatusCode::OK, "another /64 has its own budget");
}

// ---------------------------------------------------------------- cache

#[sqlx::test(migrations = "./migrations")]
async fn a_cached_answer_does_not_call_the_upstream_again(pool: PgPool) {
    let (app, stub) = app_with(pool.clone(), Mode::Ok).await;
    let (s1, first) = get(&app, "/api/geo/search?q=Porto&lang=en", "203.0.113.5").await;
    let (s2, second) = get(&app, "/api/geo/search?q=%20porto%E2%80%8B%20%20&lang=en", "203.0.113.6").await;
    assert_eq!((s1, s2), (StatusCode::OK, StatusCode::OK));
    assert_eq!(first, second);
    assert_eq!(
        stub.calls().len(),
        2,
        "same normalised query → one lookup (en: 2 calls)"
    );
    assert_eq!(cache_rows(&pool).await, 1);

    get(&app, "/api/geo/search?q=porto&lang=de", "203.0.113.5").await;
    get(&app, "/api/geo/search?q=porto&lang=en&kind=city", "203.0.113.5").await;
    assert_eq!(stub.calls().len(), 6, "language and kind are part of the key");

    // Expired: served fresh, then pruned.
    sqlx::query("UPDATE geo_cache SET expires_at = now() - interval '1 second'")
        .execute(&pool)
        .await
        .unwrap();
    get(&app, "/api/geo/search?q=porto&lang=en", "203.0.113.5").await;
    assert_eq!(stub.calls().len(), 8, "an expired answer is not served");
    underground_api::geo::prune_cache(&pool).await.unwrap();
    assert_eq!(cache_rows(&pool).await, 1, "only the fresh row survives");
    let ttl = cache_ttl(&pool).await;
    assert!((ttl - 7.0 * 86400.0).abs() < 5.0, "a search is kept 7 days: {ttl}");
}

#[sqlx::test(migrations = "./migrations")]
async fn reverse_rounds_to_a_kilometre_and_keeps_a_day(pool: PgPool) {
    let (app, stub) = app_with(pool.clone(), Mode::Ok).await;
    let (status, places) = get(
        &app,
        "/api/geo/reverse?lat=41.149612&lng=-8.611023&lang=fr",
        "203.0.113.7",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(places.as_array().unwrap().iter().any(|p| p["city_default"] == "Porto"));
    get(&app, "/api/geo/reverse?lat=41.1532&lng=-8.6079&lang=fr", "203.0.113.7").await;
    let calls = stub.calls();
    assert_eq!(calls.len(), 2, "a point ~400 m away hits the cache (fr: 2 calls once)");
    assert!(calls.iter().all(|c| c.0 == "/reverse"));
    let pairs = query_pairs(&calls[0].1);
    assert!(pairs.contains(&("lat".into(), "41.15".into())), "{pairs:?}");
    assert!(pairs.contains(&("lon".into(), "-8.61".into())), "{pairs:?}");
    let ttl = cache_ttl(&pool).await;
    assert!((ttl - 86400.0).abs() < 5.0, "reverse answers are kept 24 h: {ttl}");
}

#[sqlx::test(migrations = "./migrations")]
async fn an_empty_answer_is_kept_one_hour(pool: PgPool) {
    let (app, _stub) = app_with(pool.clone(), Mode::Empty).await;
    let (status, places) = get(&app, "/api/geo/search?q=nowhere", "203.0.113.8").await;
    assert_eq!((status, places), (StatusCode::OK, json!([])));
    let ttl = cache_ttl(&pool).await;
    assert!((ttl - 3600.0).abs() < 5.0, "{ttl}");
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

async fn user_id(pool: &PgPool, email: &str) -> Uuid {
    sqlx::query_scalar("SELECT id FROM users WHERE email = $1")
        .bind(email)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn event_body(extra: Value) -> Value {
    let mut body = json!({"title": "Gig", "category": "Rock", "price": "0", "tickets_total": 10});
    body.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    body
}

async fn create(app: &Router, token: &str, extra: Value) -> (StatusCode, Value) {
    json_req(app, "POST", "/api/events", Some(token), event_body(extra)).await
}

async fn update(app: &Router, token: &str, id: &str, body: Value) -> (StatusCode, Value) {
    json_req(app, "PUT", &format!("/api/events/{id}"), Some(token), body).await
}

type CityRow = (String, String, Option<String>, Option<String>, String);

async fn city_rows(pool: &PgPool, name: &str) -> Vec<CityRow> {
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
        "[SEC] no answer seen → nothing learned"
    );
    assert_eq!(ev["timezone"], "Europe/Lisbon", "zone still follows the point");
    let (_, junk) = create(
        &app,
        &token,
        json!({"city": "Qwzxv Junkville", "lat": 10.5, "lng": 10.5}),
    )
    .await;
    assert!(city_rows(&pool, "Qwzxv Junkville").await.is_empty());
    assert_eq!(junk["lat"], 10.5, "the event keeps its point");

    // After a real search the city is learnable — but not for an event whose point is far from it.
    get(&app, "/api/geo/search?q=porto&kind=city&lang=en", "203.0.113.8").await;
    let (_, far) = create(&app, &token, json!({"city": "Porto", "lat": 50.45, "lng": 30.52})).await;
    assert!(
        city_rows(&pool, "Porto").await.is_empty(),
        "[SEC] a far point does not confirm the name"
    );
    assert!(far["country"].is_null());
    assert_eq!(far["timezone"], "Europe/Kyiv", "zone from the point");
    // Nor from the name alone.
    create(&app, &token, json!({"city": "Porto"})).await;
    assert!(
        city_rows(&pool, "Porto").await.is_empty(),
        "[SEC] a bare name never learns"
    );

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
    let learned_by: Option<Uuid> = sqlx::query_scalar("SELECT learned_by FROM cities WHERE name = 'Porto'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        learned_by,
        Some(user_id(&pool, "o@x.co").await),
        "who learned it is recorded"
    );

    // Another spelling with the same country adds nothing and resolves to the learned row.
    let (_, again) = create(&app, &token, json!({"city": "PORTO", "country_code": "pt"})).await;
    assert_eq!(city_rows(&pool, "porto").await.len(), 1);
    assert_eq!(again["lat"], 41.1496, "gazetteer coordinates");
    assert_eq!(again["timezone"], "Europe/Lisbon");
    let (_, picker) = get(&app, "/api/cities?q=Porto", "203.0.113.8").await;
    assert_eq!(picker[0]["timezone"], "Europe/Lisbon", "the city picker sees the zone");
    assert_eq!(picker[0]["country_code"], "PT");
    assert_eq!(picker[0]["source"], "osm");
}

#[sqlx::test(migrations = "./migrations")]
async fn a_learned_name_does_not_answer_for_everyone(pool: PgPool) {
    let (app, _stub) = app_with(pool.clone(), Mode::Ok).await;
    let a = user(&pool, "a@x.co").await;
    let b = user(&pool, "b@x.co").await;
    get(&app, "/api/geo/search?q=odessa&kind=city", "203.0.113.9").await;
    // A learns Odessa, Texas from the answer and a point there.
    let (_, ev) = create(&app, &a, json!({"city": "Odessa", "lat": 31.85, "lng": -102.37})).await;
    assert_eq!(ev["country_code"], "US");
    assert_eq!(ev["timezone"], "America/Chicago");
    assert_eq!(city_rows(&pool, "Odessa").await.len(), 1);

    // B names "Odessa" with nothing else: no US geo, zone or currency.
    let (_, bare) = create(&app, &b, json!({"city": "Odessa"})).await;
    assert!(bare["lat"].is_null(), "[SEC] {bare}");
    assert!(bare["country_code"].is_null());
    assert_eq!(bare["timezone"], "Europe/Kyiv");
    assert_eq!(bare["currency"], "UAH");
    // Nor as B's home city for prefs and discovery.
    let (_, prefs) = json_req(
        &app,
        "PUT",
        "/api/me/prefs",
        Some(&b),
        json!({"categories": ["Rock"], "city": "Odessa"}),
    )
    .await;
    assert!(prefs["country"].is_null(), "{prefs}");
    let (_, feed) = json_req(&app, "GET", "/api/discover", Some(&b), json!({})).await;
    assert!(
        feed.as_array().unwrap().iter().all(|e| e["distance_km"].is_null()),
        "{feed}"
    );

    // B with the country code means that Odessa.
    let (_, us) = create(&app, &b, json!({"city": "Odessa", "country_code": "US"})).await;
    assert_eq!(us["country_code"], "US");
    assert_eq!(us["timezone"], "America/Chicago");
    assert_eq!(us["currency"], "USD");
    let (_, prefs) = json_req(
        &app,
        "PUT",
        "/api/me/prefs",
        Some(&b),
        json!({"categories": ["Rock"], "city": "Odessa", "country_code": "us"}),
    )
    .await;
    assert_eq!(prefs["country"], "United States");
    assert_eq!(prefs["country_code"], "US");
    let (_, feed) = json_req(&app, "GET", "/api/discover", Some(&b), json!({})).await;
    let mine = feed.as_array().unwrap().iter().find(|e| e["id"] == ev["id"]).unwrap();
    assert!(mine["distance_km"].as_f64().unwrap() < 5.0, "{mine}");
}

#[sqlx::test(migrations = "./migrations")]
async fn learning_is_capped_per_user_per_day(pool: PgPool) {
    let (app, _stub) = app_with(pool.clone(), Mode::Ok).await;
    let token = user(&pool, "o@x.co").await;
    sqlx::query(
        "INSERT INTO cities (name, country, lat, lng, country_code, source, learned_by, learned_at) \
         SELECT 'c' || g, 'X', 1, 1, 'XX', 'osm', $1, now() FROM generate_series(1, 20) g",
    )
    .bind(user_id(&pool, "o@x.co").await)
    .execute(&pool)
    .await
    .unwrap();
    get(&app, "/api/geo/search?q=porto&kind=city", "203.0.113.10").await;
    let (_, ev) = create(&app, &token, json!({"city": "Porto", "lat": 41.15, "lng": -8.61})).await;
    assert!(
        city_rows(&pool, "Porto").await.is_empty(),
        "[SEC] the 21st city in a day is not learned"
    );
    assert_eq!(ev["lat"], 41.15, "the event itself is unaffected");
}

#[sqlx::test(migrations = "./migrations")]
async fn an_ambiguous_name_without_a_point_is_not_learned(pool: PgPool) {
    let (app, _stub) = app_with(pool.clone(), Mode::Ok).await;
    let token = user(&pool, "o@x.co").await;
    // Two "Springfield" cities the geocoder returned, in different countries.
    for (cc, lat, lng) in [("US", 39.8, -89.64), ("AU", -27.65, 152.9)] {
        let place = json!({"label": "Springfield", "name": "Springfield", "street": null, "housenumber": null,
            "city": "Springfield", "state": null, "country": cc, "country_code": cc, "lat": lat, "lng": lng,
            "kind": "city", "osm_type": "R", "osm_id": 1, "name_default": "Springfield",
            "city_default": "Springfield"});
        sqlx::query(
            "INSERT INTO geo_city_seen (name_key, country_code, lat_key, lng_key, place) \
             VALUES ('springfield', $1, $2, $3, $4)",
        )
        .bind(cc)
        .bind((lat * 100.0) as i32)
        .bind((lng * 100.0) as i32)
        .bind(&place)
        .execute(&pool)
        .await
        .unwrap();
    }
    create(&app, &token, json!({"city": "Springfield"})).await;
    assert!(city_rows(&pool, "Springfield").await.is_empty());
    // The country code settles it.
    let (_, ev) = create(&app, &token, json!({"city": "Springfield", "country_code": "au"})).await;
    let rows = city_rows(&pool, "Springfield").await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].2.as_deref(), Some("AU"));
    assert_eq!(ev["timezone"], "Australia/Brisbane");
    assert_eq!(ev["country_code"], "AU");
    // The other country's Springfield can be learned too: one row per (name, country).
    let (_, us) = create(&app, &token, json!({"city": "Springfield", "lat": 39.8, "lng": -89.64})).await;
    assert_eq!(city_rows(&pool, "Springfield").await.len(), 2);
    assert_eq!(us["timezone"], "America/Chicago");
}

#[sqlx::test(migrations = "./migrations")]
async fn the_event_zone_follows_its_coordinates(pool: PgPool) {
    let (app, _stub) = app_with(pool.clone(), Mode::Ok).await;
    let token = user(&pool, "o@x.co").await;
    let zone = |v: &Value| v["timezone"].as_str().unwrap().to_string();

    // By name from the gazetteer (seeded rows carry zones).
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
    // By point: the boundary polygons, also next to zone borders. Event and endpoint agree.
    let towns = [
        (42.9634, -85.6681, "US", "America/Detroit"),       // Grand Rapids
        (30.4383, -84.2807, "US", "America/New_York"),      // Tallahassee
        (35.2220, -101.8313, "US", "America/Chicago"),      // Amarillo
        (53.3017, -60.3261, "CA", "America/Goose_Bay"),     // Goose Bay
        (55.7596, -120.2353, "CA", "America/Dawson_Creek"), // Dawson Creek
        (52.0317, 113.5010, "RU", "Asia/Chita"),            // Chita
        (46.9591, 142.7380, "RU", "Asia/Sakhalin"),         // Yuzhno-Sakhalinsk
        (51.5331, 46.0342, "RU", "Europe/Saratov"),         // Saratov
        (50.4501, 30.5234, "UA", "Europe/Kyiv"),
        (35.6762, 139.6503, "JP", "Asia/Tokyo"),
        (38.7223, -9.1393, "PT", "Europe/Lisbon"),
    ];
    for (i, (lat, lng, cc, want)) in towns.iter().enumerate() {
        let (_, ev) = create(&app, &token, json!({"lat": lat, "lng": lng, "country_code": cc})).await;
        assert_eq!(zone(&ev), *want, "event at {lat},{lng}");
        let (_, tz) = get(
            &app,
            &format!("/api/geo/timezone?lat={lat}&lng={lng}"),
            &format!("192.0.2.{}", 10 + i),
        )
        .await;
        assert_eq!(tz["timezone"], *want, "/api/geo/timezone at {lat},{lng}");
    }
    let (_, tokyo) = create(
        &app,
        &token,
        json!({"lat": 35.69, "lng": 139.7, "country_code": "JP", "country": "Japan"}),
    )
    .await;
    assert_eq!(tokyo["country"], "Japan");
    assert_eq!(tokyo["currency"], "JPY");
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
async fn re_sending_the_same_place_keeps_geo_and_zone(pool: PgPool) {
    let (app, _stub) = app_with(pool.clone(), Mode::Ok).await;
    let token = user(&pool, "o@x.co").await;
    // An address point in Lisbon with the organiser's own zone (not the city's).
    let (_, ev) = create(
        &app,
        &token,
        json!({"city": "Lisbon", "lat": 38.7139, "lng": -9.1334, "country": "Portugal", "country_code": "PT",
            "timezone": "Atlantic/Azores"}),
    )
    .await;
    let id = ev["id"].as_str().unwrap();
    let geo = |v: &Value| {
        (
            v["lat"].clone(),
            v["lng"].clone(),
            v["country"].clone(),
            v["country_code"].clone(),
            v["timezone"].clone(),
        )
    };
    // Android re-sends the unchanged city on every save.
    for body in [
        json!({"title": "Gig 2", "city": "Lisbon", "sales_closed": true}),
        json!({"city": " lisbon ", "sales_closed": false}),
        json!({"city": "Lisbon", "lat": 38.7139, "lng": -9.1334, "country": "Portugal", "country_code": "pt"}),
    ] {
        let (status, kept) = update(&app, &token, id, body.clone()).await;
        assert_eq!(status, StatusCode::OK, "{kept}");
        assert_eq!(geo(&kept), geo(&ev), "[DATA] {body} must not move the event");
    }
    // A real change still re-settles.
    let (_, moved) = update(
        &app,
        &token,
        id,
        json!({"title": "Gig 3", "city": "Tokyo", "sales_closed": true}),
    )
    .await;
    assert_eq!(moved["timezone"], "Asia/Tokyo");
    assert_eq!(moved["country_code"], "JP");
    assert!((moved["lat"].as_f64().unwrap() - 35.6762).abs() < 0.001);
}

#[sqlx::test(migrations = "./migrations")]
async fn event_place_fields_are_validated_and_cleaned(pool: PgPool) {
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
    // Zero-width and bidi controls are removed from the place text (F11).
    let (_, clean) = create(
        &app,
        &token,
        json!({"city": "Ки\u{200B}їв\u{202E}", "venue_address": "\u{2066}Хрещатик 1\u{2069}",
            "country": "Укра\u{FEFF}їна"}),
    )
    .await;
    assert_eq!(clean["city"], "Київ");
    assert_eq!(clean["venue_address"], "Хрещатик 1");
    assert_eq!(clean["country"], "Україна");
    assert_eq!(clean["timezone"], "Europe/Kyiv", "and the cleaned name resolves");
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
        ("33.4484", "-112.074", "US", "America/Phoenix"),
        ("39.7392", "-104.9903", "US", "America/Denver"),
        ("40.7128", "-74.006", "US", "America/New_York"),
        ("-31.95", "115.86", "AU", "Australia/Perth"),
        ("38.75", "-9.2", "", "Europe/Lisbon"),
        // At sea, no country: the default an event gets.
        ("-30.0", "-140.0", "", "Europe/Kyiv"),
        // At sea with a country: that country's zone table.
        ("-30.0", "-140.0", "PF", "Pacific/Gambier"),
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

// ---------------------------------------------------------------- sign-up language (item 7)

async fn prefs_of(app: &Router, token: &str) -> Value {
    json_req(app, "GET", "/api/me/prefs", Some(token), json!({})).await.1
}

fn locale_and_onboarded(p: &Value) -> (Value, Value) {
    (p["locale"].clone(), p["onboarded"].clone())
}

#[sqlx::test(migrations = "./migrations")]
async fn the_sign_up_language_is_kept_without_finishing_onboarding(pool: PgPool) {
    let app = app_on(pool.clone(), "http://127.0.0.1:9");
    let (status, reg) = json_req(
        &app,
        "POST",
        "/api/auth/register",
        None,
        json!({"email": "ja@x.co", "password": "password1", "name": "J", "locale": "ja"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{reg}");
    let token = reg["token"].as_str().unwrap().to_string();
    let p = prefs_of(&app, &token).await;
    assert_eq!(locale_and_onboarded(&p), (json!("ja"), json!(false)));
    // Saving prefs without a locale keeps it and finishes onboarding.
    let (_, saved) = json_req(
        &app,
        "PUT",
        "/api/me/prefs",
        Some(&token),
        json!({"categories": ["Rock"]}),
    )
    .await;
    assert_eq!(locale_and_onboarded(&saved), (json!("ja"), json!(true)));
    let (_, saved) = json_req(
        &app,
        "PUT",
        "/api/me/prefs",
        Some(&token),
        json!({"categories": [], "locale": "tr"}),
    )
    .await;
    assert_eq!(saved["locale"], "tr", "all 10 languages are stored as given");

    // An unknown language at sign-up is ignored: no prefs row, onboarding still due.
    let (_, reg) = json_req(
        &app,
        "POST",
        "/api/auth/register",
        None,
        json!({"email": "ru@x.co", "password": "password1", "name": "R", "locale": "ru"}),
    )
    .await;
    let p = prefs_of(&app, reg["token"].as_str().unwrap()).await;
    assert_eq!(locale_and_onboarded(&p), (json!("uk"), json!(false)));

    // The e-mail code that creates the account carries the language; a later sign-in does not reset it.
    for (code, locale) in [("111111", "pl"), ("222222", "de")] {
        sqlx::query(
            "INSERT INTO login_codes (email, code_hash, expires_at) \
             VALUES ('pl@x.co', $1, now() + interval '10 minutes')",
        )
        .bind(underground_api::emailcode::code_hash(code))
        .execute(&pool)
        .await
        .unwrap();
        let (status, v) = json_req(
            &app,
            "POST",
            "/api/auth/email/verify-code",
            None,
            json!({"email": "pl@x.co", "code": code, "locale": locale}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{v}");
        let p = prefs_of(&app, v["token"].as_str().unwrap()).await;
        assert_eq!(locale_and_onboarded(&p), (json!("pl"), json!(false)));
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn languages_without_server_texts_get_english_ones(pool: PgPool) {
    let app = app_on(pool.clone(), "http://127.0.0.1:9");
    let maker = user(&pool, "m@x.co").await;
    let maker_id = user_id(&pool, "m@x.co").await;
    let mut fans = Vec::new();
    for locale in ["de", "uk"] {
        let fan = user(&pool, &format!("{locale}@x.co")).await;
        json_req(
            &app,
            "PUT",
            "/api/me/prefs",
            Some(&fan),
            json!({"categories": [], "locale": locale}),
        )
        .await;
        json_req(
            &app,
            "POST",
            &format!("/api/users/{maker_id}/follow"),
            Some(&fan),
            json!({}),
        )
        .await;
        fans.push(fan);
    }
    create(&app, &maker, json!({"city": "Київ"})).await;
    let mut titles = Vec::new();
    for fan in &fans {
        let (_, list) = json_req(&app, "GET", "/api/me/notifications", Some(fan), json!({})).await;
        let title = list
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["kind"] == "organizer_event")
            .map(|n| n["title"].as_str().unwrap().to_string())
            .unwrap();
        titles.push(title);
    }
    assert!(titles[0].ends_with("announced an event"), "de → English: {}", titles[0]);
    assert!(titles[1].ends_with("оголосив(ла) подію"), "uk stays uk: {}", titles[1]);
}

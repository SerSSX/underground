//! Geo-aware discovery (TASK-017 M2): city gazetteer, geocoding-on-create, distance ranking.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use underground_api::routes::router;
use underground_api::state::AppState;

fn make_app(pool: PgPool) -> Router {
    router(AppState {
        db: pool,
        jwt_secret: "test-secret".into(),
    })
}

fn build(method: &str, path: &str, token: Option<&str>, body: Option<Value>) -> Request<Body> {
    let mut b = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    b.body(body.map(|v| Body::from(v.to_string())).unwrap_or_else(Body::empty))
        .unwrap()
}

async fn send(app: &Router, req: Request<Body>) -> (StatusCode, Value) {
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, body)
}

async fn register(app: &Router, email: &str) -> String {
    let (_, v) = send(
        app,
        build(
            "POST",
            "/api/auth/register",
            None,
            Some(json!({"email":email,"password":"password1","name":"X"})),
        ),
    )
    .await;
    v["token"].as_str().unwrap().to_string()
}

async fn create_event(app: &Router, token: &str, title: &str, city: &str) -> Value {
    let (status, v) = send(
        app,
        build(
            "POST",
            "/api/events",
            Some(token),
            Some(json!({"title":title,"category":"Rock","city":city,"price":"0","tickets_total":10})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    v
}

#[sqlx::test(migrations = "./migrations")]
async fn cities_autocomplete(pool: PgPool) {
    let app = make_app(pool);
    let (status, v) = send(&app, build("GET", "/api/cities?q=Ки", None, None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v[0]["name"], "Київ");
    assert_eq!(v[0]["country"], "Україна");

    let (_, v) = send(&app, build("GET", "/api/cities?q=Lon", None, None)).await;
    assert_eq!(v[0]["name"], "London");
    assert_eq!(v[0]["country"], "United Kingdom");

    let (_, all) = send(&app, build("GET", "/api/cities", None, None)).await;
    assert!(all.as_array().unwrap().len() >= 10, "returns a list without a query");
    assert!(all.as_array().unwrap().len() <= 20, "capped at 20");
}

#[sqlx::test(migrations = "./migrations")]
async fn event_is_geocoded_from_gazetteer(pool: PgPool) {
    let app = make_app(pool);
    let maker = register(&app, "m@x.co").await;

    let known = create_event(&app, &maker, "In Kyiv", "Київ").await;
    assert_eq!(known["country"], "Україна");
    assert!((known["lat"].as_f64().unwrap() - 50.4501).abs() < 0.001);

    let unknown = create_event(&app, &maker, "Nowhere", "Atlantis").await;
    assert!(unknown["country"].is_null(), "unknown city → no geo");
    assert!(unknown["lat"].is_null());

    // Editing the city re-geocodes.
    let id = unknown["id"].as_str().unwrap();
    let (_, edited) = send(
        &app,
        build(
            "PUT",
            &format!("/api/events/{id}"),
            Some(&maker),
            Some(json!({"city":"London"})),
        ),
    )
    .await;
    assert_eq!(edited["country"], "United Kingdom");
    assert!((edited["lng"].as_f64().unwrap() - (-0.1278)).abs() < 0.001);
}

#[sqlx::test(migrations = "./migrations")]
async fn prefs_country_derived_from_city(pool: PgPool) {
    let app = make_app(pool);
    let user = register(&app, "u@x.co").await;

    let (_, p) = send(
        &app,
        build(
            "PUT",
            "/api/me/prefs",
            Some(&user),
            Some(json!({"categories":["Rock"],"city":"Київ"})),
        ),
    )
    .await;
    assert_eq!(p["country"], "Україна");
    let (_, g) = send(&app, build("GET", "/api/me/prefs", Some(&user), None)).await;
    assert_eq!(g["country"], "Україна");

    let (_, p2) = send(
        &app,
        build(
            "PUT",
            "/api/me/prefs",
            Some(&user),
            Some(json!({"categories":["Rock"],"city":"Atlantis"})),
        ),
    )
    .await;
    assert!(p2["country"].is_null(), "unknown city → no country");
}

#[sqlx::test(migrations = "./migrations")]
async fn discover_ranks_nearby_first_with_distance(pool: PgPool) {
    let app = make_app(pool);
    let user = register(&app, "u@x.co").await;
    let maker = register(&app, "m@x.co").await;

    send(
        &app,
        build(
            "PUT",
            "/api/me/prefs",
            Some(&user),
            Some(json!({"categories":["Rock"],"city":"Київ"})),
        ),
    )
    .await;
    create_event(&app, &maker, "Local Kyiv", "Київ").await;
    create_event(&app, &maker, "Far London", "London").await;

    let (status, v) = send(&app, build("GET", "/api/discover", Some(&user), None)).await;
    assert_eq!(status, StatusCode::OK);
    let items = v.as_array().unwrap();
    let local = items.iter().find(|e| e["title"] == "Local Kyiv").unwrap();
    let far = items.iter().find(|e| e["title"] == "Far London").unwrap();

    assert_eq!(local["reason"], "near", "same-city event is 'near'");
    assert!(local["distance_km"].as_f64().unwrap() < 1.0, "≈0 km to own city");
    assert!(far["distance_km"].as_f64().unwrap() > 1500.0, "London is far from Kyiv");

    let titles: Vec<&str> = items.iter().map(|e| e["title"].as_str().unwrap()).collect();
    assert!(
        titles.iter().position(|t| *t == "Local Kyiv") < titles.iter().position(|t| *t == "Far London"),
        "nearby ranks above far: {titles:?}",
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn discover_anonymous_has_no_distance(pool: PgPool) {
    let app = make_app(pool);
    let maker = register(&app, "m@x.co").await;
    create_event(&app, &maker, "Somewhere", "Київ").await;
    let (_, v) = send(&app, build("GET", "/api/discover", None, None)).await;
    assert!(v[0]["distance_km"].is_null(), "no viewer city → no distance");
    assert_eq!(v[0]["reason"], "trending");
}

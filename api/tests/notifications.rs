//! Integration tests for notifications (F-14): prefs-matched fan-out on event creation.

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
    let req = Request::post("/api/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"email": email, "password": "password1", "name": email.split('@').next().unwrap()}).to_string(),
        ))
        .unwrap();
    let (_, v) = send(app, req).await;
    v["token"].as_str().unwrap().to_string()
}

async fn put_prefs(app: &Router, token: &str, categories: Value, city: Value) {
    let req = Request::put("/api/me/prefs")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(json!({"categories": categories, "city": city}).to_string()))
        .unwrap();
    let (status, _) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
}

async fn create_event(app: &Router, token: &str, title: &str, category: &str, city: Value) {
    let req = Request::post("/api/events")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            json!({"title": title, "category": category, "city": city, "price": "0", "tickets_total": 10}).to_string(),
        ))
        .unwrap();
    let (status, _) = send(app, req).await;
    assert_eq!(status, StatusCode::CREATED);
}

async fn my_notifications(app: &Router, token: &str) -> Vec<Value> {
    let req = Request::get("/api/me/notifications")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(app, req).await;
    assert_eq!(status, StatusCode::OK);
    body.as_array().unwrap().clone()
}

#[sqlx::test(migrations = "./migrations")]
async fn notifications_require_auth(pool: PgPool) {
    let app = make_app(pool);
    let (status, _) = send(&app, Request::get("/api/me/notifications").body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "[SEC] list without token → 401");
}

#[sqlx::test(migrations = "./migrations")]
async fn fan_out_matches_category_and_city(pool: PgPool) {
    let app = make_app(pool);
    let maker = register(&app, "maker@x.co").await;
    let rocker_kyiv = register(&app, "rocker@x.co").await;
    let rocker_lviv = register(&app, "lviv@x.co").await;
    let jazz_fan = register(&app, "jazz@x.co").await;
    let rocker_anywhere = register(&app, "any@x.co").await;

    put_prefs(&app, &rocker_kyiv, json!(["Rock"]), json!("Київ")).await;
    put_prefs(&app, &rocker_lviv, json!(["Rock"]), json!("Львів")).await;
    put_prefs(&app, &jazz_fan, json!(["Jazz"]), json!("Київ")).await;
    put_prefs(&app, &rocker_anywhere, json!(["Rock"]), json!(null)).await;

    create_event(&app, &maker, "Гурт у клубі", "Rock", json!("Київ")).await;

    let got = my_notifications(&app, &rocker_kyiv).await;
    assert_eq!(got.len(), 1, "category+city match notified");
    assert_eq!(got[0]["kind"], "event_match");
    assert_eq!(got[0]["title"], "Нова подія: Гурт у клубі");
    assert_eq!(got[0]["read"], false);
    assert!(got[0]["event_id"].is_string());

    assert!(
        my_notifications(&app, &rocker_lviv).await.is_empty(),
        "other city skipped"
    );
    assert!(
        my_notifications(&app, &jazz_fan).await.is_empty(),
        "other category skipped"
    );
    assert_eq!(
        my_notifications(&app, &rocker_anywhere).await.len(),
        1,
        "no-city prefs match any city"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn organizer_not_notified_about_own_event(pool: PgPool) {
    let app = make_app(pool);
    let maker = register(&app, "maker@x.co").await;
    put_prefs(&app, &maker, json!(["Rock"]), json!("Київ")).await;
    create_event(&app, &maker, "Own gig", "Rock", json!("Київ")).await;
    assert!(my_notifications(&app, &maker).await.is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn city_match_is_case_insensitive_and_eventless_city_matches_all(pool: PgPool) {
    let app = make_app(pool);
    let maker = register(&app, "maker@x.co").await;
    let fan = register(&app, "fan@x.co").await;
    put_prefs(&app, &fan, json!(["Rock"]), json!("київ")).await;

    create_event(&app, &maker, "Case", "Rock", json!("КИЇВ")).await;
    assert_eq!(
        my_notifications(&app, &fan).await.len(),
        1,
        "city compared case-insensitively"
    );

    create_event(&app, &maker, "Nomad", "Rock", json!(null)).await;
    assert_eq!(
        my_notifications(&app, &fan).await.len(),
        2,
        "event without city matches everyone"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn mark_all_read(pool: PgPool) {
    let app = make_app(pool);
    let maker = register(&app, "maker@x.co").await;
    let fan = register(&app, "fan@x.co").await;
    put_prefs(&app, &fan, json!(["Rock"]), json!(null)).await;
    create_event(&app, &maker, "Gig", "Rock", json!("Київ")).await;

    let req = Request::post("/api/me/notifications/read")
        .header("authorization", format!("Bearer {fan}"))
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(&app, req).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let got = my_notifications(&app, &fan).await;
    assert_eq!(got.len(), 1);
    assert_eq!(got[0]["read"], true);
}

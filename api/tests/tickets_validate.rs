//! Integration tests for QR ticket validation (organizer scan at the door).

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
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(t) = token {
        builder = builder.header("authorization", format!("Bearer {t}"));
    }
    let body = body.map(|v| Body::from(v.to_string())).unwrap_or_else(Body::empty);
    builder.body(body).unwrap()
}

async fn json_body(resp: axum::response::Response) -> Value {
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn token_for(app: &Router, email: &str) -> String {
    let resp = app
        .clone()
        .oneshot(build(
            "POST",
            "/api/auth/register",
            None,
            Some(json!({"email":email,"password":"password1","name":"X"})),
        ))
        .await
        .unwrap();
    json_body(resp).await["token"].as_str().unwrap().to_string()
}

async fn free_event_id(app: &Router, token: &str) -> String {
    let body = json!({"title":"Free","category":"Poetry","price":"0","tickets_total":3,"cost_type":"free"});
    let resp = app
        .clone()
        .oneshot(build("POST", "/api/events", Some(token), Some(body)))
        .await
        .unwrap();
    json_body(resp).await["id"].as_str().unwrap().to_string()
}

async fn reserve_qr(app: &Router, token: &str, event: &str) -> String {
    let resp = app
        .clone()
        .oneshot(build(
            "POST",
            &format!("/api/events/{event}/reserve"),
            Some(token),
            None,
        ))
        .await
        .unwrap();
    json_body(resp).await["qr_token"].as_str().unwrap().to_string()
}

#[sqlx::test(migrations = "./migrations")]
async fn organizer_validates_then_marks_used(pool: PgPool) {
    let app = make_app(pool);
    let org = token_for(&app, "org@x.co").await;
    let event = free_event_id(&app, &org).await;
    let qr = reserve_qr(&app, &org, &event).await;

    let resp = app
        .clone()
        .oneshot(build(
            "POST",
            "/api/tickets/validate",
            Some(&org),
            Some(json!({"qr_token": qr})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = json_body(resp).await;
    assert_eq!(v["valid"], true);
    assert_eq!(v["reason"], "ok");
    assert_eq!(v["event_title"], "Free");

    let resp = app
        .oneshot(build(
            "POST",
            "/api/tickets/validate",
            Some(&org),
            Some(json!({"qr_token": qr})),
        ))
        .await
        .unwrap();
    let v = json_body(resp).await;
    assert_eq!(v["valid"], false);
    assert_eq!(v["reason"], "already_used");
}

#[sqlx::test(migrations = "./migrations")]
async fn non_organizer_forbidden(pool: PgPool) {
    let app = make_app(pool);
    let org = token_for(&app, "org@x.co").await;
    let other = token_for(&app, "other@x.co").await;
    let event = free_event_id(&app, &org).await;
    let qr = reserve_qr(&app, &org, &event).await;

    let resp = app
        .oneshot(build(
            "POST",
            "/api/tickets/validate",
            Some(&other),
            Some(json!({"qr_token": qr})),
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "[SEC] only the event organizer may validate"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn unknown_qr_404_and_no_token_401(pool: PgPool) {
    let app = make_app(pool);
    let org = token_for(&app, "org@x.co").await;

    let resp = app
        .clone()
        .oneshot(build(
            "POST",
            "/api/tickets/validate",
            Some(&org),
            Some(json!({"qr_token":"UG-nope"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let resp = app
        .oneshot(build(
            "POST",
            "/api/tickets/validate",
            None,
            Some(json!({"qr_token":"UG-x"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

//! Integration tests for ticket return + transfer (gift).

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

async fn reserve_ticket_id(app: &Router, token: &str, event: &str) -> String {
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
    json_body(resp).await["id"].as_str().unwrap().to_string()
}

#[sqlx::test(migrations = "./migrations")]
async fn owner_returns_active_ticket(pool: PgPool) {
    let app = make_app(pool);
    let user = token_for(&app, "u@x.co").await;
    let event = free_event_id(&app, &user).await;
    let tid = reserve_ticket_id(&app, &user, &event).await;

    let resp = app
        .clone()
        .oneshot(build("POST", &format!("/api/tickets/{tid}/return"), Some(&user), None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(json_body(resp).await["status"], "returned");

    let resp = app
        .oneshot(build("POST", &format!("/api/tickets/{tid}/return"), Some(&user), None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT, "cannot return twice");
}

#[sqlx::test(migrations = "./migrations")]
async fn non_owner_cannot_return(pool: PgPool) {
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let other = token_for(&app, "x@x.co").await;
    let event = free_event_id(&app, &owner).await;
    let tid = reserve_ticket_id(&app, &owner, &event).await;

    let resp = app
        .oneshot(build("POST", &format!("/api/tickets/{tid}/return"), Some(&other), None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "[SEC] not the owner");
}

#[sqlx::test(migrations = "./migrations")]
async fn transfer_moves_ticket_to_recipient(pool: PgPool) {
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let recipient = token_for(&app, "r@x.co").await;
    let event = free_event_id(&app, &owner).await;
    let tid = reserve_ticket_id(&app, &owner, &event).await;

    let resp = app
        .clone()
        .oneshot(build(
            "POST",
            &format!("/api/tickets/{tid}/transfer"),
            Some(&owner),
            Some(json!({"email":"r@x.co"})),
        ))
        .await
        .unwrap();
    // TASK-305 R2 (owner decision 2026-11-14): a transfer is an offer (was 200 with the moved
    // ticket); the ticket moves when the recipient accepts it from their alert.
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let resp = app
        .clone()
        .oneshot(build("GET", "/api/me/notifications", Some(&recipient), None))
        .await
        .unwrap();
    let alerts = json_body(resp).await;
    let claim = alerts[0]["link"]
        .as_str()
        .unwrap()
        .strip_prefix("/g/")
        .unwrap()
        .to_string();
    let resp = app
        .clone()
        .oneshot(build(
            "POST",
            "/api/tickets/claim",
            Some(&recipient),
            Some(json!({ "token": claim })),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = app
        .clone()
        .oneshot(build("GET", "/api/me/tickets", Some(&owner), None))
        .await
        .unwrap();
    assert_eq!(
        json_body(resp).await.as_array().unwrap().len(),
        0,
        "owner no longer holds it"
    );

    let resp = app
        .oneshot(build("GET", "/api/me/tickets", Some(&recipient), None))
        .await
        .unwrap();
    assert_eq!(
        json_body(resp).await.as_array().unwrap().len(),
        1,
        "recipient now holds it"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn transfer_validation(pool: PgPool) {
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let event = free_event_id(&app, &owner).await;
    let tid = reserve_ticket_id(&app, &owner, &event).await;

    let resp = app
        .clone()
        .oneshot(build(
            "POST",
            &format!("/api/tickets/{tid}/transfer"),
            Some(&owner),
            Some(json!({"email":"nobody@x.co"})),
        ))
        .await
        .unwrap();
    // TASK-305 R2 (owner decision 2026-11-14): the sender's answer never depends on the email
    // (was 422 for an unknown recipient and for self). Nobody receives the ticket.
    assert_eq!(resp.status(), StatusCode::ACCEPTED, "unknown recipient");
    let resp = app
        .clone()
        .oneshot(build("DELETE", &format!("/api/tickets/{tid}/gift"), Some(&owner), None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "the offer can be cancelled");

    let resp = app
        .clone()
        .oneshot(build(
            "POST",
            &format!("/api/tickets/{tid}/transfer"),
            Some(&owner),
            Some(json!({"email":"o@x.co"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED, "to self: the same answer");
    let resp = app
        .oneshot(build("GET", "/api/me/tickets", Some(&owner), None))
        .await
        .unwrap();
    let mine = json_body(resp).await;
    assert_eq!(mine.as_array().unwrap().len(), 1, "still the sender's");
}

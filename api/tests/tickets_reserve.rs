//! Integration tests for ticket reservation (free events).

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

async fn register_token(app: &Router) -> String {
    let req = Request::post("/api/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"email":"u@x.co","password":"password1","name":"U"}).to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice::<Value>(&bytes).unwrap()["token"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn register_email(app: &Router, email: &str) -> String {
    let req = Request::post("/api/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"email":email,"password":"password1","name":"U"}).to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice::<Value>(&bytes).unwrap()["token"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn create_event(app: &Router, token: &str, body: Value) -> String {
    let req = Request::post("/api/events")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(body.to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice::<Value>(&bytes).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}

fn reserve_req(event_id: &str, token: Option<&str>) -> Request<Body> {
    let mut builder = Request::post(format!("/api/events/{event_id}/reserve"));
    if let Some(t) = token {
        builder = builder.header("authorization", format!("Bearer {t}"));
    }
    builder.body(Body::empty()).unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn reserve_free_mints_ticket(pool: PgPool) {
    let app = make_app(pool);
    let token = register_token(&app).await;
    let event = create_event(
        &app,
        &token,
        json!({"title":"Free Show","category":"Poetry","price":"0","tickets_total":2,"cost_type":"free"}),
    )
    .await;

    let resp = app.clone().oneshot(reserve_req(&event, Some(&token))).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let ticket: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(ticket["qr_token"].as_str().unwrap().starts_with("UG-"));
    assert_eq!(ticket["event_title"], "Free Show");
    assert_eq!(ticket["status"], "active");

    let req = Request::get("/api/me/tickets")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes)
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn free_ticket_deduped_per_user(pool: PgPool) {
    let app = make_app(pool);
    let a = register_email(&app, "a@x.co").await;
    let b = register_email(&app, "b@x.co").await;
    let event = create_event(
        &app,
        &a,
        json!({"title":"Free","category":"Poetry","price":"0","tickets_total":5,"cost_type":"free"}),
    )
    .await;

    // First reserve OK; remember the ticket id.
    let resp = app.clone().oneshot(reserve_req(&event, Some(&a))).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let ticket_id = serde_json::from_slice::<Value>(&bytes).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Same user reserving again → 201: up to 5 places per person per event (TASK-302 Change C;
    // was 409 "one free ticket per user"; the owner approved this change on 2026-11-12).
    let resp = app.clone().oneshot(reserve_req(&event, Some(&a))).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "a second place is allowed (cap is 5)"
    );

    // A different user can still reserve.
    let resp = app.clone().oneshot(reserve_req(&event, Some(&b))).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // After returning, the first user may reserve again (returned places do not count).
    let ret = Request::post(format!("/api/tickets/{ticket_id}/return"))
        .header("authorization", format!("Bearer {a}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(ret).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = app.oneshot(reserve_req(&event, Some(&a))).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED, "re-reserve allowed after return");
}

#[sqlx::test(migrations = "./migrations")]
async fn reserve_requires_auth(pool: PgPool) {
    let app = make_app(pool);
    let token = register_token(&app).await;
    let event = create_event(
        &app,
        &token,
        json!({"title":"Free","category":"Poetry","price":"0","tickets_total":1,"cost_type":"free"}),
    )
    .await;
    let resp = app.oneshot(reserve_req(&event, None)).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "[SEC] reserve without token → 401"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn reserve_paid_event_conflicts(pool: PgPool) {
    let app = make_app(pool);
    let token = register_token(&app).await;
    let event = create_event(
        &app,
        &token,
        json!({"title":"Paid","category":"Rock","price":"100","tickets_total":5,"cost_type":"paid"}),
    )
    .await;
    let resp = app.oneshot(reserve_req(&event, Some(&token))).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

#[sqlx::test(migrations = "./migrations")]
async fn reserve_missing_event_404(pool: PgPool) {
    let app = make_app(pool);
    let token = register_token(&app).await;
    let resp = app
        .oneshot(reserve_req("00000000-0000-0000-0000-000000000000", Some(&token)))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

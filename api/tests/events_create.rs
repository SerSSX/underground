//! Integration tests for event creation (ephemeral DB via `#[sqlx::test]`).

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
            json!({"email":"maker@x.co","password":"password1","name":"Maker"}).to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    v["token"].as_str().unwrap().to_string()
}

fn post_event(token: Option<&str>, body: Value) -> Request<Body> {
    let mut builder = Request::post("/api/events").header("content-type", "application/json");
    if let Some(t) = token {
        builder = builder.header("authorization", format!("Bearer {t}"));
    }
    builder.body(Body::from(body.to_string())).unwrap()
}

fn valid_event() -> Value {
    json!({
        "title": "My Gig", "description": "desc", "category": "Rock", "city": "Kyiv",
        // Relative, not a fixed date: the public afisha hides past events, so a fixed date made this
        // test fail once the calendar passed it (CI, 12 Nov 2026).
        "venue_address": "Club", "starts_at": (chrono::Utc::now() + chrono::Duration::days(30)).to_rfc3339(),
        "price": "250.00", "currency": "UAH", "tickets_total": 100, "cost_type": "paid"
    })
}

#[sqlx::test(migrations = "./migrations")]
async fn create_requires_auth(pool: PgPool) {
    let app = make_app(pool);
    let resp = app.oneshot(post_event(None, valid_event())).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "[SEC] create without token → 401"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn create_then_appears_in_lists(pool: PgPool) {
    let app = make_app(pool);
    let token = register_token(&app).await;

    let resp = app
        .clone()
        .oneshot(post_event(Some(&token), valid_event()))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let created: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(created["title"], "My Gig");
    assert_eq!(created["tickets_left"], 100, "tickets_left seeded from tickets_total");

    let resp = app
        .clone()
        .oneshot(Request::get("/api/events").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let public: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(public.as_array().unwrap().len(), 1, "appears in public afisha");

    let req = Request::get("/api/me/events")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let mine: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(mine.as_array().unwrap().len(), 1, "appears in my events");
}

#[sqlx::test(migrations = "./migrations")]
async fn create_validates_input(pool: PgPool) {
    let app = make_app(pool);
    let token = register_token(&app).await;

    let blank_title = json!({"title":"  ","category":"Rock","price":"0","tickets_total":10});
    let resp = app
        .clone()
        .oneshot(post_event(Some(&token), blank_title))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let negative = json!({"title":"X","category":"Rock","price":"0","tickets_total":-5});
    let resp = app.oneshot(post_event(Some(&token), negative)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// TASK-102 / BR-020 — event_timezone contract.

#[sqlx::test(migrations = "./migrations")]
async fn create_defaults_timezone_to_kyiv(pool: PgPool) {
    let app = make_app(pool);
    let token = register_token(&app).await;
    let resp = app.oneshot(post_event(Some(&token), valid_event())).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let created: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        created["timezone"], "Europe/Kyiv",
        "absent timezone defaults to launch market"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn create_accepts_valid_iana_timezone(pool: PgPool) {
    let app = make_app(pool);
    let token = register_token(&app).await;
    let mut body = valid_event();
    body["timezone"] = json!("Europe/London");
    let resp = app.oneshot(post_event(Some(&token), body)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let created: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        created["timezone"], "Europe/London",
        "valid IANA timezone stored + returned"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn create_rejects_invalid_timezone(pool: PgPool) {
    let app = make_app(pool);
    let token = register_token(&app).await;
    let mut body = valid_event();
    body["timezone"] = json!("Mars/Phobos");
    let resp = app.oneshot(post_event(Some(&token), body)).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "invalid IANA timezone → 422"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn update_event_timezone_valid_and_invalid(pool: PgPool) {
    let app = make_app(pool);
    let token = register_token(&app).await;
    let created = app
        .clone()
        .oneshot(post_event(Some(&token), valid_event()))
        .await
        .unwrap();
    let bytes = to_bytes(created.into_body(), usize::MAX).await.unwrap();
    let id = serde_json::from_slice::<Value>(&bytes).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let put = |body: Value| {
        Request::put(format!("/api/events/{id}"))
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    };

    let ok = app
        .clone()
        .oneshot(put(json!({ "timezone": "Europe/London" })))
        .await
        .unwrap();
    assert_eq!(ok.status(), StatusCode::OK);
    let b = to_bytes(ok.into_body(), usize::MAX).await.unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&b).unwrap()["timezone"],
        "Europe/London",
        "update sets a new event timezone"
    );

    let bad = app.oneshot(put(json!({ "timezone": "Bad/Zone" }))).await.unwrap();
    assert_eq!(
        bad.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "update rejects an invalid timezone"
    );
}

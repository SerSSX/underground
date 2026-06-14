//! Integration tests for `GET/PUT /api/me/prefs` (onboarding preferences).

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

async fn register_token(app: &Router, email: &str) -> String {
    let req = Request::post("/api/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"email": email, "password": "password1", "name": "U"}).to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    v["token"].as_str().unwrap().to_string()
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

fn get_prefs(token: &str) -> Request<Body> {
    Request::get("/api/me/prefs")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

fn put_prefs(token: &str, body: Value) -> Request<Body> {
    Request::put("/api/me/prefs")
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn prefs_require_auth(pool: PgPool) {
    let app = make_app(pool);
    let (status, _) = send(&app, Request::get("/api/me/prefs").body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "[SEC] GET without token → 401");
    let req = Request::put("/api/me/prefs")
        .header("content-type", "application/json")
        .body(Body::from(json!({"categories": []}).to_string()))
        .unwrap();
    let (status, _) = send(&app, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "[SEC] PUT without token → 401");
}

#[sqlx::test(migrations = "./migrations")]
async fn default_prefs_not_onboarded(pool: PgPool) {
    let app = make_app(pool);
    let token = register_token(&app, "a@x.co").await;
    let (status, body) = send(&app, get_prefs(&token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["onboarded"], false);
    assert_eq!(body["categories"].as_array().unwrap().len(), 0);
    assert!(body["city"].is_null());
}

#[sqlx::test(migrations = "./migrations")]
async fn put_then_get_roundtrip(pool: PgPool) {
    let app = make_app(pool);
    let token = register_token(&app, "a@x.co").await;

    let (status, body) = send(
        &app,
        put_prefs(&token, json!({"categories": ["Rock", " Jazz "], "city": "Kyiv"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["onboarded"], true);
    assert_eq!(body["categories"], json!(["Rock", "Jazz"]), "categories trimmed");

    let (_, body) = send(&app, get_prefs(&token)).await;
    assert_eq!(body["onboarded"], true);
    assert_eq!(body["categories"], json!(["Rock", "Jazz"]));
    assert_eq!(body["city"], "Kyiv");
}

#[sqlx::test(migrations = "./migrations")]
async fn put_upserts_and_clears(pool: PgPool) {
    let app = make_app(pool);
    let token = register_token(&app, "a@x.co").await;

    send(&app, put_prefs(&token, json!({"categories": ["Rock"], "city": "Kyiv"}))).await;
    let (status, body) = send(&app, put_prefs(&token, json!({"categories": [], "city": null}))).await;
    assert_eq!(status, StatusCode::OK);

    let (_, body_get) = send(&app, get_prefs(&token)).await;
    assert_eq!(body_get["categories"].as_array().unwrap().len(), 0, "overwritten");
    assert!(body_get["city"].is_null(), "city cleared");
    assert_eq!(body["onboarded"], true, "still onboarded after clearing");
}

#[sqlx::test(migrations = "./migrations")]
async fn put_filters_blank_and_limits_count(pool: PgPool) {
    let app = make_app(pool);
    let token = register_token(&app, "a@x.co").await;

    let (status, body) = send(&app, put_prefs(&token, json!({"categories": ["Rock", "  ", ""]}))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["categories"], json!(["Rock"]), "blank categories dropped");

    let many: Vec<String> = (0..21).map(|i| format!("c{i}")).collect();
    let (status, _) = send(&app, put_prefs(&token, json!({ "categories": many }))).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "more than 20 categories rejected"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn locale_round_trips_and_normalizes(pool: PgPool) {
    let app = make_app(pool);
    let token = register_token(&app, "a@x.co").await;

    // No prefs row yet → default locale.
    let (_, def) = send(&app, get_prefs(&token)).await;
    assert_eq!(def["locale"], "uk");

    // A supported locale round-trips.
    let (status, body) = send(&app, put_prefs(&token, json!({"categories": ["Rock"], "locale": "en"}))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["locale"], "en");
    let (_, got) = send(&app, get_prefs(&token)).await;
    assert_eq!(got["locale"], "en");

    // An unsupported locale normalizes back to the default.
    let (_, norm) = send(&app, put_prefs(&token, json!({"categories": ["Rock"], "locale": "zz"}))).await;
    assert_eq!(norm["locale"], "uk", "unknown locale falls back to uk");
}

#[sqlx::test(migrations = "./migrations")]
async fn prefs_are_per_user(pool: PgPool) {
    let app = make_app(pool);
    let alice = register_token(&app, "alice@x.co").await;
    let bob = register_token(&app, "bob@x.co").await;

    send(&app, put_prefs(&alice, json!({"categories": ["Rock"], "city": "Kyiv"}))).await;
    let (_, body) = send(&app, get_prefs(&bob)).await;
    assert_eq!(body["onboarded"], false, "bob unaffected by alice's prefs");
}

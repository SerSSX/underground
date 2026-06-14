//! Integration tests for email/password auth (ephemeral DB via `#[sqlx::test]`).

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

fn post(path: &str, body: Value) -> Request<Body> {
    Request::post(path)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn body_json(resp: axum::response::Response) -> Value {
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn register_then_me_then_login(pool: PgPool) {
    let app = make_app(pool);

    let resp = app
        .clone()
        .oneshot(post(
            "/api/auth/register",
            json!({"email":"a@b.co","password":"password1","name":"Alice"}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let v = body_json(resp).await;
    let token = v["token"].as_str().unwrap().to_string();
    assert_eq!(v["user"]["email"], "a@b.co");
    assert!(
        v["user"].get("password_hash").is_none(),
        "[SEC] password hash must not be in response"
    );

    let req = Request::get("/api/me")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_json(resp).await["email"], "a@b.co");

    let resp = app
        .oneshot(post(
            "/api/auth/login",
            json!({"email":"a@b.co","password":"password1"}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "./migrations")]
async fn duplicate_email_conflicts(pool: PgPool) {
    let app = make_app(pool);
    let body = json!({"email":"dup@b.co","password":"password1","name":"Dup"});
    let r1 = app
        .clone()
        .oneshot(post("/api/auth/register", body.clone()))
        .await
        .unwrap();
    assert_eq!(r1.status(), StatusCode::CREATED);
    let r2 = app.oneshot(post("/api/auth/register", body)).await.unwrap();
    assert_eq!(r2.status(), StatusCode::CONFLICT);
}

#[sqlx::test(migrations = "./migrations")]
async fn email_is_case_insensitive(pool: PgPool) {
    let app = make_app(pool);
    // Mixed-case registration is stored lowercased.
    let resp = app
        .clone()
        .oneshot(post(
            "/api/auth/register",
            json!({"email":"Mixed@Case.Co","password":"password1","name":"M"}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    assert_eq!(body_json(resp).await["user"]["email"], "mixed@case.co");

    // The same email in a different case can't register again (case-insensitive uniqueness).
    let resp = app
        .clone()
        .oneshot(post(
            "/api/auth/register",
            json!({"email":"mixed@case.co","password":"password1","name":"M2"}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);

    // Login works in yet another case → same account.
    let resp = app
        .oneshot(post(
            "/api/auth/login",
            json!({"email":"MIXED@CASE.CO","password":"password1"}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "./migrations")]
async fn login_wrong_password_is_unauthorized(pool: PgPool) {
    let app = make_app(pool);
    app.clone()
        .oneshot(post(
            "/api/auth/register",
            json!({"email":"c@b.co","password":"password1","name":"C"}),
        ))
        .await
        .unwrap();
    let resp = app
        .oneshot(post(
            "/api/auth/login",
            json!({"email":"c@b.co","password":"wrongpass1"}),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn me_requires_valid_token(pool: PgPool) {
    let app = make_app(pool);
    let resp = app
        .clone()
        .oneshot(Request::get("/api/me").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "[SEC] missing token → 401");

    let req = Request::get("/api/me")
        .header("authorization", "Bearer garbage")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "[SEC] forged token → 401");
}

#[sqlx::test(migrations = "./migrations")]
async fn register_rejects_invalid_input(pool: PgPool) {
    let app = make_app(pool);
    for body in [
        json!({"email":"nope","password":"password1","name":"X"}),
        json!({"email":"x@b.co","password":"short","name":"X"}),
        json!({"email":"y@b.co","password":"password1","name":"  "}),
    ] {
        let r = app.clone().oneshot(post("/api/auth/register", body)).await.unwrap();
        assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
}

//! Integration tests for passwordless email-code login (TASK-115, UC-001, WF-Auth).

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use chrono::{Duration, Utc};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use underground_api::emailcode::code_hash;
use underground_api::routes::router;
use underground_api::state::AppState;

fn make_app(pool: PgPool) -> Router {
    router(AppState {
        db: pool,
        jwt_secret: "test-secret".into(),
    })
}

fn post(path: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn json_body(resp: axum::response::Response) -> Value {
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn seed_code(pool: &PgPool, email: &str, code: &str, ttl_mins: i64) {
    let expires = Utc::now() + Duration::minutes(ttl_mins);
    sqlx::query("INSERT INTO login_codes (email, code_hash, expires_at) VALUES ($1, $2, $3)")
        .bind(email)
        .bind(code_hash(code))
        .bind(expires)
        .execute(pool)
        .await
        .unwrap();
}

fn verify(email: &str, code: &str) -> Request<Body> {
    post("/api/auth/email/verify-code", json!({ "email": email, "code": code }))
}

#[sqlx::test(migrations = "./migrations")]
async fn request_code_stores_and_returns_204(pool: PgPool) {
    let db = pool.clone();
    let app = make_app(pool);
    let resp = app
        .oneshot(post("/api/auth/email/request-code", json!({ "email": "a@b.co" })))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM login_codes WHERE email = 'a@b.co'")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(n, 1, "a code row is stored");
}

#[sqlx::test(migrations = "./migrations")]
async fn request_code_rejects_invalid_email(pool: PgPool) {
    let app = make_app(pool);
    let resp = app
        .oneshot(post("/api/auth/email/request-code", json!({ "email": "nope" })))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "./migrations")]
async fn verify_creates_account_and_returns_token(pool: PgPool) {
    let db = pool.clone();
    let app = make_app(pool);
    seed_code(&db, "new@b.co", "123456", 10).await;
    let resp = app.oneshot(verify("new@b.co", "123456")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert!(body["token"].as_str().unwrap().len() > 10, "issues a JWT");
    assert_eq!(body["user"]["email"], "new@b.co");
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE email = 'new@b.co'")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(n, 1, "account is created on first passwordless login");
}

#[sqlx::test(migrations = "./migrations")]
async fn verify_rejects_wrong_code(pool: PgPool) {
    let db = pool.clone();
    let app = make_app(pool);
    seed_code(&db, "a@b.co", "123456", 10).await;
    let resp = app.oneshot(verify("a@b.co", "000000")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "[SEC] wrong code is rejected");
}

#[sqlx::test(migrations = "./migrations")]
async fn verify_rejects_expired_code(pool: PgPool) {
    let db = pool.clone();
    let app = make_app(pool);
    seed_code(&db, "a@b.co", "123456", -1).await;
    let resp = app.oneshot(verify("a@b.co", "123456")).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "[SEC] expired code is rejected"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn verify_code_is_single_use(pool: PgPool) {
    let db = pool.clone();
    let app = make_app(pool);
    seed_code(&db, "a@b.co", "123456", 10).await;
    let first = app.clone().oneshot(verify("a@b.co", "123456")).await.unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    let second = app.oneshot(verify("a@b.co", "123456")).await.unwrap();
    assert_eq!(
        second.status(),
        StatusCode::UNAUTHORIZED,
        "[SEC] a used code cannot be replayed"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn request_code_soft_caps_per_email(pool: PgPool) {
    let db = pool.clone();
    let app = make_app(pool);
    for _ in 0..6 {
        let resp = app
            .clone()
            .oneshot(post("/api/auth/email/request-code", json!({ "email": "a@b.co" })))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    }
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM login_codes WHERE email = 'a@b.co'")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(n, 5, "soft cap stops issuing after 5 codes in the window");
}

#[sqlx::test(migrations = "./migrations")]
async fn verify_blocked_user_is_unauthorized(pool: PgPool) {
    let db = pool.clone();
    let app = make_app(pool);
    app.clone()
        .oneshot(post(
            "/api/auth/register",
            json!({"email":"blk@x.co","password":"password1","name":"B"}),
        ))
        .await
        .unwrap();
    sqlx::query("UPDATE users SET blocked = true WHERE email = 'blk@x.co'")
        .execute(&db)
        .await
        .unwrap();
    seed_code(&db, "blk@x.co", "123456", 10).await;
    let resp = app.oneshot(verify("blk@x.co", "123456")).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "[SEC] a blocked account can't passwordless-login even with a valid code"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn verify_logs_in_existing_account(pool: PgPool) {
    let db = pool.clone();
    let app = make_app(pool);
    app.clone()
        .oneshot(post(
            "/api/auth/register",
            json!({"email":"ex@x.co","password":"password1","name":"E"}),
        ))
        .await
        .unwrap();
    seed_code(&db, "ex@x.co", "123456", 10).await;
    let resp = app.oneshot(verify("ex@x.co", "123456")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "an existing account logs in via code");
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE email = 'ex@x.co'")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(n, 1, "no duplicate account is created");
}

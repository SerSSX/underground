//! Integration tests for rate limiting on auth endpoints (TASK-113, BR-017).

use axum::body::Body;
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
    Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn login_is_rate_limited_after_ten_attempts(pool: PgPool) {
    let app = make_app(pool);
    let bad = || {
        post(
            "/api/auth/login",
            json!({ "email": "x@y.co", "password": "wrong-pass" }),
        )
    };
    for _ in 0..10 {
        let resp = app.clone().oneshot(bad()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
    let resp = app.oneshot(bad()).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "[SEC] brute-force login is throttled after the cap"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn verify_code_is_rate_limited_after_ten_attempts(pool: PgPool) {
    let app = make_app(pool);
    let bad = || {
        post(
            "/api/auth/email/verify-code",
            json!({ "email": "a@b.co", "code": "000000" }),
        )
    };
    for _ in 0..10 {
        let resp = app.clone().oneshot(bad()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
    let resp = app.oneshot(bad()).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "[SEC] code guessing is throttled after the cap"
    );
}

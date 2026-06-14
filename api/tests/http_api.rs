//! HTTP-level tests for the router (in-process via `oneshot`), covering the
//! handler + routing layer. Ephemeral DB via `#[sqlx::test]` (rules §6.3).

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use sqlx::PgPool;
use tower::ServiceExt;
use underground_api::routes::router;
use underground_api::state::AppState;

fn get(path: &str) -> Request<Body> {
    Request::get(path).body(Body::empty()).unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn health_returns_ok(pool: PgPool) {
    let resp = router(AppState {
        db: pool,
        jwt_secret: "test-secret".into(),
    })
    .oneshot(get("/health"))
    .await
    .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "./migrations")]
async fn version_returns_name(pool: PgPool) {
    let resp = router(AppState {
        db: pool,
        jwt_secret: "test-secret".into(),
    })
    .oneshot(get("/api/version"))
    .await
    .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(body.contains("underground-api"), "version body: {body}");
}

#[sqlx::test(migrations = "./migrations")]
async fn events_returns_empty_json_array(pool: PgPool) {
    let resp = router(AppState {
        db: pool,
        jwt_secret: "test-secret".into(),
    })
    .oneshot(get("/api/events"))
    .await
    .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    assert_eq!(&bytes[..], b"[]");
}

//! Negative HTTP-level test: a DB failure surfaces as 500 without leaking details.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use sqlx::PgPool;
use tower::ServiceExt;
use underground_api::routes::router;
use underground_api::state::AppState;

#[sqlx::test(migrations = "./migrations")]
async fn events_returns_500_on_db_error(pool: PgPool) {
    // Close the pool so the query fails → AppError::Db → 500.
    pool.close().await;
    let req = Request::get("/api/events").body(Body::empty()).unwrap();
    let resp = router(AppState {
        db: pool,
        jwt_secret: "test-secret".into(),
    })
    .oneshot(req)
    .await
    .unwrap();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

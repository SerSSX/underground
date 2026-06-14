//! Product telemetry (TASK-017 M4): client log endpoint + admin aggregation.
//! Server-side instrumentation (signup/save/follow/ticket) is exercised by the
//! handler suites; here we cover the client endpoint and the admin rollup.

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
    let mut b = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    b.body(body.map(|v| Body::from(v.to_string())).unwrap_or_else(Body::empty))
        .unwrap()
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

async fn register(app: &Router, email: &str) -> String {
    let (_, v) = send(
        app,
        build(
            "POST",
            "/api/auth/register",
            None,
            Some(json!({"email":email,"password":"password1","name":email.split('@').next().unwrap()})),
        ),
    )
    .await;
    v["token"].as_str().unwrap().to_string()
}

async fn make_admin(app: &Router, pool: &PgPool, email: &str) -> String {
    let token = register(app, email).await;
    sqlx::query("UPDATE users SET role = 'ADMIN' WHERE email = $1")
        .bind(email)
        .execute(pool)
        .await
        .unwrap();
    token
}

#[sqlx::test(migrations = "./migrations")]
async fn log_events_and_admin_rollup(pool: PgPool) {
    let app = make_app(pool.clone());
    // Registering an admin user also emits a server-side `signup` event.
    let admin = make_admin(&app, &pool, "admin@x.co").await;
    let u = register(&app, "u@x.co").await;

    // Anonymous client event with props.
    let (ok1, _) = send(
        &app,
        build(
            "POST",
            "/api/telemetry",
            None,
            Some(json!({"name":"discover_impression","props":{"n":5}})),
        ),
    )
    .await;
    assert_eq!(ok1, StatusCode::NO_CONTENT);

    // Authenticated client event without props.
    let (ok2, _) = send(
        &app,
        build("POST", "/api/telemetry", Some(&u), Some(json!({ "name": "share" }))),
    )
    .await;
    assert_eq!(ok2, StatusCode::NO_CONTENT);

    // Validation: blank and over-long names are rejected.
    let (bad1, _) = send(
        &app,
        build("POST", "/api/telemetry", None, Some(json!({ "name": " " }))),
    )
    .await;
    assert_eq!(bad1, StatusCode::UNPROCESSABLE_ENTITY);
    let long = "x".repeat(65);
    let (bad2, _) = send(
        &app,
        build("POST", "/api/telemetry", None, Some(json!({ "name": long }))),
    )
    .await;
    assert_eq!(bad2, StatusCode::UNPROCESSABLE_ENTITY);

    // Admin rollup sees the events grouped by name.
    let (st, stats) = send(&app, build("GET", "/api/admin/telemetry", Some(&admin), None)).await;
    assert_eq!(st, StatusCode::OK);
    let names: Vec<&str> = stats
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"signup"), "server-side signup instrumentation recorded");
    assert!(names.contains(&"discover_impression"));
    assert!(names.contains(&"share"));

    // Non-admin is forbidden.
    let (forbidden, _) = send(&app, build("GET", "/api/admin/telemetry", Some(&u), None)).await;
    assert_eq!(forbidden, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "./migrations")]
async fn log_swallows_insert_errors(pool: PgPool) {
    // A non-existent user_id violates the FK; log() must swallow the error (best-effort)
    // and emit nothing, never panicking the caller.
    underground_api::telemetry::log(&pool, Some(uuid::Uuid::new_v4()), "orphan", serde_json::json!({})).await;
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM telemetry_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 0, "FK-violating telemetry write is dropped, not inserted");
}

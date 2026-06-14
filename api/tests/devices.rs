//! Device push-token registration (TASK-017 M4).

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

#[sqlx::test(migrations = "./migrations")]
async fn register_refresh_and_unregister(pool: PgPool) {
    let app = make_app(pool);
    let u = register(&app, "u@x.co").await;

    // Blank token is rejected.
    let (bad, _) = send(
        &app,
        build("POST", "/api/me/devices", Some(&u), Some(json!({ "token": "  " }))),
    )
    .await;
    assert_eq!(bad, StatusCode::UNPROCESSABLE_ENTITY);

    // Explicit platform.
    let (ok1, _) = send(
        &app,
        build(
            "POST",
            "/api/me/devices",
            Some(&u),
            Some(json!({"token":"tok-1","platform":"ios"})),
        ),
    )
    .await;
    assert_eq!(ok1, StatusCode::NO_CONTENT);

    // Missing platform defaults to android.
    let (ok2, _) = send(
        &app,
        build("POST", "/api/me/devices", Some(&u), Some(json!({ "token": "tok-2" }))),
    )
    .await;
    assert_eq!(ok2, StatusCode::NO_CONTENT);

    // Re-registering the same token refreshes it (conflict update path).
    let (ok3, _) = send(
        &app,
        build(
            "POST",
            "/api/me/devices",
            Some(&u),
            Some(json!({"token":"tok-1","platform":"android"})),
        ),
    )
    .await;
    assert_eq!(ok3, StatusCode::NO_CONTENT);

    // Unregister.
    let (gone, _) = send(
        &app,
        build("DELETE", "/api/me/devices", Some(&u), Some(json!({ "token": "tok-1" }))),
    )
    .await;
    assert_eq!(gone, StatusCode::NO_CONTENT);

    // Unauthenticated registration is rejected.
    let (unauth, _) = send(
        &app,
        build("POST", "/api/me/devices", None, Some(json!({ "token": "x" }))),
    )
    .await;
    assert_eq!(unauth, StatusCode::UNAUTHORIZED);
}

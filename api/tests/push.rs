//! Push outbox (TASK-017 M4): selection of un-pushed notifications fanned out to
//! the recipient's devices, and the delivery watermark.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use underground_api::push;
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

async fn register(app: &Router, email: &str) -> (String, String) {
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
    (
        v["token"].as_str().unwrap().to_string(),
        v["user"]["id"].as_str().unwrap().to_string(),
    )
}

#[sqlx::test(migrations = "./migrations")]
async fn outbox_selects_then_marks(pool: PgPool) {
    let app = make_app(pool.clone());
    let (a, a_id) = register(&app, "a@x.co").await;
    let (b, _) = register(&app, "b@x.co").await;

    // A registers TWO devices; B follows A → one notification must fan out to both.
    for tok in ["tok-A1", "tok-A2"] {
        send(
            &app,
            build("POST", "/api/me/devices", Some(&a), Some(json!({ "token": tok }))),
        )
        .await;
    }
    send(
        &app,
        build("POST", &format!("/api/users/{a_id}/follow"), Some(&b), None),
    )
    .await;

    // The single notification fans out to BOTH of A's devices (not truncated by the limit).
    let pending = push::pending(&pool, 10).await.unwrap();
    assert_eq!(pending.len(), 2, "one notification fanned out to A's two devices");
    let mut tokens: Vec<&str> = pending.iter().map(|p| p.token.as_str()).collect();
    tokens.sort();
    assert_eq!(tokens, vec!["tok-A1", "tok-A2"]);
    assert!(pending.iter().all(|p| p.kind == "new_follower"));

    // Marking the (single) notification clears both device rows.
    let ids: Vec<_> = pending.iter().map(|p| p.notification_id).collect();
    push::mark_pushed(&pool, &ids).await.unwrap();
    assert!(push::pending(&pool, 10).await.unwrap().is_empty());
}

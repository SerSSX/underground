//! Integration tests for direct messages: send, thread, conversations, unread.

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
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(t) = token {
        builder = builder.header("authorization", format!("Bearer {t}"));
    }
    let body = body.map(|v| Body::from(v.to_string())).unwrap_or_else(Body::empty);
    builder.body(body).unwrap()
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

async fn register(app: &Router, email: &str, name: &str) -> (String, String) {
    let (_, v) = send(
        app,
        build(
            "POST",
            "/api/auth/register",
            None,
            Some(json!({"email": email, "password": "password1", "name": name})),
        ),
    )
    .await;
    (
        v["token"].as_str().unwrap().to_string(),
        v["user"]["id"].as_str().unwrap().to_string(),
    )
}

async fn dm(app: &Router, token: &str, to: &str, text: &str) -> (StatusCode, Value) {
    send(
        app,
        build(
            "POST",
            "/api/messages",
            Some(token),
            Some(json!({"recipient_id": to, "text": text})),
        ),
    )
    .await
}

#[sqlx::test(migrations = "./migrations")]
async fn messages_require_auth(pool: PgPool) {
    let app = make_app(pool);
    let ghost = "00000000-0000-0000-0000-000000000000";
    let (status, _) = send(
        &app,
        build(
            "POST",
            "/api/messages",
            None,
            Some(json!({"recipient_id": ghost, "text":"hi"})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "[SEC] send without token → 401");
    let (status, _) = send(&app, build("GET", "/api/me/conversations", None, None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = send(&app, build("GET", &format!("/api/messages/{ghost}"), None, None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn send_validations(pool: PgPool) {
    let app = make_app(pool);
    let (alice, alice_id) = register(&app, "a@x.co", "Alice").await;

    let (status, _) = dm(&app, &alice, &alice_id, "сам собі").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "self-message rejected");

    let ghost = "00000000-0000-0000-0000-000000000000";
    let (status, _) = dm(&app, &alice, ghost, "хто там?").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unknown recipient → 404");

    let (maker, maker_id) = register(&app, "m@x.co", "Maker").await;
    let _ = maker;
    let (status, _) = dm(&app, &alice, &maker_id, "   ").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "blank text rejected");

    let long = "x".repeat(2001);
    let (status, _) = dm(&app, &alice, &maker_id, &long).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "too long rejected");
}

#[sqlx::test(migrations = "./migrations")]
async fn chat_flow_thread_conversations_unread(pool: PgPool) {
    let app = make_app(pool);
    let (alice, alice_id) = register(&app, "a@x.co", "Alice").await;
    let (maker, maker_id) = register(&app, "m@x.co", "Maker").await;

    let (status, msg) = dm(&app, &alice, &maker_id, "Привіт! Чи буде фотозона?").await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(msg["read"], false);
    dm(&app, &alice, &maker_id, "І ще: чи можна з собакою?").await;

    // Maker sees one conversation with 2 unread.
    let (_, convs) = send(&app, build("GET", "/api/me/conversations", Some(&maker), None)).await;
    let convs = convs.as_array().unwrap().clone();
    assert_eq!(convs.len(), 1);
    assert_eq!(convs[0]["peer_name"], "Alice");
    assert_eq!(convs[0]["unread"], 2);
    assert_eq!(convs[0]["last_from_me"], false);
    assert_eq!(convs[0]["last_text"], "І ще: чи можна з собакою?");

    // Maker opens the thread → messages marked read.
    let (_, thread) = send(
        &app,
        build("GET", &format!("/api/messages/{alice_id}"), Some(&maker), None),
    )
    .await;
    assert_eq!(thread.as_array().unwrap().len(), 2, "oldest first thread");

    let (_, convs) = send(&app, build("GET", "/api/me/conversations", Some(&maker), None)).await;
    assert_eq!(convs[0]["unread"], 0, "read after opening the thread");

    // Maker replies; alice sees the reply in her thread and 1 unread.
    dm(&app, &maker, &alice_id, "Так, фотозона буде!").await;
    let (_, convs) = send(&app, build("GET", "/api/me/conversations", Some(&alice), None)).await;
    assert_eq!(convs[0]["unread"], 1);
    assert_eq!(convs[0]["last_from_me"], false);
    let (_, thread) = send(
        &app,
        build("GET", &format!("/api/messages/{maker_id}"), Some(&alice), None),
    )
    .await;
    let thread = thread.as_array().unwrap();
    assert_eq!(thread.len(), 3);
    assert_eq!(thread[2]["text"], "Так, фотозона буде!");

    // Conversation list of alice shows last_from_me=false; after she writes again — true.
    dm(&app, &alice, &maker_id, "Дякую!").await;
    let (_, convs) = send(&app, build("GET", "/api/me/conversations", Some(&alice), None)).await;
    assert_eq!(convs[0]["last_from_me"], true);
}

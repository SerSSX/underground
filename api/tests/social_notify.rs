//! Social notifications (TASK-017 M4): like/comment/follow notify the target,
//! never the actor, and never duplicate on repeat actions.

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

async fn create_post(app: &Router, token: &str, text: &str) -> String {
    let (status, v) = send(
        app,
        build("POST", "/api/posts", Some(token), Some(json!({ "text": text }))),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    v["id"].as_str().unwrap().to_string()
}

async fn notifs(app: &Router, token: &str) -> Vec<Value> {
    let (_, v) = send(app, build("GET", "/api/me/notifications", Some(token), None)).await;
    v.as_array().unwrap().clone()
}

#[sqlx::test(migrations = "./migrations")]
async fn like_and_comment_notify_author_not_self(pool: PgPool) {
    let app = make_app(pool);
    let (a, _) = register(&app, "a@x.co").await;
    let (b, b_id) = register(&app, "b@x.co").await;
    let post = create_post(&app, &a, "hello").await;

    // B likes A's post → A notified. Repeat like is a no-op. A liking own post → no self-notify.
    send(&app, build("POST", &format!("/api/posts/{post}/like"), Some(&b), None)).await;
    send(&app, build("POST", &format!("/api/posts/{post}/like"), Some(&b), None)).await;
    send(&app, build("POST", &format!("/api/posts/{post}/like"), Some(&a), None)).await;
    // B comments → A notified. A commenting own post → no self-notify.
    send(
        &app,
        build(
            "POST",
            &format!("/api/posts/{post}/comments"),
            Some(&b),
            Some(json!({"text":"nice"})),
        ),
    )
    .await;
    send(
        &app,
        build(
            "POST",
            &format!("/api/posts/{post}/comments"),
            Some(&a),
            Some(json!({"text":"mine"})),
        ),
    )
    .await;

    let list = notifs(&app, &a).await;
    let likes: Vec<&Value> = list.iter().filter(|n| n["kind"] == "post_like").collect();
    assert_eq!(likes.len(), 1, "one like notification — not duplicated, not self");
    assert_eq!(likes[0]["actor_id"], b_id);
    assert!(likes[0]["title"].as_str().unwrap().contains("вподобав"));
    let comments: Vec<&Value> = list.iter().filter(|n| n["kind"] == "post_comment").collect();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0]["body"], "nice");
    assert_eq!(comments[0]["actor_id"], b_id);

    // The actor sees none of their own social actions.
    let b_list = notifs(&app, &b).await;
    assert!(b_list
        .iter()
        .all(|n| n["kind"] != "post_like" && n["kind"] != "post_comment"));
}

#[sqlx::test(migrations = "./migrations")]
async fn follow_notifies_target_once(pool: PgPool) {
    let app = make_app(pool);
    let (a, a_id) = register(&app, "a@x.co").await;
    let (b, _) = register(&app, "b@x.co").await;

    send(
        &app,
        build("POST", &format!("/api/users/{a_id}/follow"), Some(&b), None),
    )
    .await;
    // repeat follow → no second notification
    send(
        &app,
        build("POST", &format!("/api/users/{a_id}/follow"), Some(&b), None),
    )
    .await;

    let list = notifs(&app, &a).await;
    let f: Vec<&Value> = list.iter().filter(|n| n["kind"] == "new_follower").collect();
    assert_eq!(f.len(), 1);
    assert!(f[0]["title"].as_str().unwrap().contains("підписав"));
}

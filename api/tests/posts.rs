//! Integration tests for wall posts: manual + auto (event/review), likes, comments.

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

async fn create_event(app: &Router, token: &str, title: &str) -> String {
    let (_, v) = send(
        app,
        build(
            "POST",
            "/api/events",
            Some(token),
            Some(json!({"title": title, "category": "Rock", "price": "0", "tickets_total": 10})),
        ),
    )
    .await;
    v["id"].as_str().unwrap().to_string()
}

async fn wall(app: &Router, user_id: &str, token: Option<&str>) -> Vec<Value> {
    let (status, v) = send(app, build("GET", &format!("/api/users/{user_id}/posts"), token, None)).await;
    assert_eq!(status, StatusCode::OK);
    v.as_array().unwrap().clone()
}

#[sqlx::test(migrations = "./migrations")]
async fn post_requires_auth(pool: PgPool) {
    let app = make_app(pool);
    let (status, _) = send(&app, build("POST", "/api/posts", None, Some(json!({"text":"hi"})))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "[SEC] post without token → 401");
}

#[sqlx::test(migrations = "./migrations")]
async fn manual_post_appears_on_public_wall(pool: PgPool) {
    let app = make_app(pool);
    let (token, user_id) = register(&app, "a@x.co", "Аліса").await;

    let (status, post) = send(
        &app,
        build(
            "POST",
            "/api/posts",
            Some(&token),
            Some(json!({"text":" Перший пост! "})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(post["text"], "Перший пост!", "trimmed");
    assert_eq!(post["kind"], "text");
    assert_eq!(post["author_name"], "Аліса");
    assert_eq!(post["likes_count"], 0);

    let posts = wall(&app, &user_id, None).await;
    assert_eq!(posts.len(), 1, "wall is public");
}

#[sqlx::test(migrations = "./migrations")]
async fn post_validation(pool: PgPool) {
    let app = make_app(pool);
    let (token, _) = register(&app, "a@x.co", "A").await;

    let (status, _) = send(
        &app,
        build("POST", "/api/posts", Some(&token), Some(json!({"text":"  "}))),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "blank text rejected");

    let long = "x".repeat(2001);
    let (status, _) = send(
        &app,
        build("POST", "/api/posts", Some(&token), Some(json!({"text": long}))),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "too long rejected");

    // Attaching a foreign event is not allowed for manual posts.
    let (maker, _) = register(&app, "m@x.co", "M").await;
    let event_id = create_event(&app, &maker, "Not mine").await;
    let (status, _) = send(
        &app,
        build(
            "POST",
            "/api/posts",
            Some(&token),
            Some(json!({"text":"x","event_id": event_id})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "./migrations")]
async fn creating_event_auto_posts_announcement(pool: PgPool) {
    let app = make_app(pool);
    let (maker, maker_id) = register(&app, "m@x.co", "Maker").await;
    create_event(&app, &maker, "Великий концерт").await;

    let posts = wall(&app, &maker_id, None).await;
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0]["kind"], "event_share");
    assert_eq!(posts[0]["event_title"], "Великий концерт", "event card attached");
}

#[sqlx::test(migrations = "./migrations")]
async fn review_auto_posts_and_replaces_on_rereview(pool: PgPool) {
    let app = make_app(pool);
    let (maker, _) = register(&app, "m@x.co", "Maker").await;
    let (fan, fan_id) = register(&app, "f@x.co", "Fan").await;
    let event_id = create_event(&app, &maker, "Gig").await;

    send(
        &app,
        build(
            "POST",
            "/api/reviews",
            Some(&fan),
            Some(json!({"event_id": event_id, "rating": 5, "text": "Вогонь!"})),
        ),
    )
    .await;
    let posts = wall(&app, &fan_id, None).await;
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0]["kind"], "review");
    assert_eq!(posts[0]["rating"], 5);
    assert_eq!(posts[0]["text"], "Вогонь!");
    assert_eq!(posts[0]["event_title"], "Gig");

    // Re-review: the wall keeps a single, updated review post.
    send(
        &app,
        build(
            "POST",
            "/api/reviews",
            Some(&fan),
            Some(json!({"event_id": event_id, "rating": 3, "text": "Передумав"})),
        ),
    )
    .await;
    let posts = wall(&app, &fan_id, None).await;
    assert_eq!(posts.len(), 1, "review post replaced, not duplicated");
    assert_eq!(posts[0]["rating"], 3);
}

#[sqlx::test(migrations = "./migrations")]
async fn likes_lifecycle(pool: PgPool) {
    let app = make_app(pool);
    let (author, author_id) = register(&app, "a@x.co", "A").await;
    let (fan, _) = register(&app, "f@x.co", "F").await;
    let (_, post) = send(
        &app,
        build("POST", "/api/posts", Some(&author), Some(json!({"text":"hi"}))),
    )
    .await;
    let post_id = post["id"].as_str().unwrap();

    let (status, _) = send(&app, build("POST", &format!("/api/posts/{post_id}/like"), None, None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "[SEC] like without token → 401");

    let (status, _) = send(
        &app,
        build("POST", &format!("/api/posts/{post_id}/like"), Some(&fan), None),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    send(
        &app,
        build("POST", &format!("/api/posts/{post_id}/like"), Some(&fan), None),
    )
    .await;

    let posts = wall(&app, &author_id, Some(&fan)).await;
    assert_eq!(posts[0]["likes_count"], 1, "idempotent like");
    assert_eq!(posts[0]["liked_by_me"], true);

    let anon = wall(&app, &author_id, None).await;
    assert_eq!(anon[0]["liked_by_me"], false, "anonymous viewer sees no own-like");

    let (status, _) = send(
        &app,
        build("DELETE", &format!("/api/posts/{post_id}/like"), Some(&fan), None),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let posts = wall(&app, &author_id, Some(&fan)).await;
    assert_eq!(posts[0]["likes_count"], 0);

    let ghost = "00000000-0000-0000-0000-000000000000";
    let (status, _) = send(
        &app,
        build("POST", &format!("/api/posts/{ghost}/like"), Some(&fan), None),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unknown post → 404");
}

#[sqlx::test(migrations = "./migrations")]
async fn comments_lifecycle(pool: PgPool) {
    let app = make_app(pool);
    let (author, author_id) = register(&app, "a@x.co", "A").await;
    let (fan, _) = register(&app, "f@x.co", "Фан").await;
    let (_, post) = send(
        &app,
        build("POST", "/api/posts", Some(&author), Some(json!({"text":"hi"}))),
    )
    .await;
    let post_id = post["id"].as_str().unwrap();

    let (status, comment) = send(
        &app,
        build(
            "POST",
            &format!("/api/posts/{post_id}/comments"),
            Some(&fan),
            Some(json!({"text":"Клас!"})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(comment["author_name"], "Фан");

    let (status, list) = send(
        &app,
        build("GET", &format!("/api/posts/{post_id}/comments"), None, None),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "comments are public");
    assert_eq!(list.as_array().unwrap().len(), 1);

    let posts = wall(&app, &author_id, None).await;
    assert_eq!(posts[0]["comments_count"], 1);

    let (status, _) = send(
        &app,
        build(
            "POST",
            &format!("/api/posts/{post_id}/comments"),
            Some(&fan),
            Some(json!({"text":"  "})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "blank comment rejected");

    let long = "x".repeat(2001);
    let (status, _) = send(
        &app,
        build(
            "POST",
            &format!("/api/posts/{post_id}/comments"),
            Some(&fan),
            Some(json!({"text": long})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "long comment rejected");

    let ghost = "00000000-0000-0000-0000-000000000000";
    let (status, _) = send(
        &app,
        build(
            "POST",
            &format!("/api/posts/{ghost}/comments"),
            Some(&fan),
            Some(json!({"text":"x"})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "./migrations")]
async fn delete_post_owner_only(pool: PgPool) {
    let app = make_app(pool);
    let (author, author_id) = register(&app, "a@x.co", "A").await;
    let (other, _) = register(&app, "o@x.co", "O").await;
    let (_, post) = send(
        &app,
        build("POST", "/api/posts", Some(&author), Some(json!({"text":"hi"}))),
    )
    .await;
    let post_id = post["id"].as_str().unwrap();

    let (status, _) = send(
        &app,
        build("DELETE", &format!("/api/posts/{post_id}"), Some(&other), None),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "[SEC] not the author → 404");

    let (status, _) = send(
        &app,
        build("DELETE", &format!("/api/posts/{post_id}"), Some(&author), None),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(wall(&app, &author_id, None).await.is_empty());
}

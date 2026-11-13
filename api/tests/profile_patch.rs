//! PATCH /api/me — self-serve profile edit + organizer label (TASK-015).

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

async fn register(app: &Router, email: &str) -> (String, String) {
    let (_, v) = send(
        app,
        build(
            "POST",
            "/api/auth/register",
            None,
            Some(json!({"email":email,"password":"password1","name":"Original"})),
        ),
    )
    .await;
    (
        v["token"].as_str().unwrap().to_string(),
        v["user"]["id"].as_str().unwrap().to_string(),
    )
}

#[sqlx::test(migrations = "./migrations")]
async fn new_user_defaults_to_not_organizer(pool: PgPool) {
    let app = make_app(pool);
    let (token, _) = register(&app, "a@x.co").await;
    let (_, me) = send(&app, build("GET", "/api/me", Some(&token), None)).await;
    assert_eq!(me["is_organizer"], false);
}

#[sqlx::test(migrations = "./migrations")]
async fn patch_requires_auth(pool: PgPool) {
    let app = make_app(pool);
    let (status, _) = send(&app, build("PATCH", "/api/me", None, Some(json!({"name":"X"})))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "[SEC] patch without token → 401");
}

#[sqlx::test(migrations = "./migrations")]
async fn patch_updates_fields_and_toggles_organizer(pool: PgPool) {
    let app = make_app(pool);
    let (token, user_id) = register(&app, "a@x.co").await;

    let (status, me) = send(
        &app,
        build(
            "PATCH",
            "/api/me",
            Some(&token),
            Some(json!({"name":" Олег ","bio":" Промоутер ","is_organizer": true})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["name"], "Олег", "name trimmed");
    assert_eq!(me["bio"], "Промоутер");
    assert_eq!(me["is_organizer"], true);

    // Persisted: a fresh GET reflects it, and the public profile shows the organizer flag.
    let (_, me2) = send(&app, build("GET", "/api/me", Some(&token), None)).await;
    assert_eq!(me2["name"], "Олег");
    let (_, profile) = send(&app, build("GET", &format!("/api/users/{user_id}"), None, None)).await;
    assert_eq!(profile["is_organizer"], true);

    // Partial patch leaves other fields untouched; organizer can be turned back off.
    let (_, me3) = send(
        &app,
        build("PATCH", "/api/me", Some(&token), Some(json!({"is_organizer": false}))),
    )
    .await;
    assert_eq!(me3["is_organizer"], false);
    assert_eq!(me3["name"], "Олег", "name unchanged by partial patch");

    // Avatar can be set; a blank avatar string is ignored (keeps the current one). TASK-305 R4b:
    // only our own upload URL is accepted, so it is built from PUBLIC_BASE_URL (was the literal
    // "http://localhost:8080/uploads/a.png", which only matches when that is the base).
    let avatar = format!(
        "{}/uploads/a.png",
        underground_api::uploads::public_base_url().trim_end_matches('/')
    );
    let (_, me4) = send(
        &app,
        build("PATCH", "/api/me", Some(&token), Some(json!({ "avatar_url": avatar }))),
    )
    .await;
    assert_eq!(me4["avatar_url"], avatar.as_str());
    let (_, me5) = send(
        &app,
        build("PATCH", "/api/me", Some(&token), Some(json!({"avatar_url": "  "}))),
    )
    .await;
    assert_eq!(me5["avatar_url"], avatar.as_str(), "blank avatar ignored");
}

#[sqlx::test(migrations = "./migrations")]
async fn patch_validates(pool: PgPool) {
    let app = make_app(pool);
    let (token, _) = register(&app, "a@x.co").await;

    let (status, _) = send(
        &app,
        build("PATCH", "/api/me", Some(&token), Some(json!({"name":"  "}))),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "blank name rejected");

    let long_bio = "x".repeat(501);
    let (status, _) = send(
        &app,
        build("PATCH", "/api/me", Some(&token), Some(json!({"bio": long_bio}))),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "long bio rejected");

    let long_name = "y".repeat(81);
    let (status, _) = send(
        &app,
        build("PATCH", "/api/me", Some(&token), Some(json!({"name": long_name}))),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "long name rejected");
}

#[sqlx::test(migrations = "./migrations")]
async fn empty_patch_is_a_noop(pool: PgPool) {
    let app = make_app(pool);
    let (token, _) = register(&app, "a@x.co").await;
    let (status, me) = send(&app, build("PATCH", "/api/me", Some(&token), Some(json!({})))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["name"], "Original");
    assert!(me["bio"].is_null());
}

//! Notification localization (TASK-017 M5): server-generated notification strings
//! are rendered in the RECIPIENT's locale (uk default, en when set).

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

async fn set_locale(app: &Router, token: &str, categories: Value, locale: &str) {
    send(
        app,
        build(
            "PUT",
            "/api/me/prefs",
            Some(token),
            Some(json!({"categories": categories, "locale": locale})),
        ),
    )
    .await;
}

async fn notifs(app: &Router, token: &str) -> Vec<Value> {
    let (_, v) = send(app, build("GET", "/api/me/notifications", Some(token), None)).await;
    v.as_array().unwrap().clone()
}

#[sqlx::test(migrations = "./migrations")]
async fn social_notification_localizes_to_recipient(pool: PgPool) {
    let app = make_app(pool);
    let (a, a_id) = register(&app, "a@x.co").await;
    let (b, _) = register(&app, "b@x.co").await;
    set_locale(&app, &a, json!([]), "en").await;

    send(
        &app,
        build("POST", &format!("/api/users/{a_id}/follow"), Some(&b), None),
    )
    .await;

    let list = notifs(&app, &a).await;
    let f = list
        .iter()
        .find(|n| n["kind"] == "new_follower")
        .expect("follower notification");
    assert!(
        f["title"].as_str().unwrap().ends_with("followed you"),
        "english suffix, got {}",
        f["title"]
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn event_notifications_localize_to_recipient(pool: PgPool) {
    let app = make_app(pool);
    let (maker, maker_id) = register(&app, "m@x.co").await;
    let (matcher, _) = register(&app, "match@x.co").await;
    let (follower, _) = register(&app, "fol@x.co").await;

    // matcher: interested in Rock, English. follower: follows the maker, English.
    set_locale(&app, &matcher, json!(["Rock"]), "en").await;
    set_locale(&app, &follower, json!([]), "en").await;
    send(
        &app,
        build("POST", &format!("/api/users/{maker_id}/follow"), Some(&follower), None),
    )
    .await;

    // maker announces a Rock event.
    let (status, _) = send(
        &app,
        build(
            "POST",
            "/api/events",
            Some(&maker),
            Some(json!({"title":"Show","category":"Rock","city":"Kyiv","price":"0","tickets_total":10,"cost_type":"free"})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // matcher (not a follower) gets the English event-match title.
    let m = notifs(&app, &matcher).await;
    assert!(
        m.iter()
            .any(|n| n["kind"] == "event_match" && n["title"] == "New event: Show"),
        "english event_match, got {m:?}"
    );
    // follower gets the English organizer announcement (and not a duplicate event_match).
    let f = notifs(&app, &follower).await;
    assert!(
        f.iter()
            .any(|n| n["kind"] == "organizer_event" && n["title"].as_str().unwrap().ends_with("announced an event")),
        "english organizer_event, got {f:?}"
    );
}

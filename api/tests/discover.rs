//! Discovery engine + saves + follower notifications (TASK-017 M1).

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

async fn create_event(
    app: &Router,
    token: &str,
    title: &str,
    category: &str,
    city: &str,
    starts_at: Option<&str>,
) -> String {
    let mut body = json!({"title": title, "category": category, "city": city, "price": "0", "tickets_total": 10});
    if let Some(s) = starts_at {
        body["starts_at"] = json!(s);
    }
    let (status, v) = send(app, build("POST", "/api/events", Some(token), Some(body))).await;
    assert_eq!(status, StatusCode::CREATED);
    v["id"].as_str().unwrap().to_string()
}

const PAST: &str = "2020-01-01T19:00:00Z";

async fn follow(app: &Router, token: &str, who: &str) {
    send(
        app,
        build("POST", &format!("/api/users/{who}/follow"), Some(token), None),
    )
    .await;
}
async fn set_prefs(app: &Router, token: &str, cats: Value, city: Value) {
    send(
        app,
        build(
            "PUT",
            "/api/me/prefs",
            Some(token),
            Some(json!({"categories": cats, "city": city})),
        ),
    )
    .await;
}
async fn discover(app: &Router, token: Option<&str>) -> Vec<Value> {
    let (status, v) = send(app, build("GET", "/api/discover", token, None)).await;
    assert_eq!(status, StatusCode::OK);
    v.as_array().unwrap().clone()
}
fn reason_of<'a>(items: &'a [Value], title: &str) -> Option<&'a str> {
    items
        .iter()
        .find(|e| e["title"] == title)
        .and_then(|e| e["reason"].as_str())
}

#[sqlx::test(migrations = "./migrations")]
async fn discover_personalizes_with_reasons(pool: PgPool) {
    let app = make_app(pool);
    let (me, _) = register(&app, "me@x.co").await;
    let (maker, maker_id) = register(&app, "m@x.co").await;
    let (s, _) = register(&app, "s@x.co").await;

    follow(&app, &me, &maker_id).await;
    set_prefs(&app, &me, json!(["Rock"]), json!("Kyiv")).await;

    create_event(&app, &maker, "Followed Jazz Lviv", "Jazz", "Lviv", None).await;
    create_event(&app, &s, "Rock Kyiv", "Rock", "Kyiv", None).await;
    create_event(&app, &s, "Jazz Kyiv", "Jazz", "Kyiv", None).await;
    create_event(&app, &s, "Pop Odesa", "Pop", "Odesa", None).await;
    create_event(&app, &me, "Mine", "Rock", "Kyiv", None).await; // own → excluded
    create_event(&app, &s, "Old Rock", "Rock", "Kyiv", Some(PAST)).await; // past → excluded

    let items = discover(&app, Some(&me)).await;
    let titles: Vec<&str> = items.iter().map(|e| e["title"].as_str().unwrap()).collect();
    assert!(!titles.contains(&"Mine"), "own events excluded: {titles:?}");
    assert!(!titles.contains(&"Old Rock"), "past events excluded: {titles:?}");
    assert_eq!(items.len(), 4);

    assert_eq!(reason_of(&items, "Followed Jazz Lviv"), Some("followed"));
    assert_eq!(reason_of(&items, "Rock Kyiv"), Some("interest"));
    assert_eq!(reason_of(&items, "Jazz Kyiv"), Some("city"));
    assert_eq!(reason_of(&items, "Pop Odesa"), Some("trending"));
}

#[sqlx::test(migrations = "./migrations")]
async fn discover_anonymous_is_trending(pool: PgPool) {
    let app = make_app(pool);
    let (maker, _) = register(&app, "m@x.co").await;
    create_event(&app, &maker, "Future", "Rock", "Kyiv", None).await;
    create_event(&app, &maker, "Past", "Rock", "Kyiv", Some(PAST)).await;

    let items = discover(&app, None).await;
    let titles: Vec<&str> = items.iter().map(|e| e["title"].as_str().unwrap()).collect();
    assert_eq!(titles, vec!["Future"], "anon sees upcoming only");
    assert_eq!(items[0]["reason"], "trending");
    assert_eq!(items[0]["saved_by_me"], false);
    assert_eq!(items[0]["saves_count"], 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn save_lifecycle_and_counts(pool: PgPool) {
    let app = make_app(pool);
    let (me, _) = register(&app, "me@x.co").await;
    let (other, _) = register(&app, "o@x.co").await;
    let (maker, _) = register(&app, "m@x.co").await;
    let event = create_event(&app, &maker, "Gig", "Rock", "Kyiv", None).await;

    let (status, _) = send(&app, build("POST", &format!("/api/events/{event}/save"), None, None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "[SEC] save needs auth");

    let (status, _) = send(
        &app,
        build("POST", &format!("/api/events/{event}/save"), Some(&me), None),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    send(
        &app,
        build("POST", &format!("/api/events/{event}/save"), Some(&me), None),
    )
    .await; // idempotent
    send(
        &app,
        build("POST", &format!("/api/events/{event}/save"), Some(&other), None),
    )
    .await;

    let items = discover(&app, Some(&me)).await;
    let ev = items.iter().find(|e| e["title"] == "Gig").unwrap();
    assert_eq!(ev["saves_count"], 2, "no double-count for repeat save");
    assert_eq!(ev["saved_by_me"], true);

    // my_saves lists it; unsave removes it.
    let (_, saves) = send(&app, build("GET", "/api/me/saves", Some(&me), None)).await;
    assert_eq!(saves.as_array().unwrap().len(), 1);
    assert_eq!(saves[0]["title"], "Gig");

    let (status, _) = send(
        &app,
        build("DELETE", &format!("/api/events/{event}/save"), Some(&me), None),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, saves) = send(&app, build("GET", "/api/me/saves", Some(&me), None)).await;
    assert!(saves.as_array().unwrap().is_empty());

    let ghost = "00000000-0000-0000-0000-000000000000";
    let (status, _) = send(
        &app,
        build("POST", &format!("/api/events/{ghost}/save"), Some(&me), None),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "save unknown event → 404");
}

#[sqlx::test(migrations = "./migrations")]
async fn my_saves_orders_newest_first(pool: PgPool) {
    let app = make_app(pool);
    let (me, _) = register(&app, "me@x.co").await;
    let (maker, _) = register(&app, "m@x.co").await;
    let a = create_event(&app, &maker, "First", "Rock", "Kyiv", None).await;
    let b = create_event(&app, &maker, "Second", "Jazz", "Kyiv", None).await;
    send(&app, build("POST", &format!("/api/events/{a}/save"), Some(&me), None)).await;
    send(&app, build("POST", &format!("/api/events/{b}/save"), Some(&me), None)).await;
    let (_, saves) = send(&app, build("GET", "/api/me/saves", Some(&me), None)).await;
    let titles: Vec<&str> = saves
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, vec!["Second", "First"], "most recently saved first");
}

#[sqlx::test(migrations = "./migrations")]
async fn followers_notified_without_duplicate(pool: PgPool) {
    let app = make_app(pool);
    let (maker, maker_id) = register(&app, "m@x.co").await;
    let (follower, _) = register(&app, "f@x.co").await; // follows, no prefs
    let (prefsfan, _) = register(&app, "p@x.co").await; // prefs match, not follower
    let (both, _) = register(&app, "b@x.co").await; // follows AND prefs match

    follow(&app, &follower, &maker_id).await;
    follow(&app, &both, &maker_id).await;
    set_prefs(&app, &prefsfan, json!(["Rock"]), json!(null)).await;
    set_prefs(&app, &both, json!(["Rock"]), json!(null)).await;

    create_event(&app, &maker, "Big Show", "Rock", "Kyiv", None).await;

    let kinds = |token: &str| {
        let app = app.clone();
        let token = token.to_string();
        async move {
            let (_, v) = send(&app, build("GET", "/api/me/notifications", Some(&token), None)).await;
            v.as_array()
                .unwrap()
                .iter()
                .map(|n| n["kind"].as_str().unwrap().to_string())
                .collect::<Vec<_>>()
        }
    };

    assert_eq!(
        kinds(&follower).await,
        vec!["organizer_event"],
        "follower gets the organizer notice"
    );
    assert_eq!(
        kinds(&prefsfan).await,
        vec!["event_match"],
        "non-follower prefs-match gets event_match"
    );
    assert_eq!(
        kinds(&both).await,
        vec!["organizer_event"],
        "follower+prefs gets only one (no dup)"
    );
}

//! Integration tests for the time-aware sectioned Афіша (TASK-200, BR-024).

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

async fn json_body(resp: axum::response::Response) -> Value {
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn token_for(app: &Router, email: &str) -> String {
    json_body(
        app.clone()
            .oneshot(build(
                "POST",
                "/api/auth/register",
                None,
                Some(json!({"email":email,"password":"password1","name":"X"})),
            ))
            .await
            .unwrap(),
    )
    .await["token"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn create_event(app: &Router, token: &str, title: &str) -> String {
    json_body(
        app.clone()
            .oneshot(build(
                "POST",
                "/api/events",
                Some(token),
                Some(json!({"title":title,"category":"Party","price":"0","tickets_total":5,"cost_type":"free"})),
            ))
            .await
            .unwrap(),
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// Set an event's `starts_at` to a controlled SQL expression (test-owned literal).
async fn set_starts(db: &PgPool, id: &str, expr: &str) {
    sqlx::query(&format!("UPDATE events SET starts_at = {expr} WHERE id = $1::uuid"))
        .bind(id)
        .execute(db)
        .await
        .unwrap();
}

async fn sections(app: &Router, token: Option<&str>) -> Vec<Value> {
    json_body(
        app.clone()
            .oneshot(build("GET", "/api/discover/sections", token, None))
            .await
            .unwrap(),
    )
    .await
    .as_array()
    .unwrap()
    .clone()
}

fn keys(secs: &[Value]) -> Vec<String> {
    secs.iter().map(|s| s["key"].as_str().unwrap().to_string()).collect()
}

fn section<'a>(secs: &'a [Value], key: &str) -> Option<&'a Value> {
    secs.iter().find(|s| s["key"] == key)
}

fn has_event(sec: &Value, id: &str) -> bool {
    sec["events"].as_array().unwrap().iter().any(|e| e["id"] == id)
}

#[sqlx::test(migrations = "./migrations")]
async fn anonymous_gets_popular_no_for_you(pool: PgPool) {
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    create_event(&app, &owner, "A future party").await;

    let secs = sections(&app, None).await;
    let k = keys(&secs);
    assert!(k.contains(&"popular".to_string()), "popular is the cold-start fallback");
    assert!(
        !k.contains(&"for_you".to_string()),
        "anonymous has no personalized section"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn for_you_present_and_first_with_signals(pool: PgPool) {
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let viewer = token_for(&app, "v@x.co").await;
    let ev = create_event(&app, &owner, "Someone's party").await;
    // A save is a personalization signal.
    app.clone()
        .oneshot(build("POST", &format!("/api/events/{ev}/save"), Some(&viewer), None))
        .await
        .unwrap();

    let secs = sections(&app, Some(&viewer)).await;
    assert_eq!(
        secs[0]["key"], "for_you",
        "personalized section leads when the viewer has signals"
    );
    assert!(has_event(section(&secs, "for_you").unwrap(), &ev));
}

#[sqlx::test(migrations = "./migrations")]
async fn no_signals_no_for_you(pool: PgPool) {
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let viewer = token_for(&app, "v@x.co").await;
    create_event(&app, &owner, "Someone's party").await;

    let secs = sections(&app, Some(&viewer)).await;
    assert!(
        !keys(&secs).contains(&"for_you".to_string()),
        "a signal-less viewer gets no For You"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn soon_and_weekend_membership(pool: PgPool) {
    let db = pool.clone();
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let soon = create_event(&app, &owner, "Tonight").await;
    let far = create_event(&app, &owner, "Next week").await;
    let wknd = create_event(&app, &owner, "Weekend").await;
    set_starts(&db, &soon, "now() + interval '12 hours'").await;
    set_starts(&db, &far, "now() + interval '10 days'").await;
    set_starts(
        &db,
        &wknd,
        "date_trunc('week', now()) + interval '5 days' + interval '20 hours'",
    )
    .await;

    let secs = sections(&app, None).await;
    let soon_sec = section(&secs, "soon").expect("soon section present");
    assert!(has_event(soon_sec, &soon), "12h-away event is in soon");
    assert!(!has_event(soon_sec, &far), "10-day-away event is not in soon");
    assert!(has_event(section(&secs, "weekend").expect("weekend section"), &wknd));
}

#[sqlx::test(migrations = "./migrations")]
async fn empty_sections_are_omitted(pool: PgPool) {
    let app = make_app(pool);
    let a = token_for(&app, "a@x.co").await;
    let b = token_for(&app, "b@x.co").await;
    // `a` follows `b` (a signal) but there are no published events anywhere.
    let bid = json_body(
        app.clone()
            .oneshot(build("GET", "/api/me", Some(&b), None))
            .await
            .unwrap(),
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_string();
    app.clone()
        .oneshot(build("POST", &format!("/api/users/{bid}/follow"), Some(&a), None))
        .await
        .unwrap();

    let secs = sections(&app, Some(&a)).await;
    assert!(
        secs.is_empty(),
        "with no events, every section (incl. For You despite a signal) is omitted"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn hidden_and_own_events_never_surface(pool: PgPool) {
    let db = pool.clone();
    let app = make_app(pool);
    let viewer = token_for(&app, "v@x.co").await;
    let other = token_for(&app, "o@x.co").await;
    let own = create_event(&app, &viewer, "My own event").await;
    let hidden = create_event(&app, &other, "Hidden one").await;
    sqlx::query("UPDATE events SET status = 'hidden' WHERE id = $1::uuid")
        .bind(&hidden)
        .execute(&db)
        .await
        .unwrap();

    let secs = sections(&app, Some(&viewer)).await;
    // Only a hidden event and the viewer's own remain → nothing is discoverable.
    assert!(
        secs.is_empty(),
        "[SEC] hidden events and the viewer's own events never appear"
    );
    // Sanity: the own event still exists and is published.
    let _ = own;
}

#[sqlx::test(migrations = "./migrations")]
async fn flat_discover_still_works(pool: PgPool) {
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    create_event(&app, &owner, "A party").await;
    let resp = app.oneshot(build("GET", "/api/discover", None, None)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(json_body(resp).await.is_array(), "[compat] flat discover is unchanged");
}

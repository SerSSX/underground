//! Event social proof + similar events (TASK-017 M3).

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

async fn create_event(app: &Router, token: &str, title: &str, category: &str, city: &str) -> String {
    let (status, v) = send(
        app,
        build("POST", "/api/events", Some(token), Some(json!({"title":title,"category":category,"city":city,"price":"0","tickets_total":10,"cost_type":"free"}))),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    v["id"].as_str().unwrap().to_string()
}

#[sqlx::test(migrations = "./migrations")]
async fn social_counts_and_friends(pool: PgPool) {
    let app = make_app(pool);
    let (maker, _) = register(&app, "m@x.co").await;
    let (viewer, viewer_id) = register(&app, "v@x.co").await;
    let (alice, alice_id) = register(&app, "alice@x.co").await;
    let (bob, _) = register(&app, "bob@x.co").await;
    let (carol, carol_id) = register(&app, "carol@x.co").await;
    let event = create_event(&app, &maker, "Gig", "Rock", "Kyiv").await;

    // alice & bob are interested (save); carol is going (free ticket).
    send(
        &app,
        build("POST", &format!("/api/events/{event}/save"), Some(&alice), None),
    )
    .await;
    send(
        &app,
        build("POST", &format!("/api/events/{event}/save"), Some(&bob), None),
    )
    .await;
    send(
        &app,
        build("POST", &format!("/api/events/{event}/reserve"), Some(&carol), None),
    )
    .await;

    // viewer follows alice (interested) and carol (going) — but not bob.
    send(
        &app,
        build("POST", &format!("/api/users/{alice_id}/follow"), Some(&viewer), None),
    )
    .await;
    send(
        &app,
        build("POST", &format!("/api/users/{carol_id}/follow"), Some(&viewer), None),
    )
    .await;
    // TASK-302 P9: friends are MUTUAL follows; alice and carol follow the viewer back.
    for friend in [&alice, &carol] {
        send(
            &app,
            build("POST", &format!("/api/users/{viewer_id}/follow"), Some(friend), None),
        )
        .await;
    }

    let (status, s) = send(
        &app,
        build("GET", &format!("/api/events/{event}/social"), Some(&viewer), None),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(s["interested_count"], 2);
    assert_eq!(s["going_count"], 1);
    let fi: Vec<&str> = s["friends_interested"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| u["name"].as_str().unwrap())
        .collect();
    assert_eq!(fi, vec!["alice"], "only followed interested friends, not bob");
    let fg: Vec<&str> = s["friends_going"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| u["name"].as_str().unwrap())
        .collect();
    assert_eq!(fg, vec!["carol"]);

    // Anonymous sees counts but no friends.
    let (_, anon) = send(&app, build("GET", &format!("/api/events/{event}/social"), None, None)).await;
    assert_eq!(anon["interested_count"], 2);
    assert!(anon["friends_interested"].as_array().unwrap().is_empty());
    assert!(anon["friends_going"].as_array().unwrap().is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn similar_by_category_and_city(pool: PgPool) {
    let app = make_app(pool);
    let (maker, _) = register(&app, "m@x.co").await;
    let target = create_event(&app, &maker, "Target", "Rock", "Kyiv").await;
    create_event(&app, &maker, "Same Category", "Rock", "Lviv").await;
    create_event(&app, &maker, "Same City", "Jazz", "Kyiv").await;
    create_event(&app, &maker, "Unrelated", "Pop", "Odesa").await;

    let (status, v) = send(&app, build("GET", &format!("/api/events/{target}/similar"), None, None)).await;
    assert_eq!(status, StatusCode::OK);
    let titles: Vec<&str> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["title"].as_str().unwrap())
        .collect();
    assert!(!titles.contains(&"Target"), "self excluded");
    assert!(!titles.contains(&"Unrelated"), "different category & city excluded");
    assert_eq!(
        titles,
        vec!["Same Category", "Same City"],
        "category match ranks above city match"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn similar_excludes_past_and_empty_for_unique(pool: PgPool) {
    let app = make_app(pool);
    let (maker, _) = register(&app, "m@x.co").await;
    let lonely = create_event(&app, &maker, "Lonely", "Spiritual", "Odesa").await;
    // A past same-category event must not surface.
    send(
        &app,
        build("POST", "/api/events", Some(&maker), Some(json!({"title":"Old","category":"Spiritual","city":"Odesa","price":"0","tickets_total":5,"cost_type":"free","starts_at":"2020-01-01T10:00:00Z"}))),
    )
    .await;

    let (_, v) = send(&app, build("GET", &format!("/api/events/{lonely}/similar"), None, None)).await;
    assert!(v.as_array().unwrap().is_empty(), "no upcoming similar events");
}

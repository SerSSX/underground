//! Integration tests for the activity feed: follows ∪ interest categories.

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

async fn create_event(app: &Router, token: &str, title: &str, category: &str) -> String {
    let (_, v) = send(
        app,
        build(
            "POST",
            "/api/events",
            Some(token),
            Some(json!({"title": title, "category": category, "price": "0", "tickets_total": 10})),
        ),
    )
    .await;
    v["id"].as_str().unwrap().to_string()
}

/// TASK-305 F9: only a guest reviews, so a reviewer first holds a place (setup only).
async fn give_ticket(pool: &PgPool, event: &str, email: &str) {
    sqlx::query(
        "INSERT INTO tickets (event_id, owner_id, qr_token, price, status) \
         SELECT $1::uuid, id, 'UG-' || gen_random_uuid(), 0, 'active' FROM users WHERE email = $2",
    )
    .bind(event)
    .bind(email)
    .execute(pool)
    .await
    .unwrap();
}

async fn feed(app: &Router, token: &str) -> Vec<Value> {
    let (status, v) = send(app, build("GET", "/api/feed", Some(token), None)).await;
    assert_eq!(status, StatusCode::OK);
    v.as_array().unwrap().clone()
}

#[sqlx::test(migrations = "./migrations")]
async fn feed_requires_auth(pool: PgPool) {
    let app = make_app(pool);
    let (status, _) = send(&app, build("GET", "/api/feed", None, None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "[SEC] feed without token → 401");
}

#[sqlx::test(migrations = "./migrations")]
async fn feed_combines_follows_and_interests(pool: PgPool) {
    let app = make_app(pool.clone());
    let (me, _) = register(&app, "me@x.co", "Me").await;
    let (followed, followed_id) = register(&app, "fm@x.co", "Followed Maker").await;
    let (rocker, _) = register(&app, "rk@x.co", "Rock Stranger").await;
    let (stranger, _) = register(&app, "st@x.co", "Jazz Stranger").await;

    // My setup: follow one maker; interests = Rock.
    send(
        &app,
        build("POST", &format!("/api/users/{followed_id}/follow"), Some(&me), None),
    )
    .await;
    send(
        &app,
        build(
            "PUT",
            "/api/me/prefs",
            Some(&me),
            Some(json!({"categories": ["Rock"], "city": null})),
        ),
    )
    .await;

    // Followed maker: a Jazz event (announcement post) + a plain text post.
    create_event(&app, &followed, "Jazz Night", "Jazz").await;
    send(
        &app,
        build(
            "POST",
            "/api/posts",
            Some(&followed),
            Some(json!({"text":"Готуємо новий лайнап"})),
        ),
    )
    .await;
    // Rock stranger: a Rock event → its announcement matches my interests.
    let rock_event = create_event(&app, &rocker, "Rock Fest", "Rock").await;
    give_ticket(&pool, &rock_event, "st@x.co").await;
    // Jazz stranger: review on the Rock event (interest match) + own Jazz event (no match).
    send(
        &app,
        build(
            "POST",
            "/api/reviews",
            Some(&stranger),
            Some(json!({"event_id": rock_event, "rating": 5, "text": "Незабутньо"})),
        ),
    )
    .await;
    create_event(&app, &stranger, "Jazz Brunch", "Jazz").await;
    // My own post must not appear in my feed.
    send(
        &app,
        build("POST", "/api/posts", Some(&me), Some(json!({"text":"мій пост"}))),
    )
    .await;

    let items = feed(&app, &me).await;
    let got: Vec<(String, String)> = items
        .iter()
        .map(|p| {
            (
                p["author_name"].as_str().unwrap().to_string(),
                p["reason"].as_str().unwrap().to_string(),
            )
        })
        .collect();

    assert_eq!(
        items.len(),
        4,
        "2 from follows + 2 by interest, own and jazz-stranger posts excluded: {got:?}"
    );
    assert!(got.contains(&("Followed Maker".into(), "following".into())), "{got:?}");
    assert!(got.contains(&("Rock Stranger".into(), "interest".into())), "{got:?}");
    assert!(
        got.contains(&("Jazz Stranger".into(), "interest".into())),
        "review on Rock event matches interest"
    );
    assert!(!got.iter().any(|(a, _)| a == "Me"), "own posts excluded from the feed");
    // Followed maker appears twice (event announcement + text post), both as 'following'.
    assert_eq!(
        got.iter()
            .filter(|(a, r)| a == "Followed Maker" && r == "following")
            .count(),
        2
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn following_wins_over_interest_and_empty_feed_for_blank_profile(pool: PgPool) {
    let app = make_app(pool);
    let (me, _) = register(&app, "me@x.co", "Me").await;
    let (maker, maker_id) = register(&app, "mk@x.co", "Maker").await;

    // Blank profile: no follows, no prefs → empty feed even though posts exist.
    create_event(&app, &maker, "Rock Gig", "Rock").await;
    assert!(feed(&app, &me).await.is_empty(), "no follows/prefs → empty feed");

    // Both follow AND interest match → single row, reason = following.
    send(
        &app,
        build("POST", &format!("/api/users/{maker_id}/follow"), Some(&me), None),
    )
    .await;
    send(
        &app,
        build(
            "PUT",
            "/api/me/prefs",
            Some(&me),
            Some(json!({"categories": ["Rock"], "city": null})),
        ),
    )
    .await;
    let items = feed(&app, &me).await;
    assert_eq!(items.len(), 1, "no duplicate when both sources match");
    assert_eq!(items[0]["reason"], "following", "follow relation wins as the reason");
    assert_eq!(items[0]["event_title"], "Rock Gig");
}

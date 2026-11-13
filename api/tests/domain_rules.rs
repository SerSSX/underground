//! Domain rules from the UX audit (TASK-014): past events, review timing,
//! single-event endpoint, unread counters.

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

async fn token_for(app: &Router, email: &str) -> String {
    let (_, v) = send(
        app,
        build(
            "POST",
            "/api/auth/register",
            None,
            Some(json!({"email":email,"password":"password1","name":"X"})),
        ),
    )
    .await;
    v["token"].as_str().unwrap().to_string()
}

/// starts_at in the past/future relative to now.
async fn create_event_at(app: &Router, token: &str, title: &str, starts_at: &str, cost: &str) -> String {
    let price = if cost == "free" { "0" } else { "100" };
    let (status, v) = send(
        app,
        build(
            "POST",
            "/api/events",
            Some(token),
            Some(json!({
                "title": title, "category": "Rock", "price": price, "tickets_total": 10,
                "cost_type": cost, "starts_at": starts_at
            })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
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

const PAST: &str = "2020-01-01T19:00:00Z";
const FUTURE: &str = "2030-01-01T19:00:00Z";

#[sqlx::test(migrations = "./migrations")]
async fn past_events_hidden_from_afisha_by_default(pool: PgPool) {
    let app = make_app(pool);
    let maker = token_for(&app, "m@x.co").await;
    create_event_at(&app, &maker, "Old gig", PAST, "free").await;
    create_event_at(&app, &maker, "New gig", FUTURE, "free").await;

    let (_, list) = send(&app, build("GET", "/api/events", None, None)).await;
    let titles: Vec<&str> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, vec!["New gig"], "past event hidden by default");

    let (_, list) = send(&app, build("GET", "/api/events?past=true", None, None)).await;
    assert_eq!(list.as_array().unwrap().len(), 2, "past=true includes history");
}

#[sqlx::test(migrations = "./migrations")]
async fn get_single_event(pool: PgPool) {
    let app = make_app(pool);
    let maker = token_for(&app, "m@x.co").await;
    let id = create_event_at(&app, &maker, "Solo", FUTURE, "free").await;

    let (status, event) = send(&app, build("GET", &format!("/api/events/{id}"), None, None)).await;
    assert_eq!(status, StatusCode::OK, "single event is public");
    assert_eq!(event["title"], "Solo");

    send(
        &app,
        build(
            "PUT",
            &format!("/api/events/{id}"),
            Some(&maker),
            Some(json!({"status":"hidden"})),
        ),
    )
    .await;
    let (status, _) = send(&app, build("GET", &format!("/api/events/{id}"), None, None)).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "hidden event → 404 for the public");
}

#[sqlx::test(migrations = "./migrations")]
async fn cannot_buy_or_reserve_past_event(pool: PgPool) {
    let app = make_app(pool);
    let maker = token_for(&app, "m@x.co").await;
    let fan = token_for(&app, "f@x.co").await;
    let free_past = create_event_at(&app, &maker, "Old free", PAST, "free").await;
    let paid_past = create_event_at(&app, &maker, "Old paid", PAST, "paid").await;

    let (status, body) = send(
        &app,
        build("POST", &format!("/api/events/{free_past}/reserve"), Some(&fan), None),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "reserve past → 409");
    assert_eq!(body["error"], "подія вже завершилась");

    let (status, _) = send(
        &app,
        build("POST", &format!("/api/events/{paid_past}/checkout"), Some(&fan), None),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "checkout past → 409");
}

#[sqlx::test(migrations = "./migrations")]
async fn reviews_open_only_after_event_starts(pool: PgPool) {
    let app = make_app(pool.clone());
    let maker = token_for(&app, "m@x.co").await;
    let fan = token_for(&app, "f@x.co").await;
    let future = create_event_at(&app, &maker, "Soon", FUTURE, "free").await;
    let started = create_event_at(&app, &maker, "Done", PAST, "free").await;
    give_ticket(&pool, &future, "f@x.co").await;
    give_ticket(&pool, &started, "f@x.co").await;

    let (status, body) = send(
        &app,
        build(
            "POST",
            "/api/reviews",
            Some(&fan),
            Some(json!({"event_id": future, "rating": 5})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "no reviews before the event");
    assert_eq!(body["error"], "відгуки відкриються після початку події");

    let (status, _) = send(
        &app,
        build(
            "POST",
            "/api/reviews",
            Some(&fan),
            Some(json!({"event_id": started, "rating": 5})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "reviews open once started");
}

#[sqlx::test(migrations = "./migrations")]
async fn unread_counters(pool: PgPool) {
    let app = make_app(pool);
    let (_, v) = send(
        &app,
        build(
            "POST",
            "/api/auth/register",
            None,
            Some(json!({"email":"fan@x.co","password":"password1","name":"Fan"})),
        ),
    )
    .await;
    let fan = v["token"].as_str().unwrap().to_string();
    let fan_id = v["user"]["id"].as_str().unwrap().to_string();
    let maker = token_for(&app, "m@x.co").await;

    let (status, _) = send(&app, build("GET", "/api/me/unread", None, None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "[SEC] unread without token → 401");

    // Notification: fan prefs Rock + maker creates a Rock event. Message: maker DMs fan.
    send(
        &app,
        build("PUT", "/api/me/prefs", Some(&fan), Some(json!({"categories":["Rock"]}))),
    )
    .await;
    create_event_at(&app, &maker, "Ping", FUTURE, "free").await;
    send(
        &app,
        build(
            "POST",
            "/api/messages",
            Some(&maker),
            Some(json!({"recipient_id": fan_id, "text": "привіт"})),
        ),
    )
    .await;

    let (_, counts) = send(&app, build("GET", "/api/me/unread", Some(&fan), None)).await;
    assert_eq!(counts["notifications"], 1);
    assert_eq!(counts["messages"], 1);

    // Reading both zeroes the counters.
    send(&app, build("POST", "/api/me/notifications/read", Some(&fan), None)).await;
    let maker_id = {
        let (_, profile) = send(&app, build("GET", "/api/me", Some(&maker), None)).await;
        profile["id"].as_str().unwrap().to_string()
    };
    send(
        &app,
        build("GET", &format!("/api/messages/{maker_id}"), Some(&fan), None),
    )
    .await;
    let (_, counts) = send(&app, build("GET", "/api/me/unread", Some(&fan), None)).await;
    assert_eq!(counts["notifications"], 0);
    assert_eq!(counts["messages"], 0);
}

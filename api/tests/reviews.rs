//! Integration tests for reviews (F-13): POST /api/reviews + GET /api/events/:id/reviews.

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
    let req = Request::post("/api/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"email": email, "password": "password1", "name": name}).to_string(),
        ))
        .unwrap();
    let (_, v) = send(app, req).await;
    (
        v["token"].as_str().unwrap().to_string(),
        v["user"]["id"].as_str().unwrap().to_string(),
    )
}

async fn create_event(app: &Router, token: &str) -> String {
    let req = Request::post("/api/events")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            json!({"title": "Gig", "category": "Rock", "price": "0", "tickets_total": 10}).to_string(),
        ))
        .unwrap();
    let (_, v) = send(app, req).await;
    v["id"].as_str().unwrap().to_string()
}

fn post_review(token: Option<&str>, body: Value) -> Request<Body> {
    let mut b = Request::post("/api/reviews").header("content-type", "application/json");
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    b.body(Body::from(body.to_string())).unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn review_requires_auth(pool: PgPool) {
    let app = make_app(pool);
    let (maker, _) = register(&app, "maker@x.co", "Maker").await;
    let event_id = create_event(&app, &maker).await;
    let (status, _) = send(&app, post_review(None, json!({"event_id": event_id, "rating": 5}))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "[SEC] review without token → 401");
}

#[sqlx::test(migrations = "./migrations")]
async fn review_validates_rating_and_event(pool: PgPool) {
    let app = make_app(pool);
    let (maker, _) = register(&app, "maker@x.co", "Maker").await;
    let event_id = create_event(&app, &maker).await;

    for bad in [0, 6, -1] {
        let (status, _) = send(
            &app,
            post_review(Some(&maker), json!({"event_id": event_id, "rating": bad})),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "rating {bad} rejected");
    }
    let ghost = "00000000-0000-0000-0000-000000000000";
    let (status, _) = send(&app, post_review(Some(&maker), json!({"event_id": ghost, "rating": 4}))).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unknown event → 404");
}

#[sqlx::test(migrations = "./migrations")]
async fn review_aggregate_and_upsert(pool: PgPool) {
    let app = make_app(pool);
    let (maker, maker_id) = register(&app, "maker@x.co", "Maker").await;
    let (alice, _) = register(&app, "alice@x.co", "Alice").await;
    let (bob, _) = register(&app, "bob@x.co", "Bob").await;
    let event_id = create_event(&app, &maker).await;

    let (status, body) = send(
        &app,
        post_review(
            Some(&alice),
            json!({"event_id": event_id, "rating": 5, "text": " Топ! "}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["text"], "Топ!", "review text trimmed");
    assert_eq!(body["author_name"], "Alice");

    send(
        &app,
        post_review(Some(&bob), json!({"event_id": event_id, "rating": 2})),
    )
    .await;

    let (status, body) = send(
        &app,
        Request::get(format!("/api/events/{event_id}/reviews"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "reviews list is public");
    assert_eq!(body["count"], 2);
    assert_eq!(body["avg_rating"], 3.5);
    assert_eq!(body["reviews"][0]["author_name"], "Bob", "newest first");

    // Re-review by the same author replaces, never duplicates.
    send(
        &app,
        post_review(Some(&bob), json!({"event_id": event_id, "rating": 4})),
    )
    .await;
    let (_, body) = send(
        &app,
        Request::get(format!("/api/events/{event_id}/reviews"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(body["count"], 2, "upsert does not duplicate");
    assert_eq!(body["avg_rating"], 4.5);

    // Organizer profile aggregates ratings over their events.
    let (_, profile) = send(
        &app,
        Request::get(format!("/api/users/{maker_id}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(profile["rating_count"], 2);
    assert_eq!(profile["rating_avg"], 4.5);
}

#[sqlx::test(migrations = "./migrations")]
async fn empty_reviews_aggregate(pool: PgPool) {
    let app = make_app(pool);
    let (maker, _) = register(&app, "maker@x.co", "Maker").await;
    let event_id = create_event(&app, &maker).await;
    let (status, body) = send(
        &app,
        Request::get(format!("/api/events/{event_id}/reviews"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["count"], 0);
    assert!(body["avg_rating"].is_null());
}

//! Integration tests for the organizer's mobile admin: edit/hide/cancel/stop-sales + media.

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

async fn create_event(app: &Router, token: &str, body: Value) -> Value {
    let (status, v) = send(app, build("POST", "/api/events", Some(token), Some(body))).await;
    assert_eq!(status, StatusCode::CREATED);
    v
}

fn base_event() -> Value {
    json!({"title":"Gig","category":"Rock","price":"100","tickets_total":10,"cost_type":"paid"})
}

#[sqlx::test(migrations = "./migrations")]
async fn create_with_media_round_trips(pool: PgPool) {
    let app = make_app(pool);
    let maker = token_for(&app, "m@x.co").await;
    let event = create_event(
        &app,
        &maker,
        json!({
            "title":"Media Gig","category":"Rock","price":"0","tickets_total":5,"cost_type":"free",
            "poster_url":"http://localhost:8080/uploads/p.jpg",
            "video_url":"https://youtube.com/watch?v=abc",
            "gallery_urls":["http://localhost:8080/uploads/g1.jpg","http://localhost:8080/uploads/g2.jpg"],
            "links":["https://instagram.com/gig","https://t.me/gig"]
        }),
    )
    .await;
    assert_eq!(event["video_url"], "https://youtube.com/watch?v=abc");
    assert_eq!(event["gallery_urls"].as_array().unwrap().len(), 2);
    assert_eq!(event["links"].as_array().unwrap().len(), 2);
    assert_eq!(event["sales_closed"], false);

    let (_, list) = send(&app, build("GET", "/api/events", None, None)).await;
    assert_eq!(list[0]["poster_url"], "http://localhost:8080/uploads/p.jpg");
}

#[sqlx::test(migrations = "./migrations")]
async fn update_owner_only_and_validates(pool: PgPool) {
    let app = make_app(pool);
    let maker = token_for(&app, "m@x.co").await;
    let other = token_for(&app, "o@x.co").await;
    let event = create_event(&app, &maker, base_event()).await;
    let id = event["id"].as_str().unwrap();

    let (status, _) = send(
        &app,
        build("PUT", &format!("/api/events/{id}"), None, Some(json!({"title":"X"}))),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "[SEC] edit without token → 401");

    let (status, _) = send(
        &app,
        build(
            "PUT",
            &format!("/api/events/{id}"),
            Some(&other),
            Some(json!({"title":"Hacked"})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "[SEC] not the owner → 403");

    let ghost = "00000000-0000-0000-0000-000000000000";
    let (status, _) = send(
        &app,
        build(
            "PUT",
            &format!("/api/events/{ghost}"),
            Some(&maker),
            Some(json!({"title":"X"})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    for bad in [
        json!({"title":"  "}),
        json!({"price":"-5"}),
        json!({"status":"vanished"}),
    ] {
        let (status, _) = send(
            &app,
            build("PUT", &format!("/api/events/{id}"), Some(&maker), Some(bad)),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    let (status, updated) = send(
        &app,
        build(
            "PUT",
            &format!("/api/events/{id}"),
            Some(&maker),
            Some(
                json!({"title":"New name","price":"150","video_url":"https://youtu.be/x","links":["https://fb.com/e"]}),
            ),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["title"], "New name");
    assert_eq!(updated["price"], "150.00");
    assert_eq!(updated["video_url"], "https://youtu.be/x");
    assert_eq!(updated["category"], "Rock", "untouched fields preserved");
}

#[sqlx::test(migrations = "./migrations")]
async fn hide_event_removes_from_afisha(pool: PgPool) {
    let app = make_app(pool);
    let maker = token_for(&app, "m@x.co").await;
    let event = create_event(&app, &maker, base_event()).await;
    let id = event["id"].as_str().unwrap();

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
    let (_, list) = send(&app, build("GET", "/api/events", None, None)).await;
    assert!(list.as_array().unwrap().is_empty(), "hidden event left the afisha");

    // The organizer still sees it among own events and can re-publish.
    let (_, mine) = send(&app, build("GET", "/api/me/events", Some(&maker), None)).await;
    assert_eq!(mine.as_array().unwrap().len(), 1);
    send(
        &app,
        build(
            "PUT",
            &format!("/api/events/{id}"),
            Some(&maker),
            Some(json!({"status":"published"})),
        ),
    )
    .await;
    let (_, list) = send(&app, build("GET", "/api/events", None, None)).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn stop_sales_blocks_reserve_and_checkout(pool: PgPool) {
    let app = make_app(pool);
    let maker = token_for(&app, "m@x.co").await;
    let fan = token_for(&app, "f@x.co").await;

    let paid = create_event(&app, &maker, base_event()).await;
    let paid_id = paid["id"].as_str().unwrap();
    let free = create_event(
        &app,
        &maker,
        json!({"title":"Free","category":"Poetry","price":"0","tickets_total":5,"cost_type":"free"}),
    )
    .await;
    let free_id = free["id"].as_str().unwrap();

    for id in [paid_id, free_id] {
        let (status, _) = send(
            &app,
            build(
                "PUT",
                &format!("/api/events/{id}"),
                Some(&maker),
                Some(json!({"sales_closed": true})),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    let (status, _) = send(
        &app,
        build("POST", &format!("/api/events/{free_id}/reserve"), Some(&fan), None),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "reserve blocked");
    let (status, _) = send(
        &app,
        build("POST", &format!("/api/events/{paid_id}/checkout"), Some(&fan), None),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "checkout blocked");

    // Resume sales → checkout works again.
    send(
        &app,
        build(
            "PUT",
            &format!("/api/events/{paid_id}"),
            Some(&maker),
            Some(json!({"sales_closed": false})),
        ),
    )
    .await;
    let (status, _) = send(
        &app,
        build("POST", &format!("/api/events/{paid_id}/checkout"), Some(&fan), None),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
}

#[sqlx::test(migrations = "./migrations")]
async fn cancel_event(pool: PgPool) {
    let app = make_app(pool);
    let maker = token_for(&app, "m@x.co").await;
    let other = token_for(&app, "o@x.co").await;
    let event = create_event(&app, &maker, base_event()).await;
    let id = event["id"].as_str().unwrap();

    let (status, _) = send(&app, build("DELETE", &format!("/api/events/{id}"), Some(&other), None)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "[SEC] not the owner → 403");

    let (status, _) = send(&app, build("DELETE", &format!("/api/events/{id}"), Some(&maker), None)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (_, list) = send(&app, build("GET", "/api/events", None, None)).await;
    assert!(list.as_array().unwrap().is_empty(), "cancelled event left the afisha");
    let (_, mine) = send(&app, build("GET", "/api/me/events", Some(&maker), None)).await;
    assert_eq!(mine[0]["status"], "cancelled", "kept in history for the organizer");
}

//! Event reminders (TASK-017 M4): the sweep reminds going + interested users of
//! events starting within the window, exactly once, and skips events outside it.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use chrono::{Duration, Utc};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use underground_api::reminders;
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

async fn register(app: &Router, email: &str) -> String {
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
    v["token"].as_str().unwrap().to_string()
}

async fn create_event_at(app: &Router, token: &str, title: &str, starts_at: &str) -> String {
    let (status, v) = send(
        app,
        build(
            "POST",
            "/api/events",
            Some(token),
            Some(json!({"title":title,"category":"Rock","city":"Kyiv","price":"0","tickets_total":10,"cost_type":"free","starts_at":starts_at})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    v["id"].as_str().unwrap().to_string()
}

async fn notifs(app: &Router, token: &str) -> Vec<Value> {
    let (_, v) = send(app, build("GET", "/api/me/notifications", Some(token), None)).await;
    v.as_array().unwrap().clone()
}

#[sqlx::test(migrations = "./migrations")]
async fn sweep_reminds_going_and_interested_once(pool: PgPool) {
    let app = make_app(pool.clone());
    let maker = register(&app, "m@x.co").await;
    let holder = register(&app, "h@x.co").await;
    let saver = register(&app, "s@x.co").await;

    let soon = (Utc::now() + Duration::hours(2)).to_rfc3339();
    let far = (Utc::now() + Duration::hours(48)).to_rfc3339();
    let soon_ev = create_event_at(&app, &maker, "Soon", &soon).await;
    let far_ev = create_event_at(&app, &maker, "Far", &far).await;

    // holder is "going" (free ticket) to both; saver is "interested" in the soon event.
    send(
        &app,
        build("POST", &format!("/api/events/{soon_ev}/reserve"), Some(&holder), None),
    )
    .await;
    send(
        &app,
        build("POST", &format!("/api/events/{far_ev}/reserve"), Some(&holder), None),
    )
    .await;
    send(
        &app,
        build("POST", &format!("/api/events/{soon_ev}/save"), Some(&saver), None),
    )
    .await;
    // The saver prefers English → their reminder is localized (TASK-017 M5).
    send(
        &app,
        build(
            "PUT",
            "/api/me/prefs",
            Some(&saver),
            Some(json!({"categories": [], "locale": "en"})),
        ),
    )
    .await;

    // Only the soon event (within 24h) reminds its 2 users.
    let n = reminders::sweep(&pool, 24).await.unwrap();
    assert_eq!(n, 2, "holder + saver of the soon event");
    // Idempotent: a second sweep emits nothing (dedup ledger).
    assert_eq!(reminders::sweep(&pool, 24).await.unwrap(), 0);

    for tok in [&holder, &saver] {
        let list = notifs(&app, tok).await;
        assert!(
            list.iter()
                .any(|x| x["kind"] == "event_reminder" && x["event_id"] == soon_ev),
            "reminded about the soon event"
        );
    }
    // The reminder body is localized per recipient locale.
    let saver_rem = notifs(&app, &saver).await;
    let sr = saver_rem.iter().find(|x| x["kind"] == "event_reminder").unwrap();
    assert_eq!(sr["body"], "Event starts soon", "saver locale=en → English body");
    let holder_rem = notifs(&app, &holder).await;
    let hr = holder_rem.iter().find(|x| x["kind"] == "event_reminder").unwrap();
    assert_eq!(
        hr["body"], "Подія скоро розпочнеться",
        "holder default uk → Ukrainian body"
    );
    // The far event (>24h) never reminds anyone.
    let hl = notifs(&app, &holder).await;
    assert!(!hl
        .iter()
        .any(|x| x["kind"] == "event_reminder" && x["event_id"] == far_ev));
}

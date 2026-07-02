//! Integration tests for the transactional-email outbox (P1 #75).

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use underground_api::email;
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
            Some(json!({"email":email,"password":"password1","name":"U"})),
        ),
    )
    .await;
    v["token"].as_str().unwrap().to_string()
}

#[sqlx::test(migrations = "./migrations")]
async fn outbox_picks_up_paid_orders_once(pool: PgPool) {
    let app = make_app(pool.clone());
    let maker = register(&app, "m@x.co").await;
    let buyer = register(&app, "b@x.co").await;
    let (_, ev) = send(
        &app,
        build(
            "POST",
            "/api/events",
            Some(&maker),
            Some(json!({"title":"Paid Gig","category":"Rock","price":"100","tickets_total":5,"cost_type":"paid"})),
        ),
    )
    .await;
    let event = ev["id"].as_str().unwrap().to_string();

    // Nothing pending before any sale.
    assert!(email::pending(&pool, 10).await.unwrap().is_empty());

    // Buy + pay → a paid order appears in the outbox.
    let (_, c) = send(
        &app,
        build("POST", &format!("/api/events/{event}/checkout"), Some(&buyer), None),
    )
    .await;
    let order_id = c["order_id"].as_str().unwrap().to_string();
    let (st, _) = send(
        &app,
        build("POST", &format!("/api/orders/{order_id}/pay-test"), Some(&buyer), None),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);

    let batch = email::pending(&pool, 10).await.unwrap();
    assert_eq!(batch.len(), 1);
    assert_eq!(batch[0].to_email, "b@x.co");
    assert_eq!(batch[0].event_title, "Paid Gig");
    let (subject, _) = email::build_confirmation(&batch[0]);
    assert!(subject.contains("Paid Gig"));

    // Watermark → no longer pending (idempotent re-send guard).
    let ids: Vec<_> = batch.iter().map(|p| p.order_id).collect();
    email::mark_sent(&pool, &ids).await.unwrap();
    assert!(email::pending(&pool, 10).await.unwrap().is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn notification_outbox_emails_retention_kinds_once(pool: PgPool) {
    let app = make_app(pool.clone());
    let _u = register(&app, "u@x.co").await;

    // Two retention/conversion notifications (emailed) + a noisy feed notification (never emailed).
    sqlx::query("INSERT INTO notifications (user_id, kind, title, body) SELECT id, 'review_prompt', 'Gig', 'Як враження?' FROM users WHERE email = 'u@x.co'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO notifications (user_id, kind, title, body) SELECT id, 'selling_fast', 'Almost Gone', 'Квитки закінчуються' FROM users WHERE email = 'u@x.co'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO notifications (user_id, kind, title, body) SELECT id, 'event_match', 'Match', 'b' FROM users WHERE email = 'u@x.co'")
        .execute(&pool)
        .await
        .unwrap();

    let batch = email::pending_notifications(&pool, 10).await.unwrap();
    assert_eq!(
        batch.len(),
        2,
        "both retention/conversion kinds are emailed; the noisy feed kind is not"
    );
    let kinds: Vec<&str> = batch.iter().map(|p| p.kind.as_str()).collect();
    assert!(kinds.contains(&"review_prompt") && kinds.contains(&"selling_fast"));
    assert!(
        !kinds.contains(&"event_match"),
        "noisy feed notifications are never emailed"
    );
    assert!(batch.iter().all(|p| p.to_email == "u@x.co"));
    assert!(!email::build_notification_email(&batch[0]).0.is_empty());

    // [SEC] a blocked user's retention notification is never emailed.
    let _b = register(&app, "x@x.co").await;
    sqlx::query("UPDATE users SET blocked = true WHERE email = 'x@x.co'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO notifications (user_id, kind, title, body) SELECT id, 'event_reminder', 'B', 'b' FROM users WHERE email = 'x@x.co'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        email::pending_notifications(&pool, 10)
            .await
            .unwrap()
            .iter()
            .all(|p| p.to_email != "x@x.co"),
        "[SEC] blocked users are not emailed"
    );

    // Watermark → no longer pending (idempotent).
    let ids: Vec<_> = batch.iter().map(|p| p.notification_id).collect();
    email::mark_notifications_sent(&pool, &ids).await.unwrap();
    assert!(
        email::pending_notifications(&pool, 10)
            .await
            .unwrap()
            .iter()
            .all(|p| p.kind != "review_prompt"),
        "an emailed notification is not re-selected"
    );
}

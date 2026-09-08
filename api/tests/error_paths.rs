//! Error-path coverage: rare branches (sold-out conflicts, mode guards,
//! validation limits, degraded-DB fallbacks) the happy-path suites never hit.

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

async fn create_event(app: &Router, token: &str, body: Value) -> String {
    let (status, v) = send(app, build("POST", "/api/events", Some(token), Some(body))).await;
    assert_eq!(status, StatusCode::CREATED);
    v["id"].as_str().unwrap().to_string()
}

#[sqlx::test(migrations = "./migrations")]
async fn register_maps_unexpected_db_error_to_500(pool: PgPool) {
    let app = make_app(pool.clone());
    // Degrade the schema so user insertion fails with a non-unique-violation DB error.
    sqlx::query("ALTER TABLE users DROP COLUMN password_hash")
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = send(
        &app,
        build(
            "POST",
            "/api/auth/register",
            None,
            Some(json!({"email":"a@x.co","password":"password1","name":"A"})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["error"], "database error", "[SEC] no internal details leaked");
}

#[sqlx::test(migrations = "./migrations")]
async fn create_event_rejects_blank_category_and_negative_price(pool: PgPool) {
    let app = make_app(pool);
    let token = token_for(&app, "m@x.co").await;

    let (status, _) = send(
        &app,
        build(
            "POST",
            "/api/events",
            Some(&token),
            Some(json!({"title":"X","category":"  ","price":"0","tickets_total":1})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "blank category rejected");

    let (status, _) = send(
        &app,
        build(
            "POST",
            "/api/events",
            Some(&token),
            Some(json!({"title":"X","category":"Rock","price":"-1","tickets_total":1})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "negative price rejected");
}

#[sqlx::test(migrations = "./migrations")]
async fn event_creation_survives_notification_fanout_failure(pool: PgPool) {
    let app = make_app(pool.clone());
    let token = token_for(&app, "m@x.co").await;
    sqlx::query("DROP TABLE notifications").execute(&pool).await.unwrap();
    let (status, _) = send(
        &app,
        build(
            "POST",
            "/api/events",
            Some(&token),
            Some(json!({"title":"Resilient","category":"Rock","price":"0","tickets_total":1})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "fan-out failure must not fail creation");
}

#[sqlx::test(migrations = "./migrations")]
async fn event_and_review_survive_wall_post_failure(pool: PgPool) {
    let app = make_app(pool.clone());
    let maker = token_for(&app, "m@x.co").await;
    let fan = token_for(&app, "f@x.co").await;
    let event = create_event(
        &app,
        &maker,
        json!({"title":"Gig","category":"Rock","price":"0","tickets_total":5,"cost_type":"free"}),
    )
    .await;

    sqlx::query("DROP TABLE posts CASCADE").execute(&pool).await.unwrap();

    let (status, _) = send(
        &app,
        build(
            "POST",
            "/api/events",
            Some(&maker),
            Some(json!({"title":"No wall","category":"Rock","price":"0","tickets_total":1})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "event creation survives wall failure");

    let (status, _) = send(
        &app,
        build(
            "POST",
            "/api/reviews",
            Some(&fan),
            Some(json!({"event_id": event, "rating": 4, "text": "ok"})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "review survives wall failure");
}

#[sqlx::test(migrations = "./migrations")]
async fn checkout_conflicts_when_sold_out(pool: PgPool) {
    let app = make_app(pool);
    let token = token_for(&app, "b@x.co").await;
    let event = create_event(
        &app,
        &token,
        json!({"title":"Sold out","category":"Rock","price":"100","tickets_total":0,"cost_type":"paid"}),
    )
    .await;
    let (status, _) = send(
        &app,
        build("POST", &format!("/api/events/{event}/checkout"), Some(&token), None),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[sqlx::test(migrations = "./migrations")]
async fn pay_test_rejected_outside_test_mode(pool: PgPool) {
    let app = make_app(pool.clone());
    let token = token_for(&app, "b@x.co").await;
    let event = create_event(
        &app,
        &token,
        json!({"title":"Live gig","category":"Rock","price":"100","tickets_total":2,"cost_type":"paid"}),
    )
    .await;
    let (_, checkout) = send(
        &app,
        build("POST", &format!("/api/events/{event}/checkout"), Some(&token), None),
    )
    .await;
    let order = checkout["order_id"].as_str().unwrap();

    sqlx::query("UPDATE payment_settings SET mode = 'live'")
        .execute(&pool)
        .await
        .unwrap();
    let (status, _) = send(
        &app,
        build("POST", &format!("/api/orders/{order}/pay-test"), Some(&token), None),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "test payment is test-mode only");
}

/// This test used to be `pay_test_conflicts_when_tickets_ran_out`: it drove `tickets_left` to 0
/// between checkout and payment and expected pay-test to answer 409. That scenario WAS the
/// worst money bug in the repo — a buyer charged for a seat that "vanished" — and PAY-3 removed
/// it by reserving the seat at checkout. The seat cannot vanish any more, so a payment on a
/// pending order always yields the ticket, and the stock never goes negative. The genuine
/// refusal case (paying an order whose hold has lapsed) lives in
/// `orders_hold::sec_pay_test_on_an_expired_order_is_refused`.
#[sqlx::test(migrations = "./migrations")]
async fn pay_test_succeeds_because_the_seat_was_reserved_at_checkout(pool: PgPool) {
    let app = make_app(pool.clone());
    let token = token_for(&app, "b@x.co").await;
    let event = create_event(
        &app,
        &token,
        json!({"title":"Last one","category":"Rock","price":"100","tickets_total":1,"cost_type":"paid"}),
    )
    .await;
    let (_, checkout) = send(
        &app,
        build("POST", &format!("/api/events/{event}/checkout"), Some(&token), None),
    )
    .await;
    let order = checkout["order_id"].as_str().unwrap();
    let left: i32 = sqlx::query_scalar("SELECT tickets_left FROM events WHERE id = $1::uuid")
        .bind(event.as_str())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(left, 0, "the last seat is held by the checkout, not still on sale");

    // Nothing external can take the held seat away; the payment lands on it.
    let (status, ticket) = send(
        &app,
        build("POST", &format!("/api/orders/{order}/pay-test"), Some(&token), None),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "paid → ticket, always: {ticket}");
    let left: i32 = sqlx::query_scalar("SELECT tickets_left FROM events WHERE id = $1::uuid")
        .bind(event.as_str())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(left, 0, "no second decrement at payment — the seat was already ours");
}

#[sqlx::test(migrations = "./migrations")]
async fn settings_validate_provider(pool: PgPool) {
    let app = make_app(pool.clone());
    let admin = token_for(&app, "admin@x.co").await;
    sqlx::query("UPDATE users SET role = 'ADMIN' WHERE email = 'admin@x.co'")
        .execute(&pool)
        .await
        .unwrap();

    let (status, _) = send(
        &app,
        build(
            "PUT",
            "/api/admin/payment-settings",
            Some(&admin),
            Some(json!({"provider":"bogus"})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // Switching provider while staying in test mode is fine.
    let (status, v) = send(
        &app,
        build(
            "PUT",
            "/api/admin/payment-settings",
            Some(&admin),
            Some(json!({"provider":"lemonsqueezy"})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["provider"], "lemonsqueezy");
    assert_eq!(v["mode"], "test");

    // But flipping to live without the Lemon Squeezy credentials is rejected (incomplete config).
    let (status, _) = send(
        &app,
        build(
            "PUT",
            "/api/admin/payment-settings",
            Some(&admin),
            Some(json!({"mode":"live"})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "live requires LS creds");
}

#[sqlx::test(migrations = "./migrations")]
async fn reserve_conflicts_when_free_event_sold_out(pool: PgPool) {
    let app = make_app(pool);
    let token = token_for(&app, "u@x.co").await;
    let event = create_event(
        &app,
        &token,
        json!({"title":"Gone","category":"Poetry","price":"0","tickets_total":0,"cost_type":"free"}),
    )
    .await;
    let (status, _) = send(
        &app,
        build("POST", &format!("/api/events/{event}/reserve"), Some(&token), None),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[sqlx::test(migrations = "./migrations")]
async fn validate_returned_ticket_is_inactive(pool: PgPool) {
    let app = make_app(pool);
    let org = token_for(&app, "org@x.co").await;
    let event = create_event(
        &app,
        &org,
        json!({"title":"Free","category":"Poetry","price":"0","tickets_total":3,"cost_type":"free"}),
    )
    .await;
    let (_, ticket) = send(
        &app,
        build("POST", &format!("/api/events/{event}/reserve"), Some(&org), None),
    )
    .await;
    let ticket_id = ticket["id"].as_str().unwrap().to_string();
    // Mint the signed door-QR while the ticket is still active (BR-023), then return it.
    let (_, qr_resp) = send(
        &app,
        build("GET", &format!("/api/tickets/{ticket_id}/qr"), Some(&org), None),
    )
    .await;
    let qr = qr_resp["token"].as_str().unwrap().to_string();

    let (status, _) = send(
        &app,
        build("POST", &format!("/api/tickets/{ticket_id}/return"), Some(&org), None),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, v) = send(
        &app,
        build(
            "POST",
            "/api/tickets/validate",
            Some(&org),
            Some(json!({"qr_token": qr})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["valid"], false);
    assert_eq!(v["reason"], "inactive", "returned ticket cannot enter");
}

#[sqlx::test(migrations = "./migrations")]
async fn review_text_too_long_rejected(pool: PgPool) {
    let app = make_app(pool);
    let token = token_for(&app, "u@x.co").await;
    let event = create_event(
        &app,
        &token,
        json!({"title":"Gig","category":"Rock","price":"0","tickets_total":1}),
    )
    .await;
    let long = "x".repeat(2001);
    let (status, _) = send(
        &app,
        build(
            "POST",
            "/api/reviews",
            Some(&token),
            Some(json!({"event_id": event, "rating": 5, "text": long})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

//! PAY-3 — the seat is reserved at checkout and payment can no longer fail after the money.
//!
//! Every test here maps to an AC or SEC row in
//! docs/backlog/2026-09-06-monetisation/PAY-3-*.md. The ones marked [SEC] are the abuse
//! cases: they assert what must be REFUSED, and several are control-presence tests — remove
//! the guard they name and they go red.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use underground_api::orders::{self, FulfillOutcome};
use underground_api::routes::router;
use underground_api::state::AppState;
use underground_api::webhooks;
use uuid::Uuid;

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

async fn json_body(resp: axum::response::Response) -> Value {
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn token_for(app: &Router, email: &str) -> String {
    let resp = app
        .clone()
        .oneshot(build(
            "POST",
            "/api/auth/register",
            None,
            Some(json!({"email":email,"password":"password1","name":"X"})),
        ))
        .await
        .unwrap();
    json_body(resp).await["token"].as_str().unwrap().to_string()
}

async fn create_event(app: &Router, token: &str, tickets_total: i32) -> Uuid {
    let body = json!({"title":"Hold Gig","category":"Rock","price":"100","currency":"UAH",
        "tickets_total":tickets_total,"cost_type":"paid"});
    let resp = app
        .clone()
        .oneshot(build("POST", "/api/events", Some(token), Some(body)))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    Uuid::parse_str(json_body(resp).await["id"].as_str().unwrap()).unwrap()
}

async fn checkout(app: &Router, token: &str, event: Uuid, promo: Option<&str>) -> (StatusCode, Value) {
    let path = match promo {
        Some(p) => format!("/api/events/{event}/checkout?promo={p}"),
        None => format!("/api/events/{event}/checkout"),
    };
    let resp = app
        .clone()
        .oneshot(build("POST", &path, Some(token), None))
        .await
        .unwrap();
    let status = resp.status();
    (status, json_body(resp).await)
}

async fn order_id(v: &Value) -> Uuid {
    Uuid::parse_str(v["order_id"].as_str().expect("order_id in checkout response")).unwrap()
}

async fn tickets_left(pool: &PgPool, event: Uuid) -> i32 {
    sqlx::query_scalar("SELECT tickets_left FROM events WHERE id = $1")
        .bind(event)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn order_status(pool: &PgPool, order: Uuid) -> (String, String) {
    sqlx::query_as("SELECT status, escrow_status FROM orders WHERE id = $1")
        .bind(order)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Push the hold into the past so the expiry sweep sees it. Time control without sleeping.
async fn lapse(pool: &PgPool, order: Uuid) {
    sqlx::query("UPDATE orders SET hold_expires_at = now() - interval '1 minute' WHERE id = $1")
        .bind(order)
        .execute(pool)
        .await
        .unwrap();
}

async fn fulfill_now(pool: &PgPool, order: Uuid) -> FulfillOutcome {
    let mut tx = pool.begin().await.unwrap();
    let out = orders::fulfill(&mut tx, order).await.unwrap();
    tx.commit().await.unwrap();
    out
}

async fn refund_jobs_for(pool: &PgPool, order: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM refund_jobs WHERE order_id = $1")
        .bind(order)
        .fetch_one(pool)
        .await
        .unwrap()
}

// ---------------------------------------------------------------- AC

#[sqlx::test(migrations = "./migrations")]
async fn checkout_reserves_seat_and_the_second_buyer_is_refused(pool: PgPool) {
    let app = make_app(pool.clone());
    let owner = token_for(&app, "owner@x.co").await;
    let a = token_for(&app, "a@x.co").await;
    let b = token_for(&app, "b@x.co").await;
    let event = create_event(&app, &owner, 1).await;

    // Genuinely concurrent: both requests are in flight at once. Exactly one may win.
    let (ra, rb) = tokio::join!(checkout(&app, &a, event, None), checkout(&app, &b, event, None));
    let statuses = [ra.0, rb.0];
    assert_eq!(
        statuses.iter().filter(|s| **s == StatusCode::CREATED).count(),
        1,
        "exactly one 201, got {statuses:?}"
    );
    assert_eq!(statuses.iter().filter(|s| **s == StatusCode::CONFLICT).count(), 1);
    assert_eq!(tickets_left(&pool, event).await, 0, "never negative, never double-sold");

    let winner = if ra.0 == StatusCode::CREATED { ra.1 } else { rb.1 };
    assert!(
        winner["hold_expires_at"].as_str().is_some(),
        "checkout tells the client until when the seat is held"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn lapsed_hold_is_expired_and_its_seat_returns_to_sale(pool: PgPool) {
    let app = make_app(pool.clone());
    let owner = token_for(&app, "owner@x.co").await;
    let a = token_for(&app, "a@x.co").await;
    let event = create_event(&app, &owner, 1).await;
    let (s, v) = checkout(&app, &a, event, None).await;
    assert_eq!(s, StatusCode::CREATED);
    let order = order_id(&v).await;
    assert_eq!(tickets_left(&pool, event).await, 0);

    lapse(&pool, order).await;
    orders::expire_holds(&pool).await.unwrap();

    assert_eq!(order_status(&pool, order).await.0, "expired");
    assert_eq!(tickets_left(&pool, event).await, 1, "seat came back exactly once");

    // The sweep is idempotent: a second pass finds nothing and adds nothing.
    orders::expire_holds(&pool).await.unwrap();
    assert_eq!(tickets_left(&pool, event).await, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn late_payment_on_expired_order_mints_when_a_seat_is_free(pool: PgPool) {
    let app = make_app(pool.clone());
    let owner = token_for(&app, "owner@x.co").await;
    let a = token_for(&app, "a@x.co").await;
    let event = create_event(&app, &owner, 1).await;
    let (_, v) = checkout(&app, &a, event, None).await;
    let order = order_id(&v).await;
    lapse(&pool, order).await;
    orders::expire_holds(&pool).await.unwrap();
    assert_eq!(tickets_left(&pool, event).await, 1);

    // The buyer finished paying after the hold lapsed; capacity still allows it.
    let out = fulfill_now(&pool, order).await;
    assert!(matches!(out, FulfillOutcome::Fulfilled(_)), "got {out:?}");
    assert_eq!(order_status(&pool, order).await, ("paid".into(), "held".into()));
    assert_eq!(tickets_left(&pool, event).await, 0);
    assert_eq!(refund_jobs_for(&pool, order).await, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn late_payment_with_no_seat_left_queues_a_refund_instead_of_failing(pool: PgPool) {
    let app = make_app(pool.clone());
    let owner = token_for(&app, "owner@x.co").await;
    let a = token_for(&app, "a@x.co").await;
    let b = token_for(&app, "b@x.co").await;
    let event = create_event(&app, &owner, 1).await;
    let (_, va) = checkout(&app, &a, event, None).await;
    let order_a = order_id(&va).await;
    lapse(&pool, order_a).await;
    orders::expire_holds(&pool).await.unwrap();
    // B takes the seat A let lapse.
    let (sb, _) = checkout(&app, &b, event, None).await;
    assert_eq!(sb, StatusCode::CREATED);
    assert_eq!(tickets_left(&pool, event).await, 0);

    // A's money arrives anyway. The old code returned 409 here — money taken, no ticket.
    let out = fulfill_now(&pool, order_a).await;
    assert_eq!(out, FulfillOutcome::RefundDue);
    assert_eq!(order_status(&pool, order_a).await, ("paid".into(), "refund_due".into()));
    assert_eq!(refund_jobs_for(&pool, order_a).await, 1, "exactly one refund per order");
    let minted: i64 = sqlx::query_scalar("SELECT count(*) FROM tickets WHERE event_id = $1")
        .bind(event)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(minted, 0, "no ticket for A");
    assert_eq!(tickets_left(&pool, event).await, 0, "B's seat untouched");

    // A replayed delivery does not create a second refund.
    assert_eq!(fulfill_now(&pool, order_a).await, FulfillOutcome::AlreadyHandled);
    assert_eq!(refund_jobs_for(&pool, order_a).await, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn webhook_paid_answers_200_when_sold_out_and_queues_the_refund(pool: PgPool) {
    let app = make_app(pool.clone());
    let owner = token_for(&app, "owner@x.co").await;
    let a = token_for(&app, "a@x.co").await;
    let b = token_for(&app, "b@x.co").await;
    let admin = token_for(&app, "admin@x.co").await;
    sqlx::query("UPDATE users SET role = 'ADMIN' WHERE email = 'admin@x.co'")
        .execute(&pool)
        .await
        .unwrap();
    let resp = app
        .clone()
        .oneshot(build(
            "PUT",
            "/api/admin/payment-settings",
            Some(&admin),
            Some(json!({ "ls_webhook_secret": "whsec_test" })),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let event = create_event(&app, &owner, 1).await;
    let (_, va) = checkout(&app, &a, event, None).await;
    let order_a = order_id(&va).await;
    lapse(&pool, order_a).await;
    orders::expire_holds(&pool).await.unwrap();
    assert_eq!(checkout(&app, &b, event, None).await.0, StatusCode::CREATED);

    // The provider reports A paid, through the real route with a real signature.
    let raw = json!({"meta":{"event_name":"order_paid","custom_data":{"order_id":order_a.to_string()}}}).to_string();
    let req = Request::builder()
        .method("POST")
        .uri("/api/webhooks/lemonsqueezy")
        .header("content-type", "application/json")
        .header("x-signature", webhooks::sign("whsec_test", raw.as_bytes()))
        .body(Body::from(raw))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "a provider must never be told to retry a state we own"
    );
    assert_eq!(order_status(&pool, order_a).await.1, "refund_due");
    assert_eq!(refund_jobs_for(&pool, order_a).await, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn promo_exhausted_at_payment_refunds_and_gives_the_seat_back(pool: PgPool) {
    let app = make_app(pool.clone());
    let owner = token_for(&app, "owner@x.co").await;
    let a = token_for(&app, "a@x.co").await;
    let b = token_for(&app, "b@x.co").await;
    let event = create_event(&app, &owner, 5).await;
    sqlx::query("INSERT INTO promo_codes (event_id, code, percent_off, max_uses) VALUES ($1, 'LAST1', 50, 1)")
        .bind(event)
        .execute(&pool)
        .await
        .unwrap();
    // Both checkouts see an unused code; only one payment can claim it.
    let (sa, va) = checkout(&app, &a, event, Some("LAST1")).await;
    let (sb, vb) = checkout(&app, &b, event, Some("LAST1")).await;
    assert_eq!((sa, sb), (StatusCode::CREATED, StatusCode::CREATED));
    assert_eq!(tickets_left(&pool, event).await, 3);

    assert!(matches!(
        fulfill_now(&pool, order_id(&va).await).await,
        FulfillOutcome::Fulfilled(_)
    ));
    let order_b = order_id(&vb).await;
    assert_eq!(fulfill_now(&pool, order_b).await, FulfillOutcome::RefundDue);
    assert_eq!(order_status(&pool, order_b).await.1, "refund_due");
    assert_eq!(refund_jobs_for(&pool, order_b).await, 1);
    assert_eq!(
        tickets_left(&pool, event).await,
        4,
        "B's reserved seat returned to sale: 5 total, 1 sold to A"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn returning_a_ticket_never_lifts_capacity_above_total(pool: PgPool) {
    let app = make_app(pool.clone());
    let owner = token_for(&app, "owner@x.co").await;
    let a = token_for(&app, "a@x.co").await;
    let event = create_event(&app, &owner, 2).await;
    let (_, v) = checkout(&app, &a, event, None).await;
    let order = order_id(&v).await;
    let resp = app
        .clone()
        .oneshot(build("POST", &format!("/api/orders/{order}/pay-test"), Some(&a), None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let ticket = json_body(resp).await["id"].as_str().unwrap().to_string();
    assert_eq!(tickets_left(&pool, event).await, 1);
    // The organiser shrinks the event after the sale.
    sqlx::query("UPDATE events SET tickets_total = 1 WHERE id = $1")
        .bind(event)
        .execute(&pool)
        .await
        .unwrap();

    let resp = app
        .clone()
        .oneshot(build("POST", &format!("/api/tickets/{ticket}/return"), Some(&a), None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "the return itself goes through");
    assert_eq!(tickets_left(&pool, event).await, 1, "clamped at the new total, not 2");
}

#[sqlx::test(migrations = "./migrations")]
async fn get_order_shows_the_hold_and_is_buyer_scoped(pool: PgPool) {
    let app = make_app(pool.clone());
    let owner = token_for(&app, "owner@x.co").await;
    let a = token_for(&app, "a@x.co").await;
    let b = token_for(&app, "b@x.co").await;
    let event = create_event(&app, &owner, 2).await;
    let (_, v) = checkout(&app, &a, event, None).await;
    let order = order_id(&v).await;

    let resp = app
        .clone()
        .oneshot(build("GET", &format!("/api/orders/{order}"), Some(&a), None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["status"], "pending");
    assert!(body["hold_expires_at"].as_str().is_some());

    // [SEC] someone else's order does not exist, as far as the API is concerned.
    let resp = app
        .clone()
        .oneshot(build("GET", &format!("/api/orders/{order}"), Some(&b), None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let resp = app
        .clone()
        .oneshot(build("GET", &format!("/api/orders/{order}"), None, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------- [SEC]

#[sqlx::test(migrations = "./migrations")]
async fn sec_same_buyer_rechecking_out_never_holds_two_seats(pool: PgPool) {
    let app = make_app(pool.clone());
    let owner = token_for(&app, "owner@x.co").await;
    let a = token_for(&app, "a@x.co").await;
    let event = create_event(&app, &owner, 2).await;
    let (_, v1) = checkout(&app, &a, event, None).await;
    let first = order_id(&v1).await;
    assert_eq!(tickets_left(&pool, event).await, 1);

    let (s2, v2) = checkout(&app, &a, event, None).await;
    assert_eq!(s2, StatusCode::CREATED);
    let second = order_id(&v2).await;
    assert_ne!(first, second);
    assert_eq!(tickets_left(&pool, event).await, 1, "one seat for one buyer, not two");
    assert_eq!(
        order_status(&pool, first).await.0,
        "expired",
        "the earlier hold was released"
    );
    assert_eq!(order_status(&pool, second).await.0, "pending");
}

#[sqlx::test(migrations = "./migrations")]
async fn sec_fifty_abandoned_checkouts_fully_restore_after_expiry(pool: PgPool) {
    let app = make_app(pool.clone());
    let owner = token_for(&app, "owner@x.co").await;
    let event = create_event(&app, &owner, 60).await;
    let mut orders_made = Vec::new();
    for i in 0..50 {
        let t = token_for(&app, &format!("bot{i}@x.co")).await;
        let (s, v) = checkout(&app, &t, event, None).await;
        assert_eq!(s, StatusCode::CREATED);
        orders_made.push(order_id(&v).await);
    }
    assert_eq!(
        tickets_left(&pool, event).await,
        10,
        "mass checkout does drain the visible stock…"
    );
    for o in &orders_made {
        lapse(&pool, *o).await;
    }
    orders::expire_holds(&pool).await.unwrap();
    assert_eq!(tickets_left(&pool, event).await, 60, "…but not for longer than a hold");
}

#[sqlx::test(migrations = "./migrations")]
async fn sec_pay_test_on_an_expired_order_is_refused(pool: PgPool) {
    let app = make_app(pool.clone());
    let owner = token_for(&app, "owner@x.co").await;
    let a = token_for(&app, "a@x.co").await;
    let event = create_event(&app, &owner, 1).await;
    let (_, v) = checkout(&app, &a, event, None).await;
    let order = order_id(&v).await;
    lapse(&pool, order).await;
    orders::expire_holds(&pool).await.unwrap();

    let resp = app
        .clone()
        .oneshot(build("POST", &format!("/api/orders/{order}/pay-test"), Some(&a), None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    assert_eq!(
        order_status(&pool, order).await.0,
        "expired",
        "still expired, nothing minted"
    );
    assert_eq!(tickets_left(&pool, event).await, 1);
}

/// Control-presence: the sweep's `status = 'pending'` filter. Remove it and this goes red.
#[sqlx::test(migrations = "./migrations")]
async fn sec_expiry_sweep_leaves_paid_orders_alone(pool: PgPool) {
    let app = make_app(pool.clone());
    let owner = token_for(&app, "owner@x.co").await;
    let a = token_for(&app, "a@x.co").await;
    let event = create_event(&app, &owner, 1).await;
    let (_, v) = checkout(&app, &a, event, None).await;
    let order = order_id(&v).await;
    let resp = app
        .clone()
        .oneshot(build("POST", &format!("/api/orders/{order}/pay-test"), Some(&a), None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // A paid order with a hold time in the past must not be touched.
    lapse(&pool, order).await;
    orders::expire_holds(&pool).await.unwrap();
    assert_eq!(order_status(&pool, order).await, ("paid".into(), "held".into()));
    assert_eq!(
        tickets_left(&pool, event).await,
        0,
        "no seat conjured for a sold ticket"
    );
}

/// Control-presence: the sweep's `hold_expires_at IS NOT NULL` guard. Orders from before
/// PAY-3 never reserved a seat; releasing one for them would create capacity from nothing.
#[sqlx::test(migrations = "./migrations")]
async fn sec_expiry_sweep_skips_orders_that_never_held_a_seat(pool: PgPool) {
    let app = make_app(pool.clone());
    let owner = token_for(&app, "owner@x.co").await;
    let a = token_for(&app, "a@x.co").await;
    let event = create_event(&app, &owner, 1).await;
    let (_, v) = checkout(&app, &a, event, None).await;
    let order = order_id(&v).await;
    // Simulate a legacy pending order: it holds no seat, so give the seat back and clear the hold.
    sqlx::query("UPDATE orders SET hold_expires_at = NULL WHERE id = $1")
        .bind(order)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE events SET tickets_left = 1 WHERE id = $1")
        .bind(event)
        .execute(&pool)
        .await
        .unwrap();

    orders::expire_holds(&pool).await.unwrap();
    assert_eq!(order_status(&pool, order).await.0, "pending", "untouched");
    assert_eq!(tickets_left(&pool, event).await, 1, "and no seat added");
}

#[sqlx::test(migrations = "./migrations")]
async fn sec_failed_charge_closes_the_order_and_returns_the_seat(pool: PgPool) {
    let app = make_app(pool.clone());
    let owner = token_for(&app, "owner@x.co").await;
    let a = token_for(&app, "a@x.co").await;
    let event = create_event(&app, &owner, 1).await;
    // The noop provider refuses any non-zero charge — a deterministic provider failure.
    sqlx::query("UPDATE payment_settings SET provider = 'noop' WHERE id = TRUE")
        .execute(&pool)
        .await
        .unwrap();

    let (s, _) = checkout(&app, &a, event, None).await;
    assert_eq!(s, StatusCode::CONFLICT);
    let (status,): (String,) =
        sqlx::query_as("SELECT status FROM orders WHERE buyer_id = (SELECT id FROM users WHERE email = 'a@x.co')")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        status, "failed",
        "no pending row a buyer could later be charged against"
    );
    assert_eq!(tickets_left(&pool, event).await, 1, "the seat taken for it came back");
}

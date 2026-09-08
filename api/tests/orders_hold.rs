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
use underground_api::stripe::sign_payload;
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
    // A status of its own — NOT `paid`: the buyer has no ticket and the money is going back, so
    // it must never read as revenue or trigger the ticket email (REVIEW-2 N3).
    assert_eq!(
        order_status(&pool, order_a).await,
        ("refund_due".into(), "refund_due".into())
    );
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

    // Orders are placed on the `test` provider (no network), then A is re-labelled as the
    // Stripe order checkout would have written: provider `stripe`, provider_ref = the session.
    let event = create_event(&app, &owner, 1).await;
    let (_, va) = checkout(&app, &a, event, None).await;
    let order_a = order_id(&va).await;
    lapse(&pool, order_a).await;
    orders::expire_holds(&pool).await.unwrap();
    assert_eq!(checkout(&app, &b, event, None).await.0, StatusCode::CREATED);
    sqlx::query("UPDATE orders SET provider = 'stripe', provider_ref = 'cs_A' WHERE id = $1")
        .bind(order_a)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE payment_settings SET provider = 'stripe', mode = 'test', \
            stripe_secret_key = 'sk_test_x', stripe_webhook_secret = 'whsec_test' WHERE id = TRUE",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Stripe reports A paid — on the object we created, for the amount we asked — through the
    // real route with a real signature. Sold out meanwhile: the answer is still 200.
    let raw = json!({"id":"evt_late","type":"checkout.session.completed","data":{"object":{
        "id":"cs_A","payment_intent":"pi_A","payment_status":"paid","amount_total":10000,"currency":"uah",
        "metadata":{"order_id":order_a.to_string()}}}})
    .to_string();
    let ts = chrono::Utc::now().timestamp();
    let req = Request::builder()
        .method("POST")
        .uri("/api/webhooks/stripe")
        .header("content-type", "application/json")
        .header(
            "stripe-signature",
            format!("t={ts},v1={}", sign_payload("whsec_test", ts, raw.as_bytes())),
        )
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

/// Behaviour, not control-presence: orders from before PAY-3 never reserved a seat, and the
/// sweep must not hand one back for them. (The explicit `IS NOT NULL` in the sweep is
/// documentation — SQL's `NULL < now()` is already NULL, so removing it changes nothing. A
/// review caught the earlier comment claiming otherwise.)
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

/// The capacity ceiling as a DATABASE control: `release_seats` clamps with LEAST, so the
/// CHECK only matters for a write that bypasses it. This is the write that bypasses it.
#[sqlx::test(migrations = "./migrations")]
async fn a_direct_write_above_capacity_is_refused_by_the_database(pool: PgPool) {
    let app = make_app(pool.clone());
    let owner = token_for(&app, "owner@x.co").await;
    let event = create_event(&app, &owner, 3).await;
    let res = sqlx::query("UPDATE events SET tickets_left = tickets_total + 1 WHERE id = $1")
        .bind(event)
        .execute(&pool)
        .await;
    assert!(
        res.is_err(),
        "CHECK events_left_le_total must reject tickets_left > tickets_total"
    );
    assert_eq!(tickets_left(&pool, event).await, 3);
}

/// [SEC] An order from before the hold existed (hold_expires_at NULL) never took a seat. Paying
/// it must take one NOW — and when none is left, the money goes back, never a ticket without
/// capacity. Found by the second review: the old code minted with no decrement.
#[sqlx::test(migrations = "./migrations")]
async fn a_legacy_pending_order_takes_a_seat_at_payment_or_is_refunded(pool: PgPool) {
    let app = make_app(pool.clone());
    let owner = token_for(&app, "owner@x.co").await;
    let a = token_for(&app, "a@x.co").await;
    let b = token_for(&app, "b@x.co").await;
    let event = create_event(&app, &owner, 1).await;
    let (_, v) = checkout(&app, &a, event, None).await;
    let order = order_id(&v).await;
    // Rewind to the pre-migration shape: no hold, and the seat it "took" back on sale.
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

    // Seat available: payment takes it. The stock MUST drop.
    assert!(matches!(fulfill_now(&pool, order).await, FulfillOutcome::Fulfilled(_)));
    assert_eq!(
        tickets_left(&pool, event).await,
        0,
        "a legacy order consumes capacity like any other"
    );

    // Second legacy order, no seat left → refund, not a ticket from thin air.
    sqlx::query("UPDATE events SET tickets_left = 1 WHERE id = $1")
        .bind(event)
        .execute(&pool)
        .await
        .unwrap();
    let (_, vb) = checkout(&app, &b, event, None).await;
    let order_b = order_id(&vb).await;
    sqlx::query("UPDATE orders SET hold_expires_at = NULL WHERE id = $1")
        .bind(order_b)
        .execute(&pool)
        .await
        .unwrap();
    // tickets_left is 0 now (B's checkout took the seat, then we detached B from its hold).
    assert_eq!(fulfill_now(&pool, order_b).await, FulfillOutcome::RefundDue);
    let live: i64 = sqlx::query_scalar("SELECT count(*) FROM tickets WHERE event_id = $1 AND status = 'active'")
        .bind(event)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(live, 1, "one seat, one ticket — never two");
}

/// `refund_due` is not `paid`: the buyer has no ticket and the money is going back, so it
/// must not be the organiser's revenue, the platform's GMV, or a "Ваш квиток" email.
#[sqlx::test(migrations = "./migrations")]
async fn refund_due_orders_are_not_revenue_and_get_no_ticket_email(pool: PgPool) {
    let app = make_app(pool.clone());
    let owner = token_for(&app, "owner@x.co").await;
    let a = token_for(&app, "a@x.co").await;
    let b = token_for(&app, "b@x.co").await;
    let event = create_event(&app, &owner, 1).await;
    let (_, va) = checkout(&app, &a, event, None).await;
    let order_a = order_id(&va).await;
    lapse(&pool, order_a).await;
    orders::expire_holds(&pool).await.unwrap();
    assert_eq!(checkout(&app, &b, event, None).await.0, StatusCode::CREATED);
    assert_eq!(fulfill_now(&pool, order_a).await, FulfillOutcome::RefundDue);
    assert_eq!(
        order_status(&pool, order_a).await.0,
        "refund_due",
        "a status of its own, not 'paid'"
    );

    let emails = underground_api::email::pending(&pool, 100).await.unwrap();
    assert!(
        emails.iter().all(|e| e.order_id != order_a),
        "no confirmation email for money that is going back"
    );
    let resp = app
        .clone()
        .oneshot(build("GET", "/api/organizer/stats", Some(&owner), None))
        .await
        .unwrap();
    let stats = json_body(resp).await;
    assert_eq!(
        stats["revenue"].as_str().unwrap_or("0"),
        "0",
        "not the organiser's money: {stats}"
    );
    assert_eq!(stats["tickets_sold"], 0);
}

/// [SEC] An admin refund of a PENDING order (seat held, no ticket yet) must hand the seat back;
/// once the order is 'refunded' the expiry sweep will never see it again.
#[sqlx::test(migrations = "./migrations")]
async fn admin_refund_of_a_pending_order_returns_its_seat(pool: PgPool) {
    let app = make_app(pool.clone());
    let owner = token_for(&app, "owner@x.co").await;
    let a = token_for(&app, "a@x.co").await;
    let admin = token_for(&app, "admin@x.co").await;
    sqlx::query("UPDATE users SET role = 'ADMIN' WHERE email = 'admin@x.co'")
        .execute(&pool)
        .await
        .unwrap();
    let event = create_event(&app, &owner, 1).await;
    let (_, v) = checkout(&app, &a, event, None).await;
    let order = order_id(&v).await;
    assert_eq!(tickets_left(&pool, event).await, 0);

    let resp = app
        .clone()
        .oneshot(build(
            "POST",
            &format!("/api/admin/orders/{order}/escrow"),
            Some(&admin),
            Some(json!({"action":"refund"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(tickets_left(&pool, event).await, 1, "the held seat is back on sale");
    assert_eq!(order_status(&pool, order).await.0, "refunded");
    // The sweep must not release it a second time.
    lapse(&pool, order).await;
    orders::expire_holds(&pool).await.unwrap();
    assert_eq!(tickets_left(&pool, event).await, 1);

    // The payment lands anyway — the buyer's page completed after the admin closed the order.
    // Nothing had been taken at close time, so the money is now a recorded debt: never a silent
    // AlreadyHandled with no refund on file (REVIEW-3 #1).
    let (marker,): (Option<String>,) = sqlx::query_as("SELECT terminal_event_seen FROM orders WHERE id = $1")
        .bind(order)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(marker.as_deref(), Some("cancelled"));
    assert_eq!(fulfill_now(&pool, order).await, FulfillOutcome::RefundDue);
    assert_eq!(refund_jobs_for(&pool, order).await, 1, "the debt is on record");
    assert_eq!(order_status(&pool, order).await.0, "refund_due");
    assert_eq!(
        tickets_left(&pool, event).await,
        1,
        "no seat is taken for money that goes back"
    );
}

/// Admin refund of a PAID order: the return is a debt on record for the refund executor, and a
/// replayed payment afterwards neither mints nor queues a second return.
#[sqlx::test(migrations = "./migrations")]
async fn admin_refund_of_a_paid_order_records_the_debt_once(pool: PgPool) {
    let app = make_app(pool.clone());
    let owner = token_for(&app, "owner@x.co").await;
    let a = token_for(&app, "a@x.co").await;
    let admin = token_for(&app, "admin@x.co").await;
    sqlx::query("UPDATE users SET role = 'ADMIN' WHERE email = 'admin@x.co'")
        .execute(&pool)
        .await
        .unwrap();
    let event = create_event(&app, &owner, 1).await;
    let (_, v) = checkout(&app, &a, event, None).await;
    let order = order_id(&v).await;
    assert!(matches!(fulfill_now(&pool, order).await, FulfillOutcome::Fulfilled(_)));

    let resp = app
        .clone()
        .oneshot(build(
            "POST",
            &format!("/api/admin/orders/{order}/escrow"),
            Some(&admin),
            Some(json!({"action":"refund"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        refund_jobs_for(&pool, order).await,
        1,
        "money was taken, so its return is queued"
    );
    assert_eq!(tickets_left(&pool, event).await, 1, "the ticket's seat is back on sale");
    let (marker,): (Option<String>,) = sqlx::query_as("SELECT terminal_event_seen FROM orders WHERE id = $1")
        .bind(order)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(marker.as_deref(), Some("refunded"));
    // A replayed Paid: nothing to mint, and NOT a second refund.
    assert_eq!(fulfill_now(&pool, order).await, FulfillOutcome::AlreadyHandled);
    assert_eq!(refund_jobs_for(&pool, order).await, 1);
    assert_eq!(order_status(&pool, order).await.0, "refunded");
}

/// [SEC] The "never a ticket for a superseded checkout" rule lives in the function that mints,
/// not only in the webhook that usually calls it (REVIEW-3 #5).
#[sqlx::test(migrations = "./migrations")]
async fn a_superseded_order_paid_through_fulfill_directly_is_refunded_not_ticketed(pool: PgPool) {
    let app = make_app(pool.clone());
    let owner = token_for(&app, "owner@x.co").await;
    let a = token_for(&app, "a@x.co").await;
    let event = create_event(&app, &owner, 2).await;
    let (_, v1) = checkout(&app, &a, event, None).await;
    let first = order_id(&v1).await;
    assert_eq!(checkout(&app, &a, event, None).await.0, StatusCode::CREATED);
    assert_eq!(tickets_left(&pool, event).await, 1, "one buyer, one hold");

    assert_eq!(fulfill_now(&pool, first).await, FulfillOutcome::RefundDue);
    assert_eq!(
        order_status(&pool, first).await,
        ("refund_due".into(), "refund_due".into())
    );
    assert_eq!(refund_jobs_for(&pool, first).await, 1);
    let minted: i64 = sqlx::query_scalar("SELECT count(*) FROM tickets WHERE event_id = $1")
        .bind(event)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(minted, 0);
    assert_eq!(tickets_left(&pool, event).await, 1, "the superseded order took nothing");
}

/// A pending order from before the hold existed is superseded by a re-checkout like any other —
/// but hands back no seat, because it never took one (REVIEW-3 #6, BR-023/BR-024).
#[sqlx::test(migrations = "./migrations")]
async fn a_legacy_hold_less_pending_order_is_superseded_without_releasing_a_seat(pool: PgPool) {
    let app = make_app(pool.clone());
    let owner = token_for(&app, "owner@x.co").await;
    let a = token_for(&app, "a@x.co").await;
    // 3 seats with 2 on sale: a wrong extra release would show as 2 -> 3, not be swallowed by
    // the LEAST(tickets_total, …) ceiling (REVIEW-4 #3 — the 2-seat version could not detect it).
    let event = create_event(&app, &owner, 3).await;
    let (_, v1) = checkout(&app, &a, event, None).await;
    let legacy = order_id(&v1).await;
    // Rewind to the pre-migration shape: no hold, and the seat it "took" back on sale.
    sqlx::query("UPDATE orders SET hold_expires_at = NULL WHERE id = $1")
        .bind(legacy)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE events SET tickets_left = 2 WHERE id = $1")
        .bind(event)
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(checkout(&app, &a, event, None).await.0, StatusCode::CREATED);
    assert_eq!(
        tickets_left(&pool, event).await,
        1,
        "the new hold took one seat and nothing was released for an order that held none"
    );
    assert_eq!(order_status(&pool, legacy).await.0, "expired");
    let (marker,): (Option<String>,) = sqlx::query_as("SELECT terminal_event_seen FROM orders WHERE id = $1")
        .bind(legacy)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(marker.as_deref(), Some("superseded"));
}

/// [SEC] Re-checkout on the LAST seat: the buyer already holds it, so the second checkout must
/// hand it back and take it again — one order, one seat — not answer "квитки закінчились".
#[sqlx::test(migrations = "./migrations")]
async fn sec_rechecking_out_on_the_last_seat_succeeds_and_supersedes(pool: PgPool) {
    let app = make_app(pool.clone());
    let owner = token_for(&app, "owner@x.co").await;
    let a = token_for(&app, "a@x.co").await;
    let event = create_event(&app, &owner, 1).await;
    let (s1, v1) = checkout(&app, &a, event, None).await;
    assert_eq!(s1, StatusCode::CREATED);
    let first = order_id(&v1).await;
    let (s2, v2) = checkout(&app, &a, event, None).await;
    assert_eq!(
        s2,
        StatusCode::CREATED,
        "the buyer's own seat is re-taken, not refused: {v2}"
    );
    assert_eq!(tickets_left(&pool, event).await, 0);
    let (status, _) = order_status(&pool, first).await;
    assert_eq!(status, "expired");
    let (marker,): (Option<String>,) = sqlx::query_as("SELECT terminal_event_seen FROM orders WHERE id = $1")
        .bind(first)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        marker.as_deref(),
        Some("superseded"),
        "a late payment on it becomes a refund, never a ticket"
    );
}

/// Admin refund of an EXPIRED order: no ticket, no hold, no money — nothing to hand back, and a
/// payment that lands later is money we never asked for, so it goes back (REVIEW-4 question c).
#[sqlx::test(migrations = "./migrations")]
async fn admin_refund_of_an_expired_order_takes_no_seat_and_refunds_a_late_payment(pool: PgPool) {
    let app = make_app(pool.clone());
    let owner = token_for(&app, "owner@x.co").await;
    let a = token_for(&app, "a@x.co").await;
    let admin = token_for(&app, "admin@x.co").await;
    sqlx::query("UPDATE users SET role = 'ADMIN' WHERE email = 'admin@x.co'")
        .execute(&pool)
        .await
        .unwrap();
    let event = create_event(&app, &owner, 1).await;
    let (_, v) = checkout(&app, &a, event, None).await;
    let order = order_id(&v).await;
    lapse(&pool, order).await;
    orders::expire_holds(&pool).await.unwrap();
    assert_eq!(tickets_left(&pool, event).await, 1);

    let resp = app
        .clone()
        .oneshot(build(
            "POST",
            &format!("/api/admin/orders/{order}/escrow"),
            Some(&admin),
            Some(json!({"action":"refund"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        tickets_left(&pool, event).await,
        1,
        "nothing was held, nothing is released"
    );
    assert_eq!(
        refund_jobs_for(&pool, order).await,
        0,
        "no money was taken, no debt yet"
    );
    assert_eq!(fulfill_now(&pool, order).await, FulfillOutcome::RefundDue);
    assert_eq!(refund_jobs_for(&pool, order).await, 1, "…until the payment lands");
    assert_eq!(tickets_left(&pool, event).await, 1);
}

/// A full-refund notice for an order we do not have is acknowledged and changes nothing.
#[sqlx::test(migrations = "./migrations")]
async fn a_full_refund_of_an_unknown_order_is_a_no_op(pool: PgPool) {
    let mut conn = pool.acquire().await.unwrap();
    orders::apply_full_refund(&mut conn, Uuid::new_v4()).await.unwrap();
    let orders: i64 = sqlx::query_scalar("SELECT count(*) FROM orders")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(orders, 0);
}

/// [SEC] Two checkouts of ONE buyer racing each other. Neither request can see the other's
/// uncommitted hold, so the rule lives in the database: the second INSERT hits the one-live-hold
/// index, gets 409, and its seat decrement rolls back — capacity is never pinned twice (BR-026).
/// Reproduced deterministically: an uncommitted hold on one connection, the real checkout on
/// another, then the first commits.
#[sqlx::test(migrations = "./migrations")]
async fn sec_two_simultaneous_checkouts_of_one_buyer_yield_one_hold(pool: PgPool) {
    let app = make_app(pool.clone());
    let owner = token_for(&app, "owner@x.co").await;
    let a = token_for(&app, "a@x.co").await;
    let event = create_event(&app, &owner, 3).await;
    let buyer: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email = 'a@x.co'")
        .fetch_one(&pool)
        .await
        .unwrap();

    // Request 1, frozen just after its INSERT (not yet committed).
    let mut first = pool.begin().await.unwrap();
    sqlx::query(
        "INSERT INTO orders (event_id, buyer_id, amount, currency, provider, hold_expires_at) \
         VALUES ($1, $2, 100, 'UAH', 'test', now() + interval '15 minutes')",
    )
    .bind(event)
    .bind(buyer)
    .execute(&mut *first)
    .await
    .unwrap();
    sqlx::query("UPDATE events SET tickets_left = tickets_left - 1 WHERE id = $1")
        .bind(event)
        .execute(&mut *first)
        .await
        .unwrap();

    // Request 2 runs for real and blocks on the index until request 1 decides. Wait for the
    // block to be REAL (a backend in a lock wait) rather than for a fixed time: through the test
    // tunnel a checkout's eight round trips can take longer than any sleep we would pick.
    let app2 = app.clone();
    let racing = tokio::spawn(async move { checkout(&app2, &a, event, None).await });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        let waiting: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_stat_activity WHERE datname = current_database() AND wait_event_type = 'Lock'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        if waiting > 0 {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the racing checkout never reached the index"
        );
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    first.commit().await.unwrap();
    let (status, body) = racing.await.unwrap();
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(
        body["error"].as_str().unwrap_or("").contains("активний checkout"),
        "{body}"
    );

    let holds: i64 = sqlx::query_scalar("SELECT count(*) FROM orders WHERE event_id = $1 AND status = 'pending'")
        .bind(event)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(holds, 1, "one buyer, one live hold");
    assert_eq!(tickets_left(&pool, event).await, 2, "the loser's seat went back");
}

/// A database failure while the order row is written is a 500 — and the seat the checkout had
/// already taken goes back with the transaction, never leaking capacity.
#[sqlx::test(migrations = "./migrations")]
async fn a_database_failure_at_order_creation_rolls_the_seat_back(pool: PgPool) {
    let app = make_app(pool.clone());
    let owner = token_for(&app, "owner@x.co").await;
    let a = token_for(&app, "a@x.co").await;
    let event = create_event(&app, &owner, 2).await;
    sqlx::query(
        "CREATE FUNCTION boom() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected'; END $$",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("CREATE TRIGGER boom_on_order BEFORE INSERT ON orders FOR EACH ROW EXECUTE FUNCTION boom()")
        .execute(&pool)
        .await
        .unwrap();
    let (status, _) = checkout(&app, &a, event, None).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        tickets_left(&pool, event).await,
        2,
        "the decrement rolled back with the failed insert"
    );
}

/// A further payment event on an order whose refund is ALREADY on record changes nothing: the
/// debt stays one row, and an admin's `refunded` is not flipped back to `refund_due` (REVIEW-4).
#[sqlx::test(migrations = "./migrations")]
async fn a_repeat_payment_on_a_recorded_debt_is_already_handled(pool: PgPool) {
    let app = make_app(pool.clone());
    let owner = token_for(&app, "owner@x.co").await;
    let a = token_for(&app, "a@x.co").await;
    let b = token_for(&app, "b@x.co").await;
    let admin = token_for(&app, "admin@x.co").await;
    sqlx::query("UPDATE users SET role = 'ADMIN' WHERE email = 'admin@x.co'")
        .execute(&pool)
        .await
        .unwrap();
    let event = create_event(&app, &owner, 1).await;
    let (_, va) = checkout(&app, &a, event, None).await;
    let order_a = order_id(&va).await;
    lapse(&pool, order_a).await;
    orders::expire_holds(&pool).await.unwrap();
    assert_eq!(checkout(&app, &b, event, None).await.0, StatusCode::CREATED);
    assert_eq!(fulfill_now(&pool, order_a).await, FulfillOutcome::RefundDue);
    assert_eq!(refund_jobs_for(&pool, order_a).await, 1);

    // The admin closes it out (the debt is already queued — nothing new is inserted).
    let resp = app
        .clone()
        .oneshot(build(
            "POST",
            &format!("/api/admin/orders/{order_a}/escrow"),
            Some(&admin),
            Some(json!({"action":"refund"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(order_status(&pool, order_a).await.0, "refunded");
    assert_eq!(refund_jobs_for(&pool, order_a).await, 1);

    // A second payment event for the same order (a different delivery, same money).
    assert_eq!(fulfill_now(&pool, order_a).await, FulfillOutcome::AlreadyHandled);
    assert_eq!(
        order_status(&pool, order_a).await.0,
        "refunded",
        "the admin's decision stands"
    );
    assert_eq!(refund_jobs_for(&pool, order_a).await, 1, "still one debt");
}

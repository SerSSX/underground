//! The Stripe webhook through the REAL route: signature, binding, replay, ordering, refunds.
//!
//! These are the abuse rows from the independent review of PAY-1/PAY-4. Each one is a thing
//! that must be REFUSED, exercised end to end — not on the pure `verify_signature` function
//! (a mutation that removed the call in `verify_webhook` left every unit test green), but on
//! `POST /api/webhooks/stripe` with a real body and a real header.
//!
//! No network: orders are placed with the `test` provider through real checkout (so the seat
//! hold is real), then re-labelled as Stripe orders by SQL — the same rows checkout would
//! have written had the charge gone to Stripe.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use chrono::Utc;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use underground_api::routes::router;
use underground_api::state::AppState;
use underground_api::stripe::sign_payload;
use uuid::Uuid;

const SECRET: &str = "whsec_route_test";

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
    let body = body.map(|v| Body::from(v.to_string())).unwrap_or_else(Body::empty);
    b.body(body).unwrap()
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

/// Settings as an operator would leave them for Stripe in test mode. Written by SQL so the
/// route under test is the only thing between the request and the database.
async fn stripe_settings(pool: &PgPool) {
    sqlx::query(
        "UPDATE payment_settings SET provider = 'stripe', mode = 'test', \
            stripe_secret_key = 'sk_test_route', stripe_webhook_secret = $1 WHERE id = TRUE",
    )
    .bind(SECRET)
    .execute(pool)
    .await
    .unwrap();
}

/// Register, create a 2-seat paid event (100 UAH), and place a pending order for `buyer`
/// through real checkout on the `test` provider. Then re-label the order as one Stripe would
/// have produced: provider `stripe`, provider_ref = the Checkout Session id we "created".
///
/// The active provider is flipped to `test` for the checkout and back to `stripe` after: with
/// `stripe` active, checkout would call the real Stripe API with the fake key and 409. The
/// flip is inside the helper so test order cannot get it wrong.
async fn stripe_order(app: &Router, pool: &PgPool, buyer: &str, session: &str) -> (Uuid, Uuid) {
    sqlx::query("UPDATE payment_settings SET provider = 'test' WHERE id = TRUE")
        .execute(pool)
        .await
        .unwrap();
    let out = place_order(app, pool, buyer, session).await;
    sqlx::query("UPDATE payment_settings SET provider = 'stripe' WHERE id = TRUE")
        .execute(pool)
        .await
        .unwrap();
    out
}

async fn place_order(app: &Router, pool: &PgPool, buyer: &str, session: &str) -> (Uuid, Uuid) {
    let owner = token_for(app, &format!("owner-{session}@x.co")).await;
    let resp = app
        .clone()
        .oneshot(build(
            "POST",
            "/api/events",
            Some(&owner),
            Some(json!({"title":"Stripe Gig","category":"Rock","price":"100","currency":"UAH","tickets_total":2,"cost_type":"paid"})),
        ))
        .await
        .unwrap();
    let event = Uuid::parse_str(json_body(resp).await["id"].as_str().unwrap()).unwrap();
    let resp = app
        .clone()
        .oneshot(build(
            "POST",
            &format!("/api/events/{event}/checkout"),
            Some(buyer),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let order = Uuid::parse_str(json_body(resp).await["order_id"].as_str().unwrap()).unwrap();
    sqlx::query("UPDATE orders SET provider = 'stripe', provider_ref = $2 WHERE id = $1")
        .bind(order)
        .bind(session)
        .execute(pool)
        .await
        .unwrap();
    (order, event)
}

fn stripe_event(id: &str, kind: &str, object: Value) -> Value {
    json!({ "id": id, "type": kind, "data": { "object": object } })
}

fn paid_session(session: &str, order: Uuid, amount_total: i64) -> Value {
    json!({ "id": session, "payment_intent": format!("pi_{session}"), "payment_status": "paid",
            "amount_total": amount_total, "currency": "uah", "metadata": { "order_id": order.to_string() } })
}

/// POST with a real `Stripe-Signature` computed under `secret` at `now`.
async fn post(app: &Router, payload: &Value, secret: &str) -> StatusCode {
    let raw = payload.to_string();
    let ts = Utc::now().timestamp();
    let req = Request::builder()
        .method("POST")
        .uri("/api/webhooks/stripe")
        .header("content-type", "application/json")
        .header(
            "stripe-signature",
            format!("t={ts},v1={}", sign_payload(secret, ts, raw.as_bytes())),
        )
        .body(Body::from(raw))
        .unwrap();
    app.clone().oneshot(req).await.unwrap().status()
}

async fn order_state(pool: &PgPool, order: Uuid) -> (String, String, Option<String>, Option<String>) {
    sqlx::query_as("SELECT status, escrow_status, provider_ref, terminal_event_seen FROM orders WHERE id = $1")
        .bind(order)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn count(pool: &PgPool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

async fn tickets_for(pool: &PgPool, order: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM tickets t JOIN orders o ON o.ticket_id = t.id WHERE o.id = $1 AND t.status = 'active'",
    )
    .bind(order)
    .fetch_one(pool)
    .await
    .unwrap()
}

// ---------------------------------------------------------------- signature (wiring)

/// [SEC] The signature check must be REACHED from the route. A mutation that removed the
/// call left every unit test green; this one goes red.
#[sqlx::test(migrations = "./migrations")]
async fn a_signature_under_another_secret_is_refused_and_counted(pool: PgPool) {
    let app = make_app(pool.clone());
    stripe_settings(&pool).await;
    let buyer = token_for(&app, "b@x.co").await;
    let (order, _) = stripe_order(&app, &pool, &buyer, "cs_A").await;
    let payload = stripe_event(
        "evt_1",
        "checkout.session.completed",
        paid_session("cs_A", order, 10_000),
    );

    assert_eq!(post(&app, &payload, "whsec_attacker").await, StatusCode::UNAUTHORIZED);
    // No header at all → 401 too.
    let raw = payload.to_string();
    let bare = Request::builder()
        .method("POST")
        .uri("/api/webhooks/stripe")
        .body(Body::from(raw))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(bare).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );

    assert_eq!(
        count(&pool, "SELECT count(*) FROM webhook_events").await,
        0,
        "nothing recorded"
    );
    assert_eq!(order_state(&pool, order).await.0, "pending");
    let (rejections, last): (i64, Option<chrono::DateTime<Utc>>) =
        sqlx::query_as("SELECT stripe_webhook_rejections, stripe_last_webhook_at FROM payment_settings")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(rejections, 2, "the operator can SEE a wrong secret");
    assert!(last.is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn a_bound_paid_event_mints_once_and_replays_are_duplicates(pool: PgPool) {
    let app = make_app(pool.clone());
    stripe_settings(&pool).await;
    let buyer = token_for(&app, "b@x.co").await;
    let (order, event) = stripe_order(&app, &pool, &buyer, "cs_A").await;
    let payload = stripe_event(
        "evt_1",
        "checkout.session.completed",
        paid_session("cs_A", order, 10_000),
    );

    assert_eq!(post(&app, &payload, SECRET).await, StatusCode::OK);
    let (status, escrow, provider_ref, _) = order_state(&pool, order).await;
    assert_eq!((status.as_str(), escrow.as_str()), ("paid", "held"));
    assert_eq!(
        provider_ref.as_deref(),
        Some("pi_cs_A"),
        "refund handle moved to the PaymentIntent"
    );
    assert_eq!(tickets_for(&pool, order).await, 1);
    let left: i32 = sqlx::query_scalar("SELECT tickets_left FROM events WHERE id = $1")
        .bind(event)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        left, 1,
        "the seat reserved at checkout is the one sold — not a second one"
    );
    let (rejections, last): (i64, Option<chrono::DateTime<Utc>>) =
        sqlx::query_as("SELECT stripe_webhook_rejections, stripe_last_webhook_at FROM payment_settings")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(rejections, 0);
    assert!(last.is_some(), "a passing delivery proves the secret");

    // Replay inside the tolerance window: same event id → 200, no second ticket, one row.
    assert_eq!(post(&app, &payload, SECRET).await, StatusCode::OK);
    assert_eq!(tickets_for(&pool, order).await, 1);
    assert_eq!(count(&pool, "SELECT count(*) FROM webhook_events").await, 1);

    // [SEC] Same event id, DIFFERENT body: the first body was applied; this one is ignored.
    let other_buyer = token_for(&app, "c@x.co").await;
    let (other, _) = stripe_order(&app, &pool, &other_buyer, "cs_B").await;
    let forged = stripe_event(
        "evt_1",
        "checkout.session.completed",
        paid_session("cs_B", other, 10_000),
    );
    assert_eq!(post(&app, &forged, SECRET).await, StatusCode::OK);
    assert_eq!(
        order_state(&pool, other).await.0,
        "pending",
        "a reused event id cannot smuggle a second payment"
    );
}

// ---------------------------------------------------------------- binding (the CRITICAL)

/// [SEC] Pay for order B's session while naming order A in metadata. A is NOT fulfilled:
/// the event arrived on cs_B, and A was created with cs_A.
#[sqlx::test(migrations = "./migrations")]
async fn a_paid_event_names_another_order_in_metadata_and_is_ignored(pool: PgPool) {
    let app = make_app(pool.clone());
    stripe_settings(&pool).await;
    let attacker = token_for(&app, "attacker@x.co").await;
    let (cheap, _) = stripe_order(&app, &pool, &attacker, "cs_cheap").await;
    let (expensive, _) = stripe_order(&app, &pool, &attacker, "cs_expensive").await;

    // The session that was actually paid is the cheap one; metadata claims the expensive one.
    let forged = stripe_event(
        "evt_f",
        "checkout.session.completed",
        paid_session("cs_cheap", expensive, 10_000),
    );
    assert_eq!(
        post(&app, &forged, SECRET).await,
        StatusCode::OK,
        "acknowledged — Stripe must not retry"
    );
    assert_eq!(
        order_state(&pool, expensive).await.0,
        "pending",
        "the expensive order is untouched"
    );
    assert_eq!(
        order_state(&pool, cheap).await.0,
        "pending",
        "and the cheap one was not named, so nothing minted"
    );
    assert_eq!(count(&pool, "SELECT count(*) FROM tickets").await, 0);
    assert_eq!(
        count(&pool, "SELECT count(*) FROM webhook_events").await,
        1,
        "recorded, not applied"
    );
}

/// [SEC] The right session, the wrong amount.
#[sqlx::test(migrations = "./migrations")]
async fn a_paid_event_with_a_different_amount_or_currency_is_ignored(pool: PgPool) {
    let app = make_app(pool.clone());
    stripe_settings(&pool).await;
    let buyer = token_for(&app, "b@x.co").await;
    let (order, _) = stripe_order(&app, &pool, &buyer, "cs_A").await;

    let short = stripe_event("evt_s", "checkout.session.completed", paid_session("cs_A", order, 100));
    assert_eq!(post(&app, &short, SECRET).await, StatusCode::OK);
    assert_eq!(
        order_state(&pool, order).await.0,
        "pending",
        "1.00 does not buy a 100.00 ticket"
    );

    let mut wrong_cur = paid_session("cs_A", order, 10_000);
    wrong_cur["currency"] = json!("eur");
    let wrong = stripe_event("evt_c", "checkout.session.completed", wrong_cur);
    assert_eq!(post(&app, &wrong, SECRET).await, StatusCode::OK);
    assert_eq!(
        order_state(&pool, order).await.0,
        "pending",
        "10000 EUR is not 10000 UAH"
    );
}

/// [SEC] A Stripe delivery naming a `test`-provider order does nothing to it.
#[sqlx::test(migrations = "./migrations")]
async fn a_stripe_event_cannot_touch_an_order_from_another_provider(pool: PgPool) {
    let app = make_app(pool.clone());
    stripe_settings(&pool).await;
    let buyer = token_for(&app, "b@x.co").await;
    let (order, _) = stripe_order(&app, &pool, &buyer, "cs_A").await;
    sqlx::query("UPDATE orders SET provider = 'test' WHERE id = $1")
        .bind(order)
        .execute(&pool)
        .await
        .unwrap();
    let payload = stripe_event(
        "evt_x",
        "checkout.session.completed",
        paid_session("cs_A", order, 10_000),
    );
    assert_eq!(post(&app, &payload, SECRET).await, StatusCode::OK);
    assert_eq!(order_state(&pool, order).await.0, "pending");
}

#[sqlx::test(migrations = "./migrations")]
async fn the_stripe_route_refuses_when_another_provider_is_active(pool: PgPool) {
    let app = make_app(pool.clone());
    // Default settings: provider `test`. A Stripe delivery has no business here.
    let payload = stripe_event("evt_1", "checkout.session.completed", json!({}));
    assert_eq!(post(&app, &payload, SECRET).await, StatusCode::CONFLICT);
}

// ---------------------------------------------------------------- ordering + refunds

/// Delivery is unordered. A refund that overtakes its payment must leave the order
/// unmintable — and the refund must be traceable to the order even though it arrives on the
/// Charge, which only carries metadata because checkout now sets payment_intent_data.
#[sqlx::test(migrations = "./migrations")]
async fn a_refund_that_arrives_before_the_payment_blocks_the_late_payment(pool: PgPool) {
    let app = make_app(pool.clone());
    stripe_settings(&pool).await;
    let buyer = token_for(&app, "b@x.co").await;
    let (order, _) = stripe_order(&app, &pool, &buyer, "cs_A").await;

    let refunded = stripe_event(
        "evt_r",
        "charge.refunded",
        json!({ "payment_intent": "pi_cs_A", "amount": 10_000, "amount_refunded": 10_000,
                "metadata": { "order_id": order.to_string() } }),
    );
    assert_eq!(post(&app, &refunded, SECRET).await, StatusCode::OK);
    let (_, escrow, _, terminal) = order_state(&pool, order).await;
    assert_eq!(escrow, "refunded");
    assert_eq!(terminal.as_deref(), Some("refunded"));

    let paid = stripe_event(
        "evt_p",
        "checkout.session.completed",
        paid_session("cs_A", order, 10_000),
    );
    assert_eq!(post(&app, &paid, SECRET).await, StatusCode::OK);
    assert_eq!(
        tickets_for(&pool, order).await,
        0,
        "money already went back — no ticket"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a_partial_refund_is_recorded_and_a_full_one_returns_the_ticket(pool: PgPool) {
    let app = make_app(pool.clone());
    stripe_settings(&pool).await;
    let buyer = token_for(&app, "b@x.co").await;
    let (order, event) = stripe_order(&app, &pool, &buyer, "cs_A").await;
    assert_eq!(
        post(
            &app,
            &stripe_event(
                "evt_p",
                "checkout.session.completed",
                paid_session("cs_A", order, 10_000)
            ),
            SECRET
        )
        .await,
        StatusCode::OK
    );
    assert_eq!(tickets_for(&pool, order).await, 1);

    // 40.00 of 100.00 back: bookkeeping only. Ticket stays valid, escrow stays held.
    let partial = stripe_event(
        "evt_r1",
        "charge.refunded",
        json!({ "payment_intent": "pi_cs_A", "amount": 10_000, "amount_refunded": 4_000,
                "metadata": { "order_id": order.to_string() } }),
    );
    assert_eq!(post(&app, &partial, SECRET).await, StatusCode::OK);
    let (refunded_amount,): (rust_decimal::Decimal,) =
        sqlx::query_as("SELECT refunded_amount FROM orders WHERE id = $1")
            .bind(order)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(refunded_amount.to_string(), "40.00");
    assert_eq!(
        order_state(&pool, order).await.1,
        "held",
        "a partial refund does not end the order"
    );
    assert_eq!(tickets_for(&pool, order).await, 1, "ticket still valid at the door");

    // The rest comes back: the sale is undone on our side too.
    let full = stripe_event(
        "evt_r2",
        "charge.refunded",
        json!({ "payment_intent": "pi_cs_A", "amount": 10_000, "amount_refunded": 10_000,
                "metadata": { "order_id": order.to_string() } }),
    );
    assert_eq!(post(&app, &full, SECRET).await, StatusCode::OK);
    assert_eq!(order_state(&pool, order).await.1, "refunded");
    assert_eq!(
        tickets_for(&pool, order).await,
        0,
        "ticket returned — it would be waved through at the door otherwise"
    );
    let left: i32 = sqlx::query_scalar("SELECT tickets_left FROM events WHERE id = $1")
        .bind(event)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(left, 2, "seat back on sale");
}

/// A dispute arrives on an object with no metadata at all. It is still traced to the order
/// through the PaymentIntent id we stored when the payment completed.
#[sqlx::test(migrations = "./migrations")]
async fn a_dispute_without_metadata_is_traced_through_the_payment_intent_and_freezes(pool: PgPool) {
    let app = make_app(pool.clone());
    stripe_settings(&pool).await;
    let buyer = token_for(&app, "b@x.co").await;
    let (order, _) = stripe_order(&app, &pool, &buyer, "cs_A").await;
    assert_eq!(
        post(
            &app,
            &stripe_event(
                "evt_p",
                "checkout.session.completed",
                paid_session("cs_A", order, 10_000)
            ),
            SECRET
        )
        .await,
        StatusCode::OK
    );
    let dispute = stripe_event(
        "evt_d",
        "charge.dispute.created",
        json!({ "id": "dp_1", "payment_intent": "pi_cs_A", "amount": 10_000 }),
    );
    assert_eq!(post(&app, &dispute, SECRET).await, StatusCode::OK);
    let (_, escrow, _, terminal) = order_state(&pool, order).await;
    assert_eq!(escrow, "frozen", "money is contested — the sweep must not pay it out");
    assert_eq!(terminal.as_deref(), Some("disputed"));
}

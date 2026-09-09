//! Escrow clearing lifecycle, payouts, disputes, and organizer verification (TASK-017 M6).

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use underground_api::escrow;
use underground_api::routes::router;
use underground_api::state::AppState;
use uuid::Uuid;

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

async fn register(app: &Router, email: &str) -> (String, String) {
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
    (
        v["token"].as_str().unwrap().to_string(),
        v["user"]["id"].as_str().unwrap().to_string(),
    )
}

async fn make_admin(app: &Router, pool: &PgPool, email: &str) -> String {
    let (token, _) = register(app, email).await;
    sqlx::query("UPDATE users SET role = 'ADMIN' WHERE email = $1")
        .bind(email)
        .execute(pool)
        .await
        .unwrap();
    token
}

async fn paid_event(app: &Router, maker: &str) -> String {
    let (st, v) = send(
        app,
        build(
            "POST",
            "/api/events",
            Some(maker),
            Some(json!({"title":"Paid","category":"Rock","city":"Kyiv","price":"100","tickets_total":5,"cost_type":"paid"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    v["id"].as_str().unwrap().to_string()
}

/// Checkout + pay-test → a held order; returns the order id.
async fn buy(app: &Router, buyer: &str, event: &str) -> String {
    let (_, c) = send(
        app,
        build("POST", &format!("/api/events/{event}/checkout"), Some(buyer), None),
    )
    .await;
    let order = c["order_id"].as_str().unwrap().to_string();
    let (st, _) = send(
        app,
        build("POST", &format!("/api/orders/{order}/pay-test"), Some(buyer), None),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    order
}

/// Backdate an event's end so the clearing sweep treats it as finished.
async fn end_event_days_ago(pool: &PgPool, event: &str, days: i32) {
    sqlx::query("UPDATE events SET ends_at = now() - make_interval(days => $1) WHERE id = $2")
        .bind(days)
        .bind(Uuid::parse_str(event).unwrap())
        .execute(pool)
        .await
        .unwrap();
}

async fn order_escrow(app: &Router, admin: &str, order: &str) -> String {
    let (_, orders) = send(app, build("GET", "/api/admin/orders", Some(admin), None)).await;
    orders.as_array().unwrap().iter().find(|o| o["id"] == order).unwrap()["escrow_status"]
        .as_str()
        .unwrap()
        .to_string()
}

#[sqlx::test(migrations = "./migrations")]
async fn sweep_advances_held_then_clears_after_window(pool: PgPool) {
    let app = make_app(pool.clone());
    let admin = make_admin(&app, &pool, "admin@x.co").await;
    let (maker, _) = register(&app, "m@x.co").await;
    let (buyer, _) = register(&app, "b@x.co").await;
    let event = paid_event(&app, &maker).await;
    let order = buy(&app, &buyer, &event).await;

    // Event hasn't ended → nothing moves.
    assert_eq!(escrow::sweep(&pool, 5).await.unwrap(), (0, 0));
    assert_eq!(order_escrow(&app, &admin, &order).await, "held");

    // Event ended 1 day ago, window is 5 → held becomes clearing (not yet cleared).
    end_event_days_ago(&pool, &event, 1).await;
    assert_eq!(escrow::sweep(&pool, 5).await.unwrap(), (1, 0));
    assert_eq!(order_escrow(&app, &admin, &order).await, "clearing");
    assert_eq!(escrow::sweep(&pool, 5).await.unwrap(), (0, 0), "still inside window");

    // Event ended 10 days ago → clearing becomes cleared.
    end_event_days_ago(&pool, &event, 10).await;
    assert_eq!(escrow::sweep(&pool, 5).await.unwrap(), (0, 1));
    assert_eq!(order_escrow(&app, &admin, &order).await, "cleared");
}

#[sqlx::test(migrations = "./migrations")]
async fn frozen_orders_are_never_swept(pool: PgPool) {
    let app = make_app(pool.clone());
    let (maker, _) = register(&app, "m@x.co").await;
    let (buyer, _) = register(&app, "b@x.co").await;
    let event = paid_event(&app, &maker).await;
    let order = buy(&app, &buyer, &event).await;

    // Buyer disputes → frozen; even a long-past event must not clear it.
    let (st, _) = send(
        &app,
        build("POST", &format!("/api/orders/{order}/dispute"), Some(&buyer), None),
    )
    .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    end_event_days_ago(&pool, &event, 30).await;
    assert_eq!(escrow::sweep(&pool, 5).await.unwrap(), (0, 0), "frozen excluded");
}

#[sqlx::test(migrations = "./migrations")]
async fn buyer_dispute_guards(pool: PgPool) {
    let app = make_app(pool.clone());
    let (maker, _) = register(&app, "m@x.co").await;
    let (buyer, _) = register(&app, "b@x.co").await;
    let (other, _) = register(&app, "o@x.co").await;
    let event = paid_event(&app, &maker).await;
    let order = buy(&app, &buyer, &event).await;

    // A non-buyer can't dispute → 409 (no matching held order for them).
    let (st, _) = send(
        &app,
        build("POST", &format!("/api/orders/{order}/dispute"), Some(&other), None),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT);
    // The buyer can → frozen.
    let (st, _) = send(
        &app,
        build("POST", &format!("/api/orders/{order}/dispute"), Some(&buyer), None),
    )
    .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    // A second dispute (already frozen, not held/clearing) → 409.
    let (st, _) = send(
        &app,
        build("POST", &format!("/api/orders/{order}/dispute"), Some(&buyer), None),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT);
}

#[sqlx::test(migrations = "./migrations")]
async fn payout_settles_cleared_escrow_net_of_fee(pool: PgPool) {
    let app = make_app(pool.clone());
    let admin = make_admin(&app, &pool, "admin@x.co").await;
    let (maker, maker_id) = register(&app, "m@x.co").await;
    let (buyer, _) = register(&app, "b@x.co").await;
    let event = paid_event(&app, &maker).await;
    let order = buy(&app, &buyer, &event).await;

    // Clear the escrow (event ended long ago → held → clearing → cleared in one sweep).
    end_event_days_ago(&pool, &event, 20).await;
    escrow::sweep(&pool, 5).await.unwrap();
    assert_eq!(order_escrow(&app, &admin, &order).await, "cleared");

    // Payout is blocked until the organizer passes KYC.
    let (st, _) = send(
        &app,
        build(
            "POST",
            &format!("/api/admin/organizers/{maker_id}/payout"),
            Some(&admin),
            None,
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "[$] no payout before KYC");
    sqlx::query("UPDATE users SET kyc_status = 'verified' WHERE id = $1::uuid")
        .bind(&maker_id)
        .execute(&pool)
        .await
        .unwrap();

    // Settling also requires the PROVIDER to say this organiser can receive money — our own KYC
    // verdict is no longer enough on its own (PAY-5).
    sqlx::query("UPDATE users SET psp_account_id = 'acct_org', psp_payouts_enabled = true WHERE id = $1::uuid")
        .bind(&maker_id)
        .execute(&pool)
        .await
        .unwrap();

    // Settle the payout: 100 less 10% commission = 90.00, one order, marked paid_out.
    let (st, payouts) = send(
        &app,
        build(
            "POST",
            &format!("/api/admin/organizers/{maker_id}/payout"),
            Some(&admin),
            None,
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let p = &payouts.as_array().unwrap()[0];
    assert_eq!(p["amount"], "90.00", "net of 10% platform commission");
    assert_eq!(p["currency"], "UAH");
    assert_eq!(p["order_count"], 1);
    assert_eq!(order_escrow(&app, &admin, &order).await, "paid_out");

    // Payout history lists it; a repeat settlement finds nothing to pay.
    let (_, list) = send(&app, build("GET", "/api/admin/payouts", Some(&admin), None)).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    let pid = p["id"].as_str().unwrap().to_string();

    // R5: the organizer sees their OWN settlement history (scoped to organizer_id = caller).
    let (st, mine) = send(&app, build("GET", "/api/organizer/payouts", Some(&maker), None)).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(mine.as_array().unwrap().len(), 1);
    assert_eq!(mine[0]["amount"], "90.00");

    // ADM-7 manual payout workflow: mark failed with a reason, then retry, then mark paid.
    let (st, _) = send(
        &app,
        build(
            "PATCH",
            &format!("/api/admin/payouts/{pid}"),
            Some(&admin),
            Some(json!({"status": "bogus"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "invalid payout status");
    let (st, _) = send(
        &app,
        build(
            "PATCH",
            "/api/admin/payouts/11111111-2222-3333-4444-555555555555",
            Some(&admin),
            Some(json!({"status": "paid"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND, "unknown payout");
    let (st, _) = send(
        &app,
        build(
            "PATCH",
            &format!("/api/admin/payouts/{pid}"),
            Some(&admin),
            Some(json!({"status": "failed", "failure_reason": "bank rejected"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    // Filter surfaces it under "failed" with the reason; "paid" filter excludes it.
    let (_, failed) = send(
        &app,
        build("GET", "/api/admin/payouts?status=failed", Some(&admin), None),
    )
    .await;
    assert_eq!(failed.as_array().unwrap().len(), 1);
    assert_eq!(failed[0]["failure_reason"], "bank rejected");
    let (_, paid_only) = send(&app, build("GET", "/api/admin/payouts?status=paid", Some(&admin), None)).await;
    assert!(paid_only.as_array().unwrap().is_empty());
    // Mark paid again — the non-failed branch clears the reason.
    let (st, _) = send(
        &app,
        build(
            "PATCH",
            &format!("/api/admin/payouts/{pid}"),
            Some(&admin),
            Some(json!({"status": "paid"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    let (_, after) = send(&app, build("GET", "/api/admin/payouts?status=paid", Some(&admin), None)).await;
    assert!(
        after[0]["failure_reason"].is_null(),
        "reason cleared on non-failed status"
    );
    let (_, again) = send(
        &app,
        build(
            "POST",
            &format!("/api/admin/organizers/{maker_id}/payout"),
            Some(&admin),
            None,
        ),
    )
    .await;
    assert!(again.as_array().unwrap().is_empty(), "nothing left to settle");
}

#[sqlx::test(migrations = "./migrations")]
async fn payout_blocked_for_blocked_organizer(pool: PgPool) {
    let app = make_app(pool.clone());
    let admin = make_admin(&app, &pool, "admin@x.co").await;
    let (_, maker_id) = register(&app, "m@x.co").await;
    // Even a KYC-verified organizer can't be paid once blocked/erased.
    sqlx::query("UPDATE users SET kyc_status = 'verified', blocked = true WHERE id = $1::uuid")
        .bind(&maker_id)
        .execute(&pool)
        .await
        .unwrap();
    let (st, _) = send(
        &app,
        build(
            "POST",
            &format!("/api/admin/organizers/{maker_id}/payout"),
            Some(&admin),
            None,
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "[$] blocked organizer can't be paid");
}

async fn newest_ticket(app: &Router, buyer: &str) -> String {
    let (_, t) = send(app, build("GET", "/api/me/tickets", Some(buyer), None)).await;
    t.as_array().unwrap()[0]["id"].as_str().unwrap().to_string()
}

#[sqlx::test(migrations = "./migrations")]
async fn return_refunds_during_clearing_but_not_after_cleared(pool: PgPool) {
    let app = make_app(pool.clone());
    let admin = make_admin(&app, &pool, "admin@x.co").await;
    let (maker, _) = register(&app, "m@x.co").await;
    let (buyer, _) = register(&app, "b@x.co").await;

    // A returned ticket whose escrow is still clearing must refund (no payout for it).
    let ev_a = paid_event(&app, &maker).await;
    let order_a = buy(&app, &buyer, &ev_a).await;
    let tid_a = newest_ticket(&app, &buyer).await;
    end_event_days_ago(&pool, &ev_a, 1).await;
    escrow::sweep(&pool, 5).await.unwrap(); // held → clearing
    let (st, _) = send(
        &app,
        build("POST", &format!("/api/tickets/{tid_a}/return"), Some(&buyer), None),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    // `refund_due`, not `refunded`: the ticket is returned and the money is QUEUED to go back
    // through the provider. It reads as refunded only once the provider confirms it moved
    // (PAY-11) — saying "refunded" before that told the buyer a lie.
    assert_eq!(order_escrow(&app, &admin, &order_a).await, "refund_due");

    // Once cleared, the refund window is over → return is rejected, escrow untouched.
    let ev_b = paid_event(&app, &maker).await;
    let order_b = buy(&app, &buyer, &ev_b).await;
    let tid_b = newest_ticket(&app, &buyer).await;
    end_event_days_ago(&pool, &ev_b, 20).await;
    escrow::sweep(&pool, 5).await.unwrap(); // → cleared
    let (st, _) = send(
        &app,
        build("POST", &format!("/api/tickets/{tid_b}/return"), Some(&buyer), None),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "refund window passed");
    assert_eq!(
        order_escrow(&app, &admin, &order_b).await,
        "cleared",
        "escrow unchanged"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn admin_verifies_organizer_and_profile_shows_badge(pool: PgPool) {
    let app = make_app(pool.clone());
    let admin = make_admin(&app, &pool, "admin@x.co").await;
    let (_, maker_id) = register(&app, "m@x.co").await;

    // Not verified by default.
    let (_, before) = send(&app, build("GET", &format!("/api/users/{maker_id}"), None, None)).await;
    assert_eq!(before["verified"], false);

    // Admin grants verification.
    let (st, patched) = send(
        &app,
        build(
            "PATCH",
            &format!("/api/admin/users/{maker_id}"),
            Some(&admin),
            Some(json!({ "verified": true })),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(patched["verified"], true);

    // Public profile now shows the badge.
    let (_, after) = send(&app, build("GET", &format!("/api/users/{maker_id}"), None, None)).await;
    assert_eq!(after["verified"], true);
}

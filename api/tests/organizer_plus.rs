//! Organizer+ (TASK-017 M8): promo codes, event drafts, sales analytics.

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

async fn paid_event(app: &Router, maker: &str) -> String {
    let (st, v) = send(
        app,
        build(
            "POST",
            "/api/events",
            Some(maker),
            Some(json!({"title":"Gig","category":"Rock","city":"Kyiv","price":"100","tickets_total":10,"cost_type":"paid"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    v["id"].as_str().unwrap().to_string()
}

async fn notifs(app: &Router, token: &str) -> Vec<Value> {
    let (_, v) = send(app, build("GET", "/api/me/notifications", Some(token), None)).await;
    v.as_array().unwrap().clone()
}

#[sqlx::test(migrations = "./migrations")]
async fn promo_codes_discount_checkout(pool: PgPool) {
    let app = make_app(pool);
    let (maker, _) = register(&app, "m@x.co").await;
    let (other, _) = register(&app, "o@x.co").await;
    let (buyer, _) = register(&app, "b@x.co").await;
    let event = paid_event(&app, &maker).await;

    // Non-owner can't create a promo → 403.
    let (st, _) = send(
        &app,
        build(
            "POST",
            &format!("/api/events/{event}/promos"),
            Some(&other),
            Some(json!({"code":"X","percent_off":10})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    // Bad percent → 422.
    let (st, _) = send(
        &app,
        build(
            "POST",
            &format!("/api/events/{event}/promos"),
            Some(&maker),
            Some(json!({"code":"X","percent_off":0})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    // Blank code and a zero max_uses are both rejected.
    for bad in [
        json!({"code":"  ","percent_off":10}),
        json!({"code":"Z","percent_off":10,"max_uses":0}),
    ] {
        let (st, _) = send(
            &app,
            build("POST", &format!("/api/events/{event}/promos"), Some(&maker), Some(bad)),
        )
        .await;
        assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    }
    // Owner creates a 20%-off code.
    let (st, p) = send(
        &app,
        build(
            "POST",
            &format!("/api/events/{event}/promos"),
            Some(&maker),
            Some(json!({"code":"SAVE20","percent_off":20,"max_uses":5})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    assert_eq!(p["percent_off"], 20);
    // The same code (case-insensitive) can't be created twice → 409.
    let (st, _) = send(
        &app,
        build(
            "POST",
            &format!("/api/events/{event}/promos"),
            Some(&maker),
            Some(json!({"code":"save20","percent_off":5})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT);

    // Buyer checks out with the code → 20% off (100 → 80.00). Checkout does NOT consume it.
    let (st, c) = send(
        &app,
        build(
            "POST",
            &format!("/api/events/{event}/checkout?promo=save20"),
            Some(&buyer),
            None,
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    assert_eq!(c["amount"], "80.00", "case-insensitive promo applied");
    let save20_order = c["order_id"].as_str().unwrap().to_string();

    let used = |list: &Value, code: &str| -> i64 {
        list.as_array().unwrap().iter().find(|p| p["code"] == code).unwrap()["used_count"]
            .as_i64()
            .unwrap()
    };
    let promos = |who: &str| build("GET", &format!("/api/events/{event}/promos"), Some(who), None);

    // Checkout alone leaves the code unconsumed — no denial-of-promo on an abandoned cart.
    let (_, list) = send(&app, promos(&maker)).await;
    assert_eq!(used(&list, "SAVE20"), 0, "checkout does not consume the code");

    // Paying the order consumes exactly one use.
    let (st, _) = send(
        &app,
        build(
            "POST",
            &format!("/api/orders/{save20_order}/pay-test"),
            Some(&buyer),
            None,
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    let (_, list) = send(&app, promos(&maker)).await;
    assert_eq!(used(&list, "SAVE20"), 1, "payment consumes the code");

    // An unknown code is rejected at checkout (no silent full charge).
    let (st, _) = send(
        &app,
        build(
            "POST",
            &format!("/api/events/{event}/checkout?promo=NOPE"),
            Some(&buyer),
            None,
        ),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);

    // Single-use code: BOTH buyers can check out (not yet consumed), but only the first
    // payment succeeds — max_uses is enforced atomically at payment, not at checkout.
    send(
        &app,
        build(
            "POST",
            &format!("/api/events/{event}/promos"),
            Some(&maker),
            Some(json!({"code":"ONCE","percent_off":50,"max_uses":1})),
        ),
    )
    .await;
    let (_, c1) = send(
        &app,
        build(
            "POST",
            &format!("/api/events/{event}/checkout?promo=ONCE"),
            Some(&buyer),
            None,
        ),
    )
    .await;
    let order1 = c1["order_id"].as_str().unwrap().to_string();
    let (_, c2) = send(
        &app,
        build(
            "POST",
            &format!("/api/events/{event}/checkout?promo=ONCE"),
            Some(&other),
            None,
        ),
    )
    .await;
    let order2 = c2["order_id"].as_str().unwrap().to_string();
    let (st, _) = send(
        &app,
        build("POST", &format!("/api/orders/{order1}/pay-test"), Some(&buyer), None),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "first payment consumes the single use");
    let (st, _) = send(
        &app,
        build("POST", &format!("/api/orders/{order2}/pay-test"), Some(&other), None),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "second payment rejected — code exhausted");

    // Owner sees ONCE counted once; then deletes SAVE20.
    let (_, list) = send(&app, promos(&maker)).await;
    assert_eq!(used(&list, "ONCE"), 1);
    let save20_id = list.as_array().unwrap().iter().find(|p| p["code"] == "SAVE20").unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let (st, _) = send(
        &app,
        build("DELETE", &format!("/api/promos/{save20_id}"), Some(&maker), None),
    )
    .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    // Deleting it again (now gone / not owned) → 404.
    let (st, _) = send(
        &app,
        build("DELETE", &format!("/api/promos/{save20_id}"), Some(&maker), None),
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);

    // A promo for a non-existent event → 404.
    let ghost = "00000000-0000-0000-0000-000000000000";
    let (st, _) = send(
        &app,
        build(
            "POST",
            &format!("/api/events/{ghost}/promos"),
            Some(&maker),
            Some(json!({"code":"G","percent_off":5})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "./migrations")]
async fn refund_returns_promo_use(pool: PgPool) {
    let app = make_app(pool.clone());
    let (maker, _) = register(&app, "m@x.co").await;
    let (buyer, _) = register(&app, "b@x.co").await;
    let (admin, _) = register(&app, "a@x.co").await;
    sqlx::query("UPDATE users SET role = 'ADMIN' WHERE email = 'a@x.co'")
        .execute(&pool)
        .await
        .unwrap();
    let event = paid_event(&app, &maker).await;
    send(
        &app,
        build(
            "POST",
            &format!("/api/events/{event}/promos"),
            Some(&maker),
            Some(json!({"code":"ONCE","percent_off":50,"max_uses":1})),
        ),
    )
    .await;

    // Buyer uses the single-use code and pays → consumed.
    let (_, c) = send(
        &app,
        build(
            "POST",
            &format!("/api/events/{event}/checkout?promo=ONCE"),
            Some(&buyer),
            None,
        ),
    )
    .await;
    let order = c["order_id"].as_str().unwrap().to_string();
    let (st, _) = send(
        &app,
        build("POST", &format!("/api/orders/{order}/pay-test"), Some(&buyer), None),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    let (_, list) = send(
        &app,
        build("GET", &format!("/api/events/{event}/promos"), Some(&maker), None),
    )
    .await;
    assert_eq!(list.as_array().unwrap()[0]["used_count"], 1);

    // Admin refunds the paid order → the promo use is given back.
    let (st, _) = send(
        &app,
        build(
            "POST",
            &format!("/api/admin/orders/{order}/escrow"),
            Some(&admin),
            Some(json!({"action":"refund"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let (_, list) = send(
        &app,
        build("GET", &format!("/api/events/{event}/promos"), Some(&maker), None),
    )
    .await;
    assert_eq!(
        list.as_array().unwrap()[0]["used_count"],
        0,
        "refund returns the promo use"
    );

    // The single-use code is now usable again.
    let (_, c) = send(
        &app,
        build(
            "POST",
            &format!("/api/events/{event}/checkout?promo=ONCE"),
            Some(&buyer),
            None,
        ),
    )
    .await;
    assert_eq!(c["amount"], "50.00", "code reusable after refund");
}

#[sqlx::test(migrations = "./migrations")]
async fn draft_is_hidden_until_published_then_announces(pool: PgPool) {
    let db = pool.clone();
    let app = make_app(pool);
    let (maker, maker_id) = register(&app, "m@x.co").await;
    let (follower, _) = register(&app, "f@x.co").await;
    send(
        &app,
        build("POST", &format!("/api/users/{maker_id}/follow"), Some(&follower), None),
    )
    .await;

    // Invalid status → 422.
    let (st, _) = send(
        &app,
        build("POST", "/api/events", Some(&maker), Some(json!({"title":"X","category":"Rock","price":"0","tickets_total":1,"cost_type":"free","status":"bogus"}))),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);

    // Create a draft.
    let (st, ev) = send(
        &app,
        build("POST", "/api/events", Some(&maker), Some(json!({"title":"Secret Gig","category":"Rock","city":"Kyiv","price":"50","tickets_total":5,"cost_type":"paid","status":"draft"}))),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    assert_eq!(ev["status"], "draft");
    let id = ev["id"].as_str().unwrap().to_string();

    // Hidden from the афіша + not checkout-able while a draft, and no announcement fired.
    let (_, afisha) = send(&app, build("GET", "/api/events", None, None)).await;
    assert!(
        !afisha.as_array().unwrap().iter().any(|e| e["id"] == id),
        "draft hidden"
    );
    let (st, _) = send(
        &app,
        build("POST", &format!("/api/events/{id}/checkout"), Some(&follower), None),
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND, "can't checkout a draft");
    assert!(notifs(&app, &follower).await.is_empty(), "no announcement for a draft");

    // Publishing a PAID event now requires the organiser to be able to receive the money
    // (PAY-5); this test is about announcements, so grant it and move on.
    sqlx::query(
        "UPDATE users SET psp_account_id = 'acct_' || id, psp_charges_enabled = true WHERE psp_account_id IS NULL",
    )
    .execute(&db)
    .await
    .unwrap();

    // Publishing announces it to the follower.
    let (st, pub_ev) = send(
        &app,
        build(
            "PUT",
            &format!("/api/events/{id}"),
            Some(&maker),
            Some(json!({"status":"published"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(pub_ev["status"], "published");
    let n = notifs(&app, &follower).await;
    assert!(
        n.iter().any(|x| x["kind"] == "organizer_event"),
        "publish announces, got {n:?}"
    );
    // Now it's in the афіша.
    let (_, afisha2) = send(&app, build("GET", "/api/events", None, None)).await;
    assert!(afisha2.as_array().unwrap().iter().any(|e| e["id"] == id));
}

#[sqlx::test(migrations = "./migrations")]
async fn organizer_analytics_daily_sales(pool: PgPool) {
    let app = make_app(pool);
    let (maker, _) = register(&app, "m@x.co").await;
    let (buyer, _) = register(&app, "b@x.co").await;
    let event = paid_event(&app, &maker).await;

    // Empty before any sales.
    let (st, empty) = send(&app, build("GET", "/api/organizer/analytics", Some(&maker), None)).await;
    assert_eq!(st, StatusCode::OK);
    assert!(empty.as_array().unwrap().is_empty());

    // A completed purchase shows up as one day with revenue.
    let (_, c) = send(
        &app,
        build("POST", &format!("/api/events/{event}/checkout"), Some(&buyer), None),
    )
    .await;
    let order = c["order_id"].as_str().unwrap();
    send(
        &app,
        build("POST", &format!("/api/orders/{order}/pay-test"), Some(&buyer), None),
    )
    .await;

    let (_, a) = send(&app, build("GET", "/api/organizer/analytics", Some(&maker), None)).await;
    let rows = a.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["tickets"], 1);
    assert_eq!(rows[0]["revenue"], "100.00");
}

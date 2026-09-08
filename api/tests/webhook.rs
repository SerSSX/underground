//! Lemon Squeezy webhook (TASK-017 M6): HMAC-SHA256 verification + order fulfillment.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use underground_api::routes::router;
use underground_api::state::AppState;
use underground_api::webhooks;

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

async fn make_admin(app: &Router, pool: &PgPool, email: &str) -> String {
    let token = register(app, email).await;
    sqlx::query("UPDATE users SET role = 'ADMIN' WHERE email = $1")
        .bind(email)
        .execute(pool)
        .await
        .unwrap();
    token
}

/// Configure the webhook secret via admin settings.
async fn set_webhook_secret(app: &Router, admin: &str, secret: &str) {
    let (st, _) = send(
        app,
        build(
            "PUT",
            "/api/admin/payment-settings",
            Some(admin),
            Some(json!({ "ls_webhook_secret": secret })),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
}

async fn paid_event(app: &Router, maker: &str) -> String {
    let (st, v) = send(
        app,
        build(
            "POST",
            "/api/events",
            Some(maker),
            Some(json!({"title":"Paid Gig","category":"Rock","city":"Kyiv","price":"100","tickets_total":5,"cost_type":"paid"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    v["id"].as_str().unwrap().to_string()
}

async fn checkout(app: &Router, buyer: &str, event_id: &str) -> String {
    let (st, v) = send(
        app,
        build("POST", &format!("/api/events/{event_id}/checkout"), Some(buyer), None),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    v["order_id"].as_str().unwrap().to_string()
}

/// POST a webhook with a real (or tampered) signature over the exact body bytes.
async fn post_webhook(app: &Router, payload: &Value, secret_for_sig: Option<&str>) -> StatusCode {
    let raw = payload.to_string();
    let mut b = Request::builder()
        .method("POST")
        .uri("/api/webhooks/lemonsqueezy")
        .header("content-type", "application/json");
    if let Some(secret) = secret_for_sig {
        b = b.header("x-signature", webhooks::sign(secret, raw.as_bytes()));
    }
    app.clone()
        .oneshot(b.body(Body::from(raw)).unwrap())
        .await
        .unwrap()
        .status()
}

/// [SEC] This test used to assert that a signed Lemon Squeezy `order_paid` FULFILLED the order
/// it named. That was the exploit the review found: `custom_data.order_id` is a URL parameter
/// the buyer edits in their own browser, so a signed LS webhook could fulfil ANY pending order
/// — pay 5 €, name the 500 € order. The contract is now the opposite: a delivery is applied
/// only to an order placed with THAT provider, on the object we created for it. No order is
/// ever placed with `lemonsqueezy` any more (PAY-1 retired it), so the route is an
/// authenticated, de-duplicated no-op — and this test proves it cannot mint.
#[sqlx::test(migrations = "./migrations")]
async fn ls_webhook_is_authenticated_recorded_and_cannot_fulfil_an_order_it_did_not_charge(pool: PgPool) {
    let app = make_app(pool.clone());
    let admin = make_admin(&app, &pool, "admin@x.co").await;
    let maker = register(&app, "m@x.co").await;
    let buyer = register(&app, "b@x.co").await;
    set_webhook_secret(&app, &admin, "whsec_test").await;
    let event = paid_event(&app, &maker).await;
    // A real pending order — placed with the `test` provider, as every order is today.
    let order = checkout(&app, &buyer, &event).await;

    let payload = json!({ "meta": { "event_name": "order_paid", "webhook_id": "wh_evt_1",
                                    "custom_data": { "order_id": order } } });

    // Tampered signature (wrong secret) → 401, nothing recorded.
    assert_eq!(
        post_webhook(&app, &payload, Some("wrong-secret")).await,
        StatusCode::UNAUTHORIZED
    );
    // Missing signature header → 401.
    assert_eq!(post_webhook(&app, &payload, None).await, StatusCode::UNAUTHORIZED);
    let recorded: i64 = sqlx::query_scalar("SELECT count(*) FROM webhook_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        recorded, 0,
        "a rejected delivery leaves no trace that could block a real one"
    );

    // Valid signature → 200 (acknowledged so LS stops retrying) — but the order it names was
    // not placed with lemonsqueezy, so NOTHING moves.
    assert_eq!(post_webhook(&app, &payload, Some("whsec_test")).await, StatusCode::OK);
    let (_, orders) = send(&app, build("GET", "/api/admin/orders", Some(&admin), None)).await;
    let o = orders.as_array().unwrap().iter().find(|o| o["id"] == order).unwrap();
    assert_eq!(
        o["status"], "pending",
        "a signed LS webhook must not fulfil a test-provider order"
    );
    let (_, tickets) = send(&app, build("GET", "/api/me/tickets", Some(&buyer), None)).await;
    assert_eq!(tickets.as_array().unwrap().len(), 0, "no ticket minted");

    // The delivery IS recorded, under its own delivery id — and a replay is a duplicate.
    let recorded: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM webhook_events WHERE provider = 'lemonsqueezy' AND event_id = 'wh_evt_1'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(recorded, 1);
    assert_eq!(post_webhook(&app, &payload, Some("whsec_test")).await, StatusCode::OK);
    let recorded: i64 = sqlx::query_scalar("SELECT count(*) FROM webhook_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(recorded, 1, "replay of the same delivery id is not a second row");
}

/// Lemon Squeezy is retired (PAY-1 / PAY-12). This test used to assert that live+lemonsqueezy
/// returned a hosted-checkout URL; the product no longer does that, and the honest contract is
/// a refusal BEFORE any order row exists — never a pending order nobody can pay.
#[sqlx::test(migrations = "./migrations")]
async fn checkout_with_retired_lemonsqueezy_is_refused_before_any_order(pool: PgPool) {
    let app = make_app(pool.clone());
    let admin = make_admin(&app, &pool, "admin@x.co").await;
    let maker = register(&app, "m@x.co").await;
    let buyer = register(&app, "b@x.co").await;
    let (st, _) = send(
        &app,
        build(
            "PUT",
            "/api/admin/payment-settings",
            Some(&admin),
            Some(json!({"mode":"live","provider":"lemonsqueezy","ls_store_id":"mystore","ls_variant_id":"var123","ls_api_key":"sk_live","ls_webhook_secret":"whsec_live"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let event = paid_event(&app, &maker).await;

    let (st, c) = send(
        &app,
        build("POST", &format!("/api/events/{event}/checkout"), Some(&buyer), None),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "retired provider → 409, got {c}");
    let orders: i64 = sqlx::query_scalar("SELECT count(*) FROM orders")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        orders, 0,
        "no order row is created for a provider that cannot take payment"
    );
    let left: i32 = sqlx::query_scalar("SELECT tickets_left FROM events WHERE id = $1::uuid")
        .bind(&event)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(left, 5, "and no seat is held");
}

#[sqlx::test(migrations = "./migrations")]
async fn webhook_edge_cases(pool: PgPool) {
    let app = make_app(pool.clone());
    let admin = make_admin(&app, &pool, "admin@x.co").await;

    // No secret configured yet → 409.
    let payload = json!({ "meta": { "event_name": "order_paid", "custom_data": { "order_id": "x" } } });
    assert_eq!(
        post_webhook(&app, &payload, Some("whsec_test")).await,
        StatusCode::CONFLICT
    );

    set_webhook_secret(&app, &admin, "whsec_test").await;

    // Non-hex signature → 401 (verify rejects bad hex).
    let raw = payload.to_string();
    let bad = Request::builder()
        .method("POST")
        .uri("/api/webhooks/lemonsqueezy")
        .header("x-signature", "not-hex!!")
        .body(Body::from(raw))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(bad).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );

    // Valid signature but unrelated event → 200 (acknowledged, ignored).
    let other = json!({ "meta": { "event_name": "subscription_created" } });
    assert_eq!(post_webhook(&app, &other, Some("whsec_test")).await, StatusCode::OK);

    // Valid signature, our event, but no order_id custom data → 200 (ignored).
    let no_id = json!({ "meta": { "event_name": "order_paid", "custom_data": {} } });
    assert_eq!(post_webhook(&app, &no_id, Some("whsec_test")).await, StatusCode::OK);

    // Valid signature over a non-JSON body → 422.
    let raw = "not json";
    let req = Request::builder()
        .method("POST")
        .uri("/api/webhooks/lemonsqueezy")
        .header("x-signature", webhooks::sign("whsec_test", raw.as_bytes()))
        .body(Body::from(raw))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

/// A delivery with NO signature header is a rejected delivery like any other: it is metered,
/// so an anonymous flood of header-less POSTs cannot cost a SELECT each for free (REVIEW-3 #8).
#[sqlx::test(migrations = "./migrations")]
async fn an_unsigned_ls_delivery_is_metered_like_a_bad_signature(pool: PgPool) {
    let app = make_app(pool.clone());
    let admin = make_admin(&app, &pool, "admin@x.co").await;
    set_webhook_secret(&app, &admin, "whsec_test").await;
    let payload = json!({"meta": {"event_name": "order_paid", "webhook_id": "wh_1"}});
    assert_eq!(post_webhook(&app, &payload, None).await, StatusCode::UNAUTHORIZED);
    let metered: i64 =
        sqlx::query_scalar("SELECT count(*) FROM rate_events WHERE bucket = 'webhook:lemonsqueezy:rejected'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(metered, 1, "a missing header costs the sender a token like a wrong one");
    // The sweep is what keeps an attacker-fed table bounded: age 0 drops everything older than
    // this instant (our one row); a day-old cut-off drops nothing more.
    assert_eq!(underground_api::ratelimit::sweep(&pool, 0).await.unwrap(), 1);
    assert_eq!(underground_api::ratelimit::sweep(&pool, 86_400).await.unwrap(), 0);
}

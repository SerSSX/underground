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

#[sqlx::test(migrations = "./migrations")]
async fn webhook_fulfills_order_on_valid_signature(pool: PgPool) {
    let app = make_app(pool.clone());
    let admin = make_admin(&app, &pool, "admin@x.co").await;
    let maker = register(&app, "m@x.co").await;
    let buyer = register(&app, "b@x.co").await;
    set_webhook_secret(&app, &admin, "whsec_test").await;
    let event = paid_event(&app, &maker).await;
    let order = checkout(&app, &buyer, &event).await;

    let payload = json!({ "meta": { "event_name": "order_paid", "custom_data": { "order_id": order } } });

    // Tampered signature (wrong secret) → 401.
    assert_eq!(
        post_webhook(&app, &payload, Some("wrong-secret")).await,
        StatusCode::UNAUTHORIZED
    );
    // Missing signature header → 401.
    assert_eq!(post_webhook(&app, &payload, None).await, StatusCode::UNAUTHORIZED);
    // Valid signature → 200, order fulfilled.
    assert_eq!(post_webhook(&app, &payload, Some("whsec_test")).await, StatusCode::OK);

    // The order is now paid + escrow held, and the buyer has a ticket.
    let (_, orders) = send(&app, build("GET", "/api/admin/orders", Some(&admin), None)).await;
    let o = orders.as_array().unwrap().iter().find(|o| o["id"] == order).unwrap();
    assert_eq!(o["status"], "paid");
    assert_eq!(o["escrow_status"], "held");
    let (_, tickets) = send(&app, build("GET", "/api/me/tickets", Some(&buyer), None)).await;
    assert_eq!(tickets.as_array().unwrap().len(), 1);

    // Duplicate delivery is idempotent (still 200, still one ticket).
    assert_eq!(post_webhook(&app, &payload, Some("whsec_test")).await, StatusCode::OK);
    let (_, tickets2) = send(&app, build("GET", "/api/me/tickets", Some(&buyer), None)).await;
    assert_eq!(tickets2.as_array().unwrap().len(), 1, "no double-mint");
}

#[sqlx::test(migrations = "./migrations")]
async fn checkout_live_returns_lemonsqueezy_url(pool: PgPool) {
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
    assert_eq!(st, StatusCode::CREATED);
    assert_eq!(c["test_mode"], false);
    let url = c["checkout_url"].as_str().expect("live checkout returns a URL");
    assert!(url.contains("mystore.lemonsqueezy.com/buy/var123"), "got {url}");
    assert!(
        url.contains(c["order_id"].as_str().unwrap()),
        "URL carries the order id"
    );
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

//! Integration tests for payment settings (admin) + test-mode checkout/pay flow.

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

async fn create_event(app: &Router, token: &str, body: Value) -> String {
    let resp = app
        .clone()
        .oneshot(build("POST", "/api/events", Some(token), Some(body)))
        .await
        .unwrap();
    json_body(resp).await["id"].as_str().unwrap().to_string()
}

fn paid_event() -> Value {
    json!({"title":"Paid Gig","category":"Rock","price":"100","currency":"UAH","tickets_total":3,"cost_type":"paid"})
}

#[sqlx::test(migrations = "./migrations")]
async fn admin_settings_guarded_and_masked(pool: PgPool) {
    let pool2 = pool.clone();
    let app = make_app(pool);
    let admin = token_for(&app, "admin@x.co").await;
    sqlx::query("UPDATE users SET role = 'ADMIN' WHERE email = 'admin@x.co'")
        .execute(&pool2)
        .await
        .unwrap();
    let user = token_for(&app, "user@x.co").await;

    let resp = app
        .clone()
        .oneshot(build("GET", "/api/admin/payment-settings", Some(&admin), None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = json_body(resp).await;
    assert_eq!(v["mode"], "test");
    assert_eq!(v["provider"], "test");
    assert_eq!(v["ls_api_key_set"], false);
    assert!(v.get("ls_api_key").is_none(), "[SEC] secret must not be returned");

    let resp = app
        .clone()
        .oneshot(build("GET", "/api/admin/payment-settings", Some(&user), None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN, "[SEC] non-admin → 403");

    let resp = app
        .oneshot(build("GET", "/api/admin/payment-settings", None, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "[SEC] no token → 401");
}

#[sqlx::test(migrations = "./migrations")]
async fn admin_updates_settings(pool: PgPool) {
    let pool2 = pool.clone();
    let app = make_app(pool);
    let admin = token_for(&app, "admin@x.co").await;
    sqlx::query("UPDATE users SET role = 'ADMIN' WHERE email = 'admin@x.co'")
        .execute(&pool2)
        .await
        .unwrap();

    let body = json!({"platform_fee_bps": 1500, "ls_api_key": "sk_secret", "ls_store_id": "store_1"});
    let resp = app
        .clone()
        .oneshot(build("PUT", "/api/admin/payment-settings", Some(&admin), Some(body)))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = json_body(resp).await;
    assert_eq!(v["platform_fee_bps"], 1500);
    assert_eq!(v["ls_store_id"], "store_1");
    assert_eq!(v["ls_api_key_set"], true);
    assert!(v.get("ls_api_key").is_none());

    let resp = app
        .oneshot(build(
            "PUT",
            "/api/admin/payment-settings",
            Some(&admin),
            Some(json!({"mode":"bogus"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "./migrations")]
async fn checkout_and_pay_test_mints_ticket(pool: PgPool) {
    let app = make_app(pool);
    let user = token_for(&app, "buyer@x.co").await;
    let event = create_event(&app, &user, paid_event()).await;

    let resp = app
        .clone()
        .oneshot(build(
            "POST",
            &format!("/api/events/{event}/checkout"),
            Some(&user),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let checkout = json_body(resp).await;
    assert_eq!(checkout["test_mode"], true);
    let order_id = checkout["order_id"].as_str().unwrap().to_string();

    let resp = app
        .clone()
        .oneshot(build(
            "POST",
            &format!("/api/orders/{order_id}/pay-test"),
            Some(&user),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    assert_eq!(json_body(resp).await["event_title"], "Paid Gig");

    let resp = app
        .clone()
        .oneshot(build("GET", "/api/me/tickets", Some(&user), None))
        .await
        .unwrap();
    assert_eq!(json_body(resp).await.as_array().unwrap().len(), 1);

    let free = create_event(
        &app,
        &user,
        json!({"title":"Free","category":"Poetry","price":"0","tickets_total":5,"cost_type":"free"}),
    )
    .await;
    let resp = app
        .oneshot(build(
            "POST",
            &format!("/api/events/{free}/checkout"),
            Some(&user),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT, "free event has no checkout");
}

#[sqlx::test(migrations = "./migrations")]
async fn pay_test_guards_owner_and_double_pay(pool: PgPool) {
    let app = make_app(pool);
    let owner = token_for(&app, "owner@x.co").await;
    let other = token_for(&app, "other@x.co").await;
    let event = create_event(&app, &owner, paid_event()).await;

    let resp = app
        .clone()
        .oneshot(build(
            "POST",
            &format!("/api/events/{event}/checkout"),
            Some(&owner),
            None,
        ))
        .await
        .unwrap();
    let order_id = json_body(resp).await["order_id"].as_str().unwrap().to_string();
    let pay = |token: &str| build("POST", &format!("/api/orders/{order_id}/pay-test"), Some(token), None);

    let resp = app.clone().oneshot(pay(&other)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "[SEC] not the buyer → 404");

    let resp = app.clone().oneshot(pay(&owner)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let resp = app.oneshot(pay(&owner)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT, "already processed → 409");
}

#[sqlx::test(migrations = "./migrations")]
async fn checkout_blocked_in_live_without_variant(pool: PgPool) {
    let app = make_app(pool.clone());
    let user = token_for(&app, "b@x.co").await;
    let event = create_event(&app, &user, paid_event()).await;
    // Force live mode directly (bypassing the settings guard) with no variant configured.
    sqlx::query("UPDATE payment_settings SET mode = 'live'")
        .execute(&pool)
        .await
        .unwrap();
    let resp = app
        .clone()
        .oneshot(build(
            "POST",
            &format!("/api/events/{event}/checkout"),
            Some(&user),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::CONFLICT,
        "live mode without a variant can't take payment"
    );
    // No orphan pending order was created.
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM orders")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "no orphan order left behind");
}

#[sqlx::test(migrations = "./migrations")]
async fn payment_readiness_reports_status(pool: PgPool) {
    let pool2 = pool.clone();
    let app = make_app(pool);
    let admin = token_for(&app, "admin@x.co").await;
    sqlx::query("UPDATE users SET role = 'ADMIN' WHERE email = 'admin@x.co'")
        .execute(&pool2)
        .await
        .unwrap();
    let user = token_for(&app, "u@x.co").await;

    // Non-admin → 403.
    let resp = app
        .clone()
        .oneshot(build("GET", "/api/admin/payment-readiness", Some(&user), None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN, "[SEC] readiness is admin-only");

    // Fresh install (test mode, no creds) → not ready for live.
    let resp = app
        .clone()
        .oneshot(build("GET", "/api/admin/payment-readiness", Some(&admin), None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = json_body(resp).await;
    assert_eq!(v["ready_for_live"], false);
    assert_eq!(v["ls_api_key_set"], false);

    // After a complete live configuration → ready.
    let resp = app
        .clone()
        .oneshot(build(
            "PUT",
            "/api/admin/payment-settings",
            Some(&admin),
            Some(json!({"mode":"live","provider":"lemonsqueezy","ls_store_id":"s","ls_variant_id":"v","ls_api_key":"k","ls_webhook_secret":"w"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = app
        .oneshot(build("GET", "/api/admin/payment-readiness", Some(&admin), None))
        .await
        .unwrap();
    let v = json_body(resp).await;
    assert_eq!(v["ready_for_live"], true);
    assert_eq!(v["ls_webhook_secret_set"], true);
}

/// Live mode needs a provider that moves real money. The check runs on the MERGED settings, so
/// flipping only the mode over a `test` provider is refused the same as sending both (REVIEW-3 #10).
#[sqlx::test(migrations = "./migrations")]
async fn live_mode_with_a_money_free_provider_is_refused_at_save(pool: PgPool) {
    let app = make_app(pool.clone());
    let admin = token_for(&app, "admin@x.co").await;
    sqlx::query("UPDATE users SET role = 'ADMIN' WHERE email = 'admin@x.co'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE payment_settings SET provider = 'test', mode = 'test' WHERE id = TRUE")
        .execute(&pool)
        .await
        .unwrap();
    for body in [json!({"mode":"live"}), json!({"mode":"live","provider":"noop"})] {
        let resp = app
            .clone()
            .oneshot(build(
                "PUT",
                "/api/admin/payment-settings",
                Some(&admin),
                Some(body.clone()),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    }
    let (mode, provider): (String, String) =
        sqlx::query_as("SELECT mode, provider FROM payment_settings WHERE id = TRUE")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        (mode.as_str(), provider.as_str()),
        ("test", "test"),
        "nothing was saved"
    );
}

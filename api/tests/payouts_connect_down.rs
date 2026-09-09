//! `POST /api/me/payouts/connect` when the payment provider is unreachable or unconfigured.
//!
//! Its own binary on purpose: it points `STRIPE_API_BASE` at a closed loopback port, and that
//! environment variable is process-wide — setting it beside other tests in one binary is a race,
//! not a test.

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

/// When the payment provider cannot be reached, the organiser gets one fixed, actionable line —
/// never the provider's transport error, which describes OUR infrastructure.
#[sqlx::test(migrations = "./migrations")]
async fn connect_reports_a_provider_failure_without_leaking_infrastructure(pool: PgPool) {
    let app = make_app(pool.clone());
    let org = register(&app, "org@x.co").await;

    // 1. No provider resolves at all (stripe selected, no key saved).
    sqlx::query(
        "UPDATE payment_settings SET provider = 'stripe', mode = 'test', stripe_secret_key = NULL WHERE id = TRUE",
    )
    .execute(&pool)
    .await
    .unwrap();
    let (st, v) = send(
        &app,
        build(
            "POST",
            "/api/me/payouts/connect",
            Some(&org),
            Some(json!({"country":"pl"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "{v}");
    let msg = v["error"].as_str().unwrap();
    assert!(
        !msg.contains("stripe") && !msg.contains("key"),
        "[SEC] no configuration detail leaks: {msg}"
    );

    // 2. The provider is configured but unreachable: the account cannot be created.
    std::env::set_var("STRIPE_API_BASE", "http://127.0.0.1:9");
    sqlx::query("UPDATE payment_settings SET stripe_secret_key = 'sk_test_x', stripe_webhook_secret = 'whsec_x' WHERE id = TRUE")
        .execute(&pool)
        .await
        .unwrap();
    let (st, v) = send(
        &app,
        build(
            "POST",
            "/api/me/payouts/connect",
            Some(&org),
            Some(json!({"country":"pl"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "{v}");
    let msg = v["error"].as_str().unwrap();
    assert!(msg.contains("акаунт"), "{msg}");
    assert!(!msg.contains("127.0.0.1"), "[SEC] no host leaks: {msg}");
    let stored: Option<String> = sqlx::query_scalar("SELECT psp_account_id FROM users WHERE email = 'org@x.co'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        stored.is_none(),
        "a failed create must not leave a half-written account"
    );

    // 3. The account exists, but the link cannot be made: a different, equally fixed line.
    sqlx::query("UPDATE users SET psp_account_id = 'acct_x' WHERE email = 'org@x.co'")
        .execute(&pool)
        .await
        .unwrap();
    let (st, v) = send(
        &app,
        build(
            "POST",
            "/api/me/payouts/connect",
            Some(&org),
            Some(json!({"country":"pl"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "{v}");
    assert!(v["error"].as_str().unwrap().contains("форму"), "{v}");
    std::env::remove_var("STRIPE_API_BASE");
}

//! `POST /api/admin/stripe/test` — the operator's "does my key work" button.
//!
//! Its own binary on purpose: it points `STRIPE_API_BASE` at a local mock and at an invalid
//! scheme, and that environment variable is process-wide.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use underground_api::routes::router;
use underground_api::state::AppState;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

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

async fn check(app: &Router, admin: &str) -> Value {
    let resp = app
        .clone()
        .oneshot(build("POST", "/api/admin/stripe/test", Some(admin), None))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "the check itself never fails; its verdict is in the body"
    );
    json_body(resp).await
}

#[sqlx::test(migrations = "./migrations")]
async fn the_connection_check_reports_no_key_a_bad_base_a_refusal_and_success(pool: PgPool) {
    let app = make_app(pool.clone());
    let resp = app
        .clone()
        .oneshot(build(
            "POST",
            "/api/auth/register",
            None,
            Some(json!({"email":"admin@x.co","password":"password1","name":"A"})),
        ))
        .await
        .unwrap();
    let admin = json_body(resp).await["token"].as_str().unwrap().to_string();
    sqlx::query("UPDATE users SET role = 'ADMIN' WHERE email = 'admin@x.co'")
        .execute(&pool)
        .await
        .unwrap();

    // 1. Nothing saved yet.
    let v = check(&app, &admin).await;
    assert_eq!(v["ok"], false);
    assert!(v["message"].as_str().unwrap().contains("не збережено"), "{v}");

    sqlx::query("UPDATE payment_settings SET stripe_secret_key = 'sk_test_x' WHERE id = TRUE")
        .execute(&pool)
        .await
        .unwrap();

    // 2. A misconfigured API base is reported, not panicked on and not silently swallowed.
    std::env::set_var("STRIPE_API_BASE", "ftp://stripe.example");
    let v = check(&app, &admin).await;
    assert_eq!(v["ok"], false);
    assert!(v["account_id"].is_null());

    // 3. Stripe refuses the key.
    let server = MockServer::start().await;
    std::env::set_var("STRIPE_API_BASE", server.uri());
    Mock::given(method("GET"))
        .and(path("/v1/account"))
        .and(header("authorization", "Bearer sk_test_x"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({"error":{"message":"Invalid API Key provided"}})))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    let v = check(&app, &admin).await;
    assert_eq!(v["ok"], false);
    assert!(v["message"].as_str().unwrap().contains("Invalid API Key"), "{v}");
    assert!(
        !v["message"].as_str().unwrap().contains("sk_test_x"),
        "[SEC] the key never comes back in a message"
    );

    // 4. Stripe answers with the account.
    server.reset().await;
    Mock::given(method("GET"))
        .and(path("/v1/account"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"acct_42"})))
        .mount(&server)
        .await;
    let v = check(&app, &admin).await;
    assert_eq!(v["ok"], true);
    assert_eq!(v["account_id"], "acct_42");
    std::env::remove_var("STRIPE_API_BASE");
}

//! What a buyer sees when the payment provider is unreachable: a fixed, actionable line — never
//! the provider's transport error, which describes OUR infrastructure.
//!
//! Its own binary on purpose: it points `STRIPE_API_BASE` at a closed loopback port, and that
//! environment variable is process-wide.

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
    let body = body.map(|v| Body::from(v.to_string())).unwrap_or_else(Body::empty);
    b.body(body).unwrap()
}

async fn json_body(resp: axum::response::Response) -> Value {
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn a_transport_failure_shows_the_buyer_a_fixed_line_and_frees_the_seat(pool: PgPool) {
    let app = make_app(pool.clone());
    let resp = app
        .clone()
        .oneshot(build(
            "POST",
            "/api/auth/register",
            None,
            Some(json!({"email":"b@x.co","password":"password1","name":"B"})),
        ))
        .await
        .unwrap();
    let token = json_body(resp).await["token"].as_str().unwrap().to_string();
    let resp = app
        .clone()
        .oneshot(build(
            "POST",
            "/api/events",
            Some(&token),
            Some(json!({"title":"Gig","category":"Rock","price":"100","currency":"UAH","tickets_total":2,"cost_type":"paid"})),
        ))
        .await
        .unwrap();
    let event = json_body(resp).await["id"].as_str().unwrap().to_string();

    // An earlier checkout of the same buyer, still pending, with a provider session to void.
    let resp = app
        .clone()
        .oneshot(build(
            "POST",
            &format!("/api/events/{event}/checkout"),
            Some(&token),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let old = json_body(resp).await["order_id"].as_str().unwrap().to_string();
    sqlx::query("UPDATE orders SET provider = 'stripe', provider_ref = 'cs_old' WHERE id = $1::uuid")
        .bind(&old)
        .execute(&pool)
        .await
        .unwrap();

    // Nothing listens here; the adapter gets a connection error (ProviderError::Transport) —
    // both for voiding the old session (best effort, logged) and for creating the new one.
    std::env::set_var("STRIPE_API_BASE", "http://127.0.0.1:9");
    sqlx::query(
        "UPDATE payment_settings SET provider = 'stripe', mode = 'test', \
            stripe_secret_key = 'sk_test_x', stripe_webhook_secret = 'whsec_x' WHERE id = TRUE",
    )
    .execute(&pool)
    .await
    .unwrap();
    let resp = app
        .clone()
        .oneshot(build(
            "POST",
            &format!("/api/events/{event}/checkout"),
            Some(&token),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let body = json_body(resp).await;
    let msg = body["error"].as_str().unwrap_or("");
    assert_eq!(msg, "платіжна система тимчасово недоступна, спробуйте за хвилину");
    assert!(
        !msg.contains("stripe") && !msg.contains("127.0.0.1"),
        "[SEC] no infrastructure detail leaks: {msg}"
    );
    // Let the spawned best-effort void run and fail (it is off the request path on purpose).
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Both seats are back: the superseded hold was handed back, and the failed checkout's
    // seat too. Neither order can be paid later.
    let left: i32 = sqlx::query_scalar("SELECT tickets_left FROM events WHERE id = $1::uuid")
        .bind(&event)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(left, 2);
    let (old_status, marker): (String, Option<String>) =
        sqlx::query_as("SELECT status, terminal_event_seen FROM orders WHERE id = $1::uuid")
            .bind(&old)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        (old_status.as_str(), marker.as_deref()),
        ("expired", Some("superseded"))
    );
    let (status,): (String,) = sqlx::query_as("SELECT status FROM orders ORDER BY created_at DESC LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "failed");
    std::env::remove_var("STRIPE_API_BASE");
}

//! Wiring, not unit: a real re-checkout through the router must make the provider EXPIRE the
//! superseded Checkout Session. `stripe_http` proves `cancel_charge` works when called; this
//! proves checkout calls it (REVIEW-4 #6).
//!
//! Its own binary on purpose: it points `STRIPE_API_BASE` at a local mock, and that environment
//! variable is process-wide.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use underground_api::routes::router;
use underground_api::state::AppState;
use wiremock::matchers::{method, path};
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

#[sqlx::test(migrations = "./migrations")]
async fn a_re_checkout_expires_the_superseded_session_at_the_provider(pool: PgPool) {
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

    // The first checkout, as Stripe would have left it: a pending order on session cs_old.
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

    let server = MockServer::start().await;
    std::env::set_var("STRIPE_API_BASE", server.uri());
    sqlx::query(
        "UPDATE payment_settings SET provider = 'stripe', mode = 'test', \
            stripe_secret_key = 'sk_test_x', stripe_webhook_secret = 'whsec_x' WHERE id = TRUE",
    )
    .execute(&pool)
    .await
    .unwrap();
    Mock::given(method("POST"))
        .and(path("/v1/checkout/sessions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "cs_new", "url": "https://checkout.stripe.com/c/pay/cs_new"
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/checkout/sessions/cs_old/expire"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "cs_old", "status": "expired"})))
        .mount(&server)
        .await;

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
    assert_eq!(
        json_body(resp).await["checkout_url"],
        "https://checkout.stripe.com/c/pay/cs_new"
    );
    // The void runs off the request path; give the detached task a moment.
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;

    let hits = server.received_requests().await.unwrap();
    let expired = hits
        .iter()
        .filter(|r| r.url.path() == "/v1/checkout/sessions/cs_old/expire")
        .count();
    assert_eq!(
        expired, 1,
        "checkout must reach the provider to void the superseded session: {hits:?}"
    );
    let (status, marker): (String, Option<String>) =
        sqlx::query_as("SELECT status, terminal_event_seen FROM orders WHERE id = $1::uuid")
            .bind(&old)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((status.as_str(), marker.as_deref()), ("expired", Some("superseded")));
    std::env::remove_var("STRIPE_API_BASE");
}

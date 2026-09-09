//! Integration tests for GDPR erasure + export (P1 #73).

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

#[sqlx::test(migrations = "./migrations")]
async fn export_returns_user_data(pool: PgPool) {
    let app = make_app(pool);
    let token = register(&app, "u@x.co").await;
    let (_, ev) = send(
        &app,
        build(
            "POST",
            "/api/events",
            Some(&token),
            Some(json!({"title":"Mine","category":"Poetry","price":"0","tickets_total":3,"cost_type":"free"})),
        ),
    )
    .await;
    let event = ev["id"].as_str().unwrap().to_string();
    let (st, _) = send(
        &app,
        build("POST", &format!("/api/events/{event}/reserve"), Some(&token), None),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);

    let (st, exp) = send(&app, build("GET", "/api/me/export", Some(&token), None)).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(exp["profile"]["email"], "u@x.co");
    assert_eq!(exp["events"].as_array().unwrap().len(), 1);
    assert_eq!(exp["tickets"].as_array().unwrap().len(), 1);
    assert!(exp["orders"].is_array());

    // Export needs auth.
    let (st, _) = send(&app, build("GET", "/api/me/export", None, None)).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn delete_me_anonymizes_and_frees_email(pool: PgPool) {
    let app = make_app(pool.clone());
    let token = register(&app, "gone@x.co").await;
    // We no longer hold anyone's bank details (PAY-5). What erasure must break is the LINK to
    // the organiser's account at the payment provider, and the capabilities it granted —
    // otherwise a deleted account can still sell tickets and be paid.
    sqlx::query(
        "UPDATE users SET psp_account_id = 'acct_gone', psp_charges_enabled = true, \
            psp_payouts_enabled = true, kyc_status = 'verified' WHERE email = 'gone@x.co'",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Wrong password → 401 (re-auth required).
    let (st, _) = send(
        &app,
        build("DELETE", "/api/me", Some(&token), Some(json!({"password":"nope"}))),
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);

    // Correct password → 204.
    let (st, _) = send(
        &app,
        build("DELETE", "/api/me", Some(&token), Some(json!({"password":"password1"}))),
    )
    .await;
    assert_eq!(st, StatusCode::NO_CONTENT);

    // The account is blocked: the old token no longer works.
    let (st, _) = send(&app, build("GET", "/api/me", Some(&token), None)).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED, "[SEC] erased account loses access");

    // All identifying / financial PII is gone. Bank details are no longer ours to clear — we do
    // not store them any more (PAY-5) — so what must go is the POINTER to the organiser's
    // account at the payment provider, and every capability it granted.
    type ErasedRow = (Option<String>, Option<String>, String, Option<String>, bool, bool);
    let row: ErasedRow = sqlx::query_as(
        "SELECT psp_account_id, google_sub, kyc_status, password_hash, psp_charges_enabled, psp_payouts_enabled \
         FROM users WHERE email = 'deleted+' || id || '@deleted.invalid' AND deleted_at IS NOT NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, None, "[GDPR] the link to the provider account is cleared");
    assert!(
        !row.4 && !row.5,
        "[GDPR] an erased organiser can no longer take money or be paid"
    );
    assert_eq!(row.1, None, "[GDPR] google_sub cleared");
    assert_eq!(row.2, "none", "[GDPR] kyc reset");
    assert_eq!(row.3, None, "[GDPR] password cleared");

    // Old credentials no longer authenticate.
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/auth/login",
            None,
            Some(json!({"email":"gone@x.co","password":"password1"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);

    // The email is freed (anonymized) — it can be registered again.
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/auth/register",
            None,
            Some(json!({"email":"gone@x.co","password":"password1","name":"New"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "erased email is reusable");
}

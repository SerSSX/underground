//! Integration tests for organizer KYC (P1 #78).

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
            Some(json!({"email":email,"password":"password1","name":"U"})),
        ),
    )
    .await;
    (
        v["token"].as_str().unwrap().to_string(),
        v["user"]["id"].as_str().unwrap().to_string(),
    )
}

#[sqlx::test(migrations = "./migrations")]
async fn kyc_submit_get_and_admin_decision(pool: PgPool) {
    let app = make_app(pool.clone());
    let (org, org_id) = register(&app, "org@x.co").await;
    let (admin, _) = register(&app, "admin@x.co").await;
    sqlx::query("UPDATE users SET role = 'ADMIN' WHERE email = 'admin@x.co'")
        .execute(&pool)
        .await
        .unwrap();

    // Fresh organizer: none, no bank details.
    let (st, v) = send(&app, build("GET", "/api/me/kyc", Some(&org), None)).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v["kyc_status"], "none");
    assert_eq!(v["has_bank_details"], false);

    // Validation: blank name, bad IBAN, bad SWIFT.
    for bad in [
        json!({"name":"  ","iban":"DE89370400440532013000","swift":"PBANUA2X"}),
        json!({"name":"Org Co","iban":"NOPE","swift":"PBANUA2X"}),
        json!({"name":"Org Co","iban":"DE89370400440532013000","swift":"SHORT"}),
    ] {
        let (st, _) = send(&app, build("POST", "/api/me/kyc", Some(&org), Some(bad))).await;
        assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    }

    // Valid submission → pending + bank details on file.
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/me/kyc",
            Some(&org),
            Some(json!({"name":"Org Co","iban":"DE89 3704 0044 0532 0130 00","swift":"pbanua2x"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    let (_, v) = send(&app, build("GET", "/api/me/kyc", Some(&org), None)).await;
    assert_eq!(v["kyc_status"], "pending");
    assert_eq!(v["has_bank_details"], true);

    // Admin decision: bad status → 422; verify → 204; unknown user → 404; non-admin → 403.
    let (st, _) = send(
        &app,
        build(
            "POST",
            &format!("/api/admin/users/{org_id}/kyc"),
            Some(&admin),
            Some(json!({"status":"maybe"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    let (st, _) = send(
        &app,
        build(
            "POST",
            &format!("/api/admin/users/{org_id}/kyc"),
            Some(&admin),
            Some(json!({"status":"verified"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    let (_, v) = send(&app, build("GET", "/api/me/kyc", Some(&org), None)).await;
    assert_eq!(v["kyc_status"], "verified");

    let ghost = "00000000-0000-0000-0000-000000000000";
    let (st, _) = send(
        &app,
        build(
            "POST",
            &format!("/api/admin/users/{ghost}/kyc"),
            Some(&admin),
            Some(json!({"status":"verified"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    let (st, _) = send(
        &app,
        build(
            "POST",
            &format!("/api/admin/users/{org_id}/kyc"),
            Some(&org),
            Some(json!({"status":"verified"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
}

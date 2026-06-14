//! Integration tests for abuse/content reporting (P1 #76).

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
async fn report_create_list_resolve(pool: PgPool) {
    let app = make_app(pool.clone());
    let (reporter, _) = register(&app, "r@x.co").await;
    let (admin, _) = register(&app, "a@x.co").await;
    sqlx::query("UPDATE users SET role = 'ADMIN' WHERE email = 'a@x.co'")
        .execute(&pool)
        .await
        .unwrap();
    let (_, target_id) = register(&app, "bad@x.co").await;

    // Validation: bad target_type → 422; blank reason → 422.
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/reports",
            Some(&reporter),
            Some(json!({"target_type":"alien","target_id":target_id,"reason":"x"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/reports",
            Some(&reporter),
            Some(json!({"target_type":"user","target_id":target_id,"reason":"   "})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);

    // Anonymous can't report → 401.
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/reports",
            None,
            Some(json!({"target_type":"user","target_id":target_id,"reason":"spam"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);

    // Valid report → 201.
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/reports",
            Some(&reporter),
            Some(json!({"target_type":"user","target_id":target_id,"reason":"harassment","details":"abusive DMs"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);

    // A report for a non-existent target → 404.
    let ghost = "00000000-0000-0000-0000-000000000000";
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/reports",
            Some(&reporter),
            Some(json!({"target_type":"user","target_id":ghost,"reason":"spam"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    // Over-long details → 422.
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/reports",
            Some(&reporter),
            Some(json!({"target_type":"user","target_id":target_id,"reason":"x","details":"z".repeat(1001)})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);

    // Non-admin can't read the queue → 403.
    let (st, _) = send(&app, build("GET", "/api/admin/reports", Some(&reporter), None)).await;
    assert_eq!(st, StatusCode::FORBIDDEN);

    // Admin sees the open report.
    let (st, list) = send(&app, build("GET", "/api/admin/reports?status=open", Some(&admin), None)).await;
    assert_eq!(st, StatusCode::OK);
    let rows = list.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["reason"], "harassment");
    assert_eq!(rows[0]["reporter_email"], "r@x.co");
    let report_id = rows[0]["id"].as_str().unwrap().to_string();

    // Resolve: bad status → 422; valid → 204; then it's no longer in the open queue.
    let (st, _) = send(
        &app,
        build(
            "PATCH",
            &format!("/api/admin/reports/{report_id}"),
            Some(&admin),
            Some(json!({"status":"bogus"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    let (st, _) = send(
        &app,
        build(
            "PATCH",
            &format!("/api/admin/reports/{report_id}"),
            Some(&admin),
            Some(json!({"status":"resolved"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    let (_, open) = send(&app, build("GET", "/api/admin/reports?status=open", Some(&admin), None)).await;
    assert!(
        open.as_array().unwrap().is_empty(),
        "resolved report leaves the open queue"
    );

    // Resolving a non-existent report → 404.
    let ghost = "00000000-0000-0000-0000-000000000000";
    let (st, _) = send(
        &app,
        build(
            "PATCH",
            &format!("/api/admin/reports/{ghost}"),
            Some(&admin),
            Some(json!({"status":"reviewed"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}

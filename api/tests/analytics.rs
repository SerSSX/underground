//! Integration tests for the analytics funnel + K-factor (TASK-117, UC-015).

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

fn step_count(funnel: &Value, name: &str) -> i64 {
    funnel["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == name)
        .map(|s| s["count"].as_i64().unwrap())
        .unwrap_or(-1)
}

#[sqlx::test(migrations = "./migrations")]
async fn funnel_counts_the_viral_loop_and_k_factor(pool: PgPool) {
    let db = pool.clone();
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let event = json_body(
        app.clone()
            .oneshot(build(
                "POST",
                "/api/events",
                Some(&owner),
                Some(json!({"title":"F","category":"Poetry","price":"0","tickets_total":3,"cost_type":"free"})),
            ))
            .await
            .unwrap(),
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let tid = json_body(
        app.clone()
            .oneshot(build(
                "POST",
                &format!("/api/events/{event}/reserve"),
                Some(&owner),
                None,
            ))
            .await
            .unwrap(),
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_string();

    // event_view
    app.clone()
        .oneshot(build("GET", &format!("/api/events/{event}"), None, None))
        .await
        .unwrap();
    // gift_created
    let gift = app
        .clone()
        .oneshot(build(
            "POST",
            &format!("/api/tickets/{tid}/gift"),
            Some(&owner),
            Some(json!({"email":"n@x.co"})),
        ))
        .await
        .unwrap();
    let claim_token = json_body(gift).await["claim_token"].as_str().unwrap().to_string();
    // ticket_claimed
    let newbie = token_for(&app, "n@x.co").await;
    app.clone()
        .oneshot(build(
            "POST",
            "/api/tickets/claim",
            Some(&newbie),
            Some(json!({ "token": claim_token })),
        ))
        .await
        .unwrap();

    // Promote an admin and read the funnel.
    let admin = token_for(&app, "admin@x.co").await;
    sqlx::query("UPDATE users SET role = 'ADMIN' WHERE email = 'admin@x.co'")
        .execute(&db)
        .await
        .unwrap();
    let resp = app
        .clone()
        .oneshot(build("GET", "/api/admin/analytics/funnel", Some(&admin), None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let f = json_body(resp).await;
    assert!(step_count(&f, "event_view") >= 1, "detail open tracked");
    assert_eq!(f["gift_created"], 1);
    assert_eq!(f["ticket_claimed"], 1);
    assert_eq!(f["k_factor"].as_f64().unwrap(), 1.0, "1 claim / 1 gift = K 1.0");
}

#[sqlx::test(migrations = "./migrations")]
async fn funnel_requires_admin(pool: PgPool) {
    let app = make_app(pool);
    let user = token_for(&app, "u@x.co").await;
    let resp = app
        .oneshot(build("GET", "/api/admin/analytics/funnel", Some(&user), None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN, "[SEC] funnel is admin-only");
}

#[sqlx::test(migrations = "./migrations")]
async fn funnel_k_factor_zero_without_gifts(pool: PgPool) {
    let db = pool.clone();
    let app = make_app(pool);
    let admin = token_for(&app, "admin@x.co").await;
    sqlx::query("UPDATE users SET role = 'ADMIN' WHERE email = 'admin@x.co'")
        .execute(&db)
        .await
        .unwrap();
    let f = json_body(
        app.oneshot(build("GET", "/api/admin/analytics/funnel", Some(&admin), None))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(f["gift_created"], 0);
    assert_eq!(f["k_factor"].as_f64().unwrap(), 0.0, "no gifts → K-factor is 0");
}

//! Integration tests for age-gating (P1 #77): organizers set a minimum-age rating;
//! underage / dob-unknown buyers are blocked at checkout and reserve.

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

async fn set_dob(app: &Router, token: &str, dob: &str) {
    let (st, _) = send(app, build("PATCH", "/api/me", Some(token), Some(json!({"dob": dob})))).await;
    assert_eq!(st, StatusCode::OK);
}

#[sqlx::test(migrations = "./migrations")]
async fn age_gating_blocks_underage_and_unknown(pool: PgPool) {
    let app = make_app(pool);
    let org = register(&app, "o@x.co").await;

    // Invalid age_rating → 422.
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/events",
            Some(&org),
            Some(json!({"title":"X","category":"Rock","price":"100","tickets_total":9,"cost_type":"paid","age_rating":"99"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);

    // 18+ paid event.
    let (st, ev) = send(
        &app,
        build(
            "POST",
            "/api/events",
            Some(&org),
            Some(json!({"title":"Adults Only","category":"Rock","price":"100","tickets_total":9,"cost_type":"paid","age_rating":"18"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    assert_eq!(ev["age_rating"], "18");
    let event = ev["id"].as_str().unwrap().to_string();

    let checkout = |t: &str| build("POST", &format!("/api/events/{event}/checkout"), Some(t), None);

    // Buyer with no dob → blocked.
    let nodob = register(&app, "nodob@x.co").await;
    let (st, _) = send(&app, checkout(&nodob)).await;
    assert_eq!(st, StatusCode::CONFLICT, "unknown age blocked");

    // Underage buyer → blocked.
    let minor = register(&app, "minor@x.co").await;
    set_dob(&app, &minor, "2015-06-15").await;
    let (st, _) = send(&app, checkout(&minor)).await;
    assert_eq!(st, StatusCode::CONFLICT, "minor blocked");

    // Adult buyer → allowed.
    let adult = register(&app, "adult@x.co").await;
    // A future date of birth is rejected.
    let (st, _) = send(
        &app,
        build("PATCH", "/api/me", Some(&adult), Some(json!({"dob":"2099-01-01"}))),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "future dob rejected");
    set_dob(&app, &adult, "1990-06-15").await;
    // dob is now exposed via /api/me so the client can pre-check age-gating.
    let (_, me) = send(&app, build("GET", "/api/me", Some(&adult), None)).await;
    assert_eq!(me["dob"], "1990-06-15", "dob exposed in /api/me");
    let (st, _) = send(&app, checkout(&adult)).await;
    assert_eq!(st, StatusCode::CREATED, "adult can buy");

    // Free 16+ event uses the same gate on reserve.
    let (_, free) = send(
        &app,
        build(
            "POST",
            "/api/events",
            Some(&org),
            Some(json!({"title":"Teens+","category":"Poetry","price":"0","tickets_total":9,"cost_type":"free","age_rating":"16"})),
        ),
    )
    .await;
    let free_id = free["id"].as_str().unwrap().to_string();
    let reserve = |t: &str| build("POST", &format!("/api/events/{free_id}/reserve"), Some(t), None);
    let (st, _) = send(&app, reserve(&minor)).await;
    assert_eq!(st, StatusCode::CONFLICT, "minor can't reserve 16+");
    let (st, t) = send(&app, reserve(&adult)).await;
    assert_eq!(st, StatusCode::CREATED, "adult can reserve 16+");
    let ticket_id = t["id"].as_str().unwrap().to_string();

    // Transfer can't bypass the age gate: gifting the 16+ ticket to the minor is rejected.
    let (st, _) = send(
        &app,
        build(
            "POST",
            &format!("/api/tickets/{ticket_id}/transfer"),
            Some(&adult),
            Some(json!({"email":"minor@x.co"})),
        ),
    )
    .await;
    assert_eq!(
        st,
        StatusCode::CONFLICT,
        "can't gift an age-restricted ticket to a minor"
    );

    // Organizer can change the rating via PUT (covers the update path).
    let (st, upd) = send(
        &app,
        build(
            "PUT",
            &format!("/api/events/{free_id}"),
            Some(&org),
            Some(json!({"age_rating":"18"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(upd["age_rating"], "18");
    // And an invalid rating on update is rejected too.
    let (st, _) = send(
        &app,
        build(
            "PUT",
            &format!("/api/events/{free_id}"),
            Some(&org),
            Some(json!({"age_rating":"7"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
}

//! Integration tests for public profiles + follows (F-11).

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

/// Registers a user; returns (token, user_id).
async fn register(app: &Router, email: &str, name: &str) -> (String, String) {
    let req = Request::post("/api/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"email": email, "password": "password1", "name": name}).to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    (
        v["token"].as_str().unwrap().to_string(),
        v["user"]["id"].as_str().unwrap().to_string(),
    )
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

fn get(path: &str, token: Option<&str>) -> Request<Body> {
    let mut b = Request::get(path);
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    b.body(Body::empty()).unwrap()
}

fn follow_req(method: &str, id: &str, token: Option<&str>) -> Request<Body> {
    let mut b = Request::builder().method(method).uri(format!("/api/users/{id}/follow"));
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    b.body(Body::empty()).unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn profile_is_public_and_hides_email(pool: PgPool) {
    let app = make_app(pool);
    // TASK-305 P: a name equal to the email's local part is shown as «Гість», so this account's email
    // differs from its name (setup only; the assertions are unchanged).
    let (_, maker_id) = register(&app, "maker.co@x.co", "Maker").await;

    let (status, body) = send(&app, get(&format!("/api/users/{maker_id}"), None)).await;
    assert_eq!(status, StatusCode::OK, "profile readable without auth");
    assert_eq!(body["name"], "Maker");
    assert_eq!(body["followers_count"], 0);
    assert_eq!(body["is_following"], false);
    assert!(
        body.get("email").is_none(),
        "[SEC] email never exposed on public profile"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn profile_404_for_unknown_user(pool: PgPool) {
    let app = make_app(pool);
    let (status, _) = send(&app, get("/api/users/00000000-0000-0000-0000-000000000000", None)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "./migrations")]
async fn follow_unfollow_lifecycle(pool: PgPool) {
    let app = make_app(pool);
    let (alice_token, alice_id) = register(&app, "alice@x.co", "Alice").await;
    let (_, maker_id) = register(&app, "maker@x.co", "Maker").await;

    // [SEC] follow requires auth
    let (status, _) = send(&app, follow_req("POST", &maker_id, None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = send(&app, follow_req("POST", &maker_id, Some(&alice_token))).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    // idempotent
    let (status, _) = send(&app, follow_req("POST", &maker_id, Some(&alice_token))).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (_, profile) = send(&app, get(&format!("/api/users/{maker_id}"), Some(&alice_token))).await;
    assert_eq!(profile["followers_count"], 1, "no double-count after repeat follow");
    assert_eq!(profile["is_following"], true);

    let (_, followers) = send(&app, get(&format!("/api/users/{maker_id}/followers"), None)).await;
    assert_eq!(followers[0]["id"], alice_id.as_str());
    let (_, following) = send(&app, get(&format!("/api/users/{alice_id}/following"), None)).await;
    assert_eq!(following[0]["id"], maker_id.as_str());

    let (status, _) = send(&app, follow_req("DELETE", &maker_id, Some(&alice_token))).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, profile) = send(&app, get(&format!("/api/users/{maker_id}"), Some(&alice_token))).await;
    assert_eq!(profile["followers_count"], 0);
    assert_eq!(profile["is_following"], false);
}

#[sqlx::test(migrations = "./migrations")]
async fn cannot_follow_self_or_ghost(pool: PgPool) {
    let app = make_app(pool);
    let (token, my_id) = register(&app, "a@x.co", "A").await;

    let (status, _) = send(&app, follow_req("POST", &my_id, Some(&token))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "self-follow rejected");

    let ghost = "00000000-0000-0000-0000-000000000000";
    let (status, _) = send(&app, follow_req("POST", ghost, Some(&token))).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unknown followee → 404");
}

#[sqlx::test(migrations = "./migrations")]
async fn user_events_lists_published_only(pool: PgPool) {
    let app = make_app(pool.clone());
    let (token, maker_id) = register(&app, "maker@x.co", "Maker").await;

    let create = |title: &str| {
        Request::post("/api/events")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                json!({"title": title, "category": "Rock", "price": "0", "tickets_total": 10}).to_string(),
            ))
            .unwrap()
    };
    send(&app, create("Visible")).await;
    let (_, hidden) = send(&app, create("Hidden later")).await;
    sqlx::query("UPDATE events SET status = 'hidden' WHERE id = $1::uuid")
        .bind(hidden["id"].as_str().unwrap())
        .execute(&pool)
        .await
        .unwrap();

    let (status, body) = send(&app, get(&format!("/api/users/{maker_id}/events"), None)).await;
    assert_eq!(status, StatusCode::OK);
    let titles: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, vec!["Visible"], "hidden events excluded from public profile");

    let (_, profile) = send(&app, get(&format!("/api/users/{maker_id}"), None)).await;
    assert_eq!(profile["events_count"], 1, "counter matches published events");
}

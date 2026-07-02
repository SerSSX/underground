//! Integration tests for the gift/claim viral loop (TASK-110, WF-GiftClaim).
//! Headline: a ticket can be gifted to an email with NO account, then claimed after signup.

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
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(t) = token {
        builder = builder.header("authorization", format!("Bearer {t}"));
    }
    let body = body.map(|v| Body::from(v.to_string())).unwrap_or_else(Body::empty);
    builder.body(body).unwrap()
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

async fn free_event_id(app: &Router, token: &str) -> String {
    let body = json!({"title":"Free","category":"Poetry","price":"0","tickets_total":3,"cost_type":"free"});
    let resp = app
        .clone()
        .oneshot(build("POST", "/api/events", Some(token), Some(body)))
        .await
        .unwrap();
    json_body(resp).await["id"].as_str().unwrap().to_string()
}

async fn reserve_ticket_id(app: &Router, token: &str, event: &str) -> String {
    let resp = app
        .clone()
        .oneshot(build(
            "POST",
            &format!("/api/events/{event}/reserve"),
            Some(token),
            None,
        ))
        .await
        .unwrap();
    json_body(resp).await["id"].as_str().unwrap().to_string()
}

async fn gift(app: &Router, token: &str, tid: &str, email: &str) -> axum::response::Response {
    app.clone()
        .oneshot(build(
            "POST",
            &format!("/api/tickets/{tid}/gift"),
            Some(token),
            Some(json!({ "email": email })),
        ))
        .await
        .unwrap()
}

async fn my_tickets_len(app: &Router, token: &str) -> usize {
    let resp = app
        .clone()
        .oneshot(build("GET", "/api/me/tickets", Some(token), None))
        .await
        .unwrap();
    json_body(resp).await.as_array().unwrap().len()
}

#[sqlx::test(migrations = "./migrations")]
async fn gift_to_unregistered_email_then_claim(pool: PgPool) {
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let event = free_event_id(&app, &owner).await;
    let tid = reserve_ticket_id(&app, &owner, &event).await;

    // Gift to an email that has NO account yet.
    let resp = gift(&app, &owner, &tid, "newbie@x.co").await;
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "gift to unregistered email succeeds"
    );
    let claim_token = json_body(resp).await["claim_token"].as_str().unwrap().to_string();
    assert!(!claim_token.is_empty());

    // The recipient signs up, then claims with the token.
    let newbie = token_for(&app, "newbie@x.co").await;
    let resp = app
        .clone()
        .oneshot(build(
            "POST",
            "/api/tickets/claim",
            Some(&newbie),
            Some(json!({ "token": claim_token })),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "claim succeeds");
    assert_eq!(json_body(resp).await["status"], "active", "claimed ticket is active");

    assert_eq!(my_tickets_len(&app, &owner).await, 0, "owner no longer holds it");
    assert_eq!(my_tickets_len(&app, &newbie).await, 1, "claimer now holds it");
}

#[sqlx::test(migrations = "./migrations")]
async fn gift_locks_ticket_and_blocks_double_gift(pool: PgPool) {
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let event = free_event_id(&app, &owner).await;
    let tid = reserve_ticket_id(&app, &owner, &event).await;

    assert_eq!(gift(&app, &owner, &tid, "a@x.co").await.status(), StatusCode::CREATED);

    let resp = app
        .clone()
        .oneshot(build("GET", "/api/me/tickets", Some(&owner), None))
        .await
        .unwrap();
    let tickets = json_body(resp).await;
    assert_eq!(
        tickets[0]["status"], "pending_claim",
        "gifted ticket is locked pending claim"
    );

    assert_eq!(
        gift(&app, &owner, &tid, "b@x.co").await.status(),
        StatusCode::CONFLICT,
        "cannot gift the same ticket twice while outstanding"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn claim_rejects_invalid_token(pool: PgPool) {
    let app = make_app(pool);
    let u = token_for(&app, "u@x.co").await;
    let resp = app
        .oneshot(build(
            "POST",
            "/api/tickets/claim",
            Some(&u),
            Some(json!({ "token": "deadbeef" })),
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "[SEC] unknown token is not claimable"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn claim_token_is_single_use(pool: PgPool) {
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let event = free_event_id(&app, &owner).await;
    let tid = reserve_ticket_id(&app, &owner, &event).await;
    let claim_token = json_body(gift(&app, &owner, &tid, "a@x.co").await).await["claim_token"]
        .as_str()
        .unwrap()
        .to_string();

    let first = token_for(&app, "first@x.co").await;
    let resp = app
        .clone()
        .oneshot(build(
            "POST",
            "/api/tickets/claim",
            Some(&first),
            Some(json!({ "token": claim_token.clone() })),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let second = token_for(&app, "second@x.co").await;
    let resp = app
        .oneshot(build(
            "POST",
            "/api/tickets/claim",
            Some(&second),
            Some(json!({ "token": claim_token })),
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "[SEC] a claimed token cannot be reused"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn gift_requires_owner(pool: PgPool) {
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let other = token_for(&app, "x@x.co").await;
    let event = free_event_id(&app, &owner).await;
    let tid = reserve_ticket_id(&app, &owner, &event).await;

    assert_eq!(
        gift(&app, &other, &tid, "a@x.co").await.status(),
        StatusCode::NOT_FOUND,
        "[SEC] only the owner can gift a ticket"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn gift_empty_email_rejected(pool: PgPool) {
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let event = free_event_id(&app, &owner).await;
    let tid = reserve_ticket_id(&app, &owner, &event).await;
    assert_eq!(
        gift(&app, &owner, &tid, "   ").await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn gift_returned_ticket_conflicts(pool: PgPool) {
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let event = free_event_id(&app, &owner).await;
    let tid = reserve_ticket_id(&app, &owner, &event).await;
    app.clone()
        .oneshot(build("POST", &format!("/api/tickets/{tid}/return"), Some(&owner), None))
        .await
        .unwrap();
    assert_eq!(gift(&app, &owner, &tid, "a@x.co").await.status(), StatusCode::CONFLICT);
}

#[sqlx::test(migrations = "./migrations")]
async fn regift_after_expiry_succeeds(pool: PgPool) {
    let db = pool.clone();
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let event = free_event_id(&app, &owner).await;
    let tid = reserve_ticket_id(&app, &owner, &event).await;
    assert_eq!(gift(&app, &owner, &tid, "a@x.co").await.status(), StatusCode::CREATED);
    sqlx::query("UPDATE ticket_gifts SET expires_at = now() - interval '1 hour'")
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        gift(&app, &owner, &tid, "b@x.co").await.status(),
        StatusCode::CREATED,
        "an expired gift can be re-gifted"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn claim_expired_gift_reverts_ticket(pool: PgPool) {
    let db = pool.clone();
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let event = free_event_id(&app, &owner).await;
    let tid = reserve_ticket_id(&app, &owner, &event).await;
    let token = json_body(gift(&app, &owner, &tid, "a@x.co").await).await["claim_token"]
        .as_str()
        .unwrap()
        .to_string();
    sqlx::query("UPDATE ticket_gifts SET expires_at = now() - interval '1 hour'")
        .execute(&db)
        .await
        .unwrap();
    let newbie = token_for(&app, "a@x.co").await;
    let resp = app
        .clone()
        .oneshot(build(
            "POST",
            "/api/tickets/claim",
            Some(&newbie),
            Some(json!({ "token": token })),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT, "an expired gift can't be claimed");
    let mine = app
        .oneshot(build("GET", "/api/me/tickets", Some(&owner), None))
        .await
        .unwrap();
    assert_eq!(
        json_body(mine).await[0]["status"],
        "active",
        "ticket reverts to the gifter"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn claim_rejects_underage(pool: PgPool) {
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    // The gifter is an adult so they can reserve the 18+ ticket.
    app.clone()
        .oneshot(build(
            "PATCH",
            "/api/me",
            Some(&owner),
            Some(json!({ "dob": "1990-01-01" })),
        ))
        .await
        .unwrap();
    let event = json_body(
        app.clone()
            .oneshot(build(
                "POST",
                "/api/events",
                Some(&owner),
                Some(json!({"title":"18+","category":"Party","price":"0","tickets_total":3,"cost_type":"free","age_rating":"18"})),
            ))
            .await
            .unwrap(),
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let tid = reserve_ticket_id(&app, &owner, &event).await;
    let token = json_body(gift(&app, &owner, &tid, "kid@x.co").await).await["claim_token"]
        .as_str()
        .unwrap()
        .to_string();
    let kid = token_for(&app, "kid@x.co").await; // no dob
    let resp = app
        .oneshot(build(
            "POST",
            "/api/tickets/claim",
            Some(&kid),
            Some(json!({ "token": token })),
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::CONFLICT,
        "underage claimer blocked by the age gate"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn gift_frozen_ticket_conflicts(pool: PgPool) {
    let app = make_app(pool);
    let buyer = token_for(&app, "b@x.co").await;
    let event = json_body(
        app.clone()
            .oneshot(build(
                "POST",
                "/api/events",
                Some(&buyer),
                Some(json!({"title":"Paid","category":"Rock","price":"100","currency":"UAH","tickets_total":3,"cost_type":"paid"})),
            ))
            .await
            .unwrap(),
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let order_id = json_body(
        app.clone()
            .oneshot(build(
                "POST",
                &format!("/api/events/{event}/checkout"),
                Some(&buyer),
                None,
            ))
            .await
            .unwrap(),
    )
    .await["order_id"]
        .as_str()
        .unwrap()
        .to_string();
    let tid = json_body(
        app.clone()
            .oneshot(build(
                "POST",
                &format!("/api/orders/{order_id}/pay-test"),
                Some(&buyer),
                None,
            ))
            .await
            .unwrap(),
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_string();
    // Buyer disputes → escrow frozen → the ticket can't be gifted.
    let d = app
        .clone()
        .oneshot(build(
            "POST",
            &format!("/api/orders/{order_id}/dispute"),
            Some(&buyer),
            None,
        ))
        .await
        .unwrap();
    assert!(d.status().is_success(), "dispute freezes escrow");
    assert_eq!(
        gift(&app, &buyer, &tid, "a@x.co").await.status(),
        StatusCode::CONFLICT,
        "a disputed ticket can't be gifted"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn gift_preview_public_then_404_after_claim(pool: PgPool) {
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let event = free_event_id(&app, &owner).await;
    let tid = reserve_ticket_id(&app, &owner, &event).await;
    let token = json_body(gift(&app, &owner, &tid, "n@x.co").await).await["claim_token"]
        .as_str()
        .unwrap()
        .to_string();

    // Public preview (no auth) shows the event + gifter.
    let resp = app
        .clone()
        .oneshot(build("GET", &format!("/api/gifts/{token}"), None, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = json_body(resp).await;
    assert_eq!(v["event_title"], "Free");
    assert!(v["from_name"].is_string());

    // Unknown token → 404 (reveals nothing).
    let bad = app
        .clone()
        .oneshot(build("GET", "/api/gifts/nope", None, None))
        .await
        .unwrap();
    assert_eq!(bad.status(), StatusCode::NOT_FOUND);

    // Once claimed, the preview is gone.
    let newbie = token_for(&app, "n@x.co").await;
    app.clone()
        .oneshot(build(
            "POST",
            "/api/tickets/claim",
            Some(&newbie),
            Some(json!({ "token": token.clone() })),
        ))
        .await
        .unwrap();
    let after = app
        .oneshot(build("GET", &format!("/api/gifts/{token}"), None, None))
        .await
        .unwrap();
    assert_eq!(
        after.status(),
        StatusCode::NOT_FOUND,
        "a claimed gift is no longer previewable"
    );
}

async fn revoke(app: &Router, token: &str, tid: &str) -> axum::response::Response {
    app.clone()
        .oneshot(build("DELETE", &format!("/api/tickets/{tid}/gift"), Some(token), None))
        .await
        .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn revoke_reclaims_pending_gift(pool: PgPool) {
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let event = free_event_id(&app, &owner).await;
    let tid = reserve_ticket_id(&app, &owner, &event).await;
    let token = json_body(gift(&app, &owner, &tid, "a@x.co").await).await["claim_token"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = revoke(&app, &owner, &tid).await;
    assert_eq!(resp.status(), StatusCode::OK, "owner revokes the pending gift");
    assert_eq!(
        json_body(resp).await["status"],
        "active",
        "the ticket reverts to active"
    );
    assert_eq!(my_tickets_len(&app, &owner).await, 1, "the owner still holds it");

    // The outstanding claim token is now dead.
    let recipient = token_for(&app, "a@x.co").await;
    let claim = app
        .oneshot(build(
            "POST",
            "/api/tickets/claim",
            Some(&recipient),
            Some(json!({ "token": token })),
        ))
        .await
        .unwrap();
    assert_eq!(
        claim.status(),
        StatusCode::NOT_FOUND,
        "[SEC] a revoked gift token can no longer be claimed"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn revoke_requires_owner(pool: PgPool) {
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let other = token_for(&app, "x@x.co").await;
    let event = free_event_id(&app, &owner).await;
    let tid = reserve_ticket_id(&app, &owner, &event).await;
    gift(&app, &owner, &tid, "a@x.co").await;
    assert_eq!(
        revoke(&app, &other, &tid).await.status(),
        StatusCode::NOT_FOUND,
        "[SEC] only the owner can revoke a gift"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn revoke_without_pending_gift_conflicts(pool: PgPool) {
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let event = free_event_id(&app, &owner).await;
    let tid = reserve_ticket_id(&app, &owner, &event).await;
    assert_eq!(
        revoke(&app, &owner, &tid).await.status(),
        StatusCode::CONFLICT,
        "there is nothing to revoke on an active ticket"
    );
}

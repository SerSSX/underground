//! Integration tests for delegated door staff + event-scoped scan (TASK-118, UC-014).

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

async fn free_event(app: &Router, token: &str) -> String {
    let body = json!({"title":"Gig","category":"Poetry","price":"0","tickets_total":5,"cost_type":"free"});
    json_body(
        app.clone()
            .oneshot(build("POST", "/api/events", Some(token), Some(body)))
            .await
            .unwrap(),
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// Reserve a free ticket as `token`; return its QR token.
async fn reserve_qr(app: &Router, token: &str, event: &str) -> String {
    let t = json_body(
        app.clone()
            .oneshot(build(
                "POST",
                &format!("/api/events/{event}/reserve"),
                Some(token),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    t["qr_token"].as_str().unwrap().to_string()
}

async fn scan(app: &Router, token: &str, event: &str, qr: &str) -> axum::response::Response {
    app.clone()
        .oneshot(build(
            "POST",
            &format!("/api/events/{event}/scan"),
            Some(token),
            Some(json!({ "qr_token": qr })),
        ))
        .await
        .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn delegated_staff_scans_then_double_scan_is_used(pool: PgPool) {
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let attendee = token_for(&app, "a@x.co").await;
    let controller = token_for(&app, "c@x.co").await;
    let event = free_event(&app, &owner).await;
    let qr = reserve_signed_qr(&app, &attendee, &event).await;

    // Owner invites a controller; the controller accepts.
    let invite = json_body(
        app.clone()
            .oneshot(build(
                "POST",
                &format!("/api/events/{event}/staff/invite"),
                Some(&owner),
                None,
            ))
            .await
            .unwrap(),
    )
    .await["invite_token"]
        .as_str()
        .unwrap()
        .to_string();
    let accept = app
        .clone()
        .oneshot(build(
            "POST",
            "/api/staff/accept",
            Some(&controller),
            Some(json!({ "token": invite })),
        ))
        .await
        .unwrap();
    // TASK-302 CT12: accept answers 200 {"event_id"} (was 204) so the client can open the scanner.
    assert_eq!(accept.status(), StatusCode::OK);

    // The controller scans the attendee's ticket: valid, then already_used.
    let first = scan(&app, &controller, &event, &qr).await;
    assert_eq!(first.status(), StatusCode::OK);
    let f = json_body(first).await;
    assert_eq!(f["valid"], true);
    assert_eq!(f["reason"], "valid");

    let second = json_body(scan(&app, &controller, &event, &qr).await).await;
    assert_eq!(second["valid"], false);
    assert_eq!(second["reason"], "already_used");
}

#[sqlx::test(migrations = "./migrations")]
async fn stranger_cannot_scan(pool: PgPool) {
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let attendee = token_for(&app, "a@x.co").await;
    let stranger = token_for(&app, "s@x.co").await;
    let event = free_event(&app, &owner).await;
    let qr = reserve_qr(&app, &attendee, &event).await;

    let resp = scan(&app, &stranger, &event, &qr).await;
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "[SEC] non-owner, non-staff cannot scan"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn scan_reports_wrong_event(pool: PgPool) {
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let attendee = token_for(&app, "a@x.co").await;
    let event_a = free_event(&app, &owner).await;
    let event_b = free_event(&app, &owner).await;
    let qr_b = reserve_signed_qr(&app, &attendee, &event_b).await;

    // Owner scans a ticket for event B at event A's door.
    let resp = json_body(scan(&app, &owner, &event_a, &qr_b).await).await;
    assert_eq!(resp["valid"], false);
    assert_eq!(resp["reason"], "wrong_event");
}

#[sqlx::test(migrations = "./migrations")]
async fn invite_requires_owner(pool: PgPool) {
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let other = token_for(&app, "x@x.co").await;
    let event = free_event(&app, &owner).await;

    let resp = app
        .oneshot(build(
            "POST",
            &format!("/api/events/{event}/staff/invite"),
            Some(&other),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "[SEC] only the owner can invite staff"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn invite_nonexistent_event_is_404(pool: PgPool) {
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let resp = app
        .oneshot(build(
            "POST",
            "/api/events/00000000-0000-0000-0000-000000000000/staff/invite",
            Some(&owner),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "./migrations")]
async fn accept_expired_invite_conflicts(pool: PgPool) {
    let db = pool.clone();
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let controller = token_for(&app, "c@x.co").await;
    let event = free_event(&app, &owner).await;
    let invite = json_body(
        app.clone()
            .oneshot(build(
                "POST",
                &format!("/api/events/{event}/staff/invite"),
                Some(&owner),
                None,
            ))
            .await
            .unwrap(),
    )
    .await["invite_token"]
        .as_str()
        .unwrap()
        .to_string();
    sqlx::query("UPDATE staff_invites SET expires_at = now() - interval '1 hour'")
        .execute(&db)
        .await
        .unwrap();
    let resp = app
        .oneshot(build(
            "POST",
            "/api/staff/accept",
            Some(&controller),
            Some(json!({ "token": invite })),
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::CONFLICT,
        "an expired invite can't be accepted"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn scan_unknown_qr_is_not_found_verdict(pool: PgPool) {
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let event = free_event(&app, &owner).await;
    let resp = json_body(scan(&app, &owner, &event, "UG-nope").await).await;
    assert_eq!(resp["valid"], false);
    assert_eq!(resp["reason"], "not_found");
}

#[sqlx::test(migrations = "./migrations")]
async fn scan_returned_and_pending_verdicts(pool: PgPool) {
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let attendee = token_for(&app, "a@x.co").await;
    let event = free_event(&app, &owner).await;

    // Returned ticket → "returned". Mint the signed QR while active, then return.
    let t = json_body(
        app.clone()
            .oneshot(build(
                "POST",
                &format!("/api/events/{event}/reserve"),
                Some(&attendee),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    let tid = t["id"].as_str().unwrap().to_string();
    let qr = signed_qr_for(&app, &attendee, &tid).await;
    app.clone()
        .oneshot(build(
            "POST",
            &format!("/api/tickets/{tid}/return"),
            Some(&attendee),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(
        json_body(scan(&app, &owner, &event, &qr).await).await["reason"],
        "returned"
    );

    // A gifted (pending_claim) ticket → "inactive".
    let t2 = json_body(
        app.clone()
            .oneshot(build(
                "POST",
                &format!("/api/events/{event}/reserve"),
                Some(&attendee),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    let tid2 = t2["id"].as_str().unwrap().to_string();
    let qr2 = signed_qr_for(&app, &attendee, &tid2).await;
    app.clone()
        .oneshot(build(
            "POST",
            &format!("/api/tickets/{tid2}/gift"),
            Some(&attendee),
            Some(json!({"email":"z@x.co"})),
        ))
        .await
        .unwrap();
    assert_eq!(
        json_body(scan(&app, &owner, &event, &qr2).await).await["reason"],
        "inactive"
    );
}

async fn staff_events(app: &Router, token: &str) -> Value {
    json_body(
        app.clone()
            .oneshot(build("GET", "/api/me/staff/events", Some(token), None))
            .await
            .unwrap(),
    )
    .await
}

/// Mint a signed door-QR for an existing (active) ticket (BR-023).
async fn signed_qr_for(app: &Router, token: &str, tid: &str) -> String {
    json_body(
        app.clone()
            .oneshot(build("GET", &format!("/api/tickets/{tid}/qr"), Some(token), None))
            .await
            .unwrap(),
    )
    .await["token"]
        .as_str()
        .unwrap()
        .to_string()
}

/// Reserve a ticket and mint a signed door-QR for it (BR-023).
async fn reserve_signed_qr(app: &Router, token: &str, event: &str) -> String {
    let t = json_body(
        app.clone()
            .oneshot(build(
                "POST",
                &format!("/api/events/{event}/reserve"),
                Some(token),
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    signed_qr_for(app, token, t["id"].as_str().unwrap()).await
}

#[sqlx::test(migrations = "./migrations")]
async fn delegated_staff_scans_signed_qr(pool: PgPool) {
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let attendee = token_for(&app, "a@x.co").await;
    let event = free_event(&app, &owner).await;
    let signed = reserve_signed_qr(&app, &attendee, &event).await;

    // The owner scans the SIGNED token (not the raw qr_token): valid, then already_used.
    let first = json_body(scan(&app, &owner, &event, &signed).await).await;
    assert_eq!(first["valid"], true);
    assert_eq!(first["reason"], "valid");
    let second = json_body(scan(&app, &owner, &event, &signed).await).await;
    assert_eq!(second["reason"], "already_used");
}

#[sqlx::test(migrations = "./migrations")]
async fn signed_qr_wrong_event_verdict(pool: PgPool) {
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let attendee = token_for(&app, "a@x.co").await;
    let event_a = free_event(&app, &owner).await;
    let event_b = free_event(&app, &owner).await;
    let signed_b = reserve_signed_qr(&app, &attendee, &event_b).await;

    // A signed QR for event B, scanned at event A's door → wrong_event.
    let resp = json_body(scan(&app, &owner, &event_a, &signed_b).await).await;
    assert_eq!(resp["valid"], false);
    assert_eq!(resp["reason"], "wrong_event");
}

#[sqlx::test(migrations = "./migrations")]
async fn staff_events_lists_owned_and_delegated(pool: PgPool) {
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let controller = token_for(&app, "c@x.co").await;
    let stranger = token_for(&app, "s@x.co").await;
    let event = free_event(&app, &owner).await;

    // Owner invites; controller accepts, gaining delegated scan rights.
    let invite = json_body(
        app.clone()
            .oneshot(build(
                "POST",
                &format!("/api/events/{event}/staff/invite"),
                Some(&owner),
                None,
            ))
            .await
            .unwrap(),
    )
    .await["invite_token"]
        .as_str()
        .unwrap()
        .to_string();
    app.clone()
        .oneshot(build(
            "POST",
            "/api/staff/accept",
            Some(&controller),
            Some(json!({ "token": invite })),
        ))
        .await
        .unwrap();

    // Controller sees exactly the event they may scan.
    let ctrl_list = staff_events(&app, &controller).await;
    assert_eq!(ctrl_list.as_array().unwrap().len(), 1);
    assert_eq!(ctrl_list[0]["id"], event);

    // Owner sees their own event too.
    assert_eq!(staff_events(&app, &owner).await.as_array().unwrap().len(), 1);

    // A stranger with no role sees nothing.
    assert_eq!(
        staff_events(&app, &stranger).await.as_array().unwrap().len(),
        0,
        "[SEC] a non-staff user sees no scannable events"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn staff_events_excludes_cancelled(pool: PgPool) {
    let app = make_app(pool);
    let owner = token_for(&app, "o@x.co").await;
    let event = free_event(&app, &owner).await;

    // A live event shows; once cancelled it drops out of the scannable list.
    assert_eq!(staff_events(&app, &owner).await.as_array().unwrap().len(), 1);
    app.clone()
        .oneshot(build("DELETE", &format!("/api/events/{event}"), Some(&owner), None))
        .await
        .unwrap();
    assert_eq!(staff_events(&app, &owner).await.as_array().unwrap().len(), 0);
}

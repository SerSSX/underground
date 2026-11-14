//! Owner decision 2026-11-14: a ticket cannot be offered (gift or transfer) to the sender's own
//! email. The place is already theirs and nothing would reach them, so the answer is 422
//! `OFFER_TO_SELF_REFUSED`, and nothing changes: no offer row, no alert, the ticket stays
//! `active`. Ownership is still checked first (TASK-305 F2): on someone else's ticket the answer
//! is 404 whatever the email. Every other valid address keeps the TASK-305 R2 answer.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use underground_api::routes::router;
use underground_api::state::AppState;
use underground_api::tickets::OFFER_TO_SELF_REFUSED;
use uuid::Uuid;

const SECRET: &str = "test-secret";

fn make_app(pool: PgPool) -> Router {
    router(AppState {
        db: pool,
        jwt_secret: SECRET.into(),
    })
}

async fn user(pool: &PgPool, email: &str) -> (Uuid, String) {
    let id: Uuid = sqlx::query_scalar("INSERT INTO users (email, name) VALUES ($1, 'U') RETURNING id")
        .bind(email)
        .fetch_one(pool)
        .await
        .unwrap();
    (id, underground_api::auth::issue_token(id, SECRET).unwrap())
}

async fn call(app: &Router, method: &str, path: &str, token: &str, body: Option<Value>) -> (StatusCode, Value) {
    let req = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(body.map(|v| Body::from(v.to_string())).unwrap_or_else(Body::empty))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

/// A free event by `org` with one place reserved by `token`; returns the ticket id.
async fn ticket(app: &Router, org: &str, token: &str) -> Uuid {
    let body = json!({"title":"Gig","category":"Poetry","price":"0","tickets_total":10,"cost_type":"free"});
    let (s, v) = call(app, "POST", "/api/events", org, Some(body)).await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    let event = v["id"].as_str().unwrap().to_string();
    let (s, v) = call(app, "POST", &format!("/api/events/{event}/reserve"), token, None).await;
    assert!(s.is_success(), "{s} {v}");
    v["id"].as_str().unwrap().parse().unwrap()
}

async fn offer(app: &Router, kind: &str, ticket: Uuid, token: &str, email: &str) -> (StatusCode, Value) {
    call(
        app,
        "POST",
        &format!("/api/tickets/{ticket}/{kind}"),
        token,
        Some(json!({ "email": email })),
    )
    .await
}

async fn revoke(app: &Router, ticket: Uuid, token: &str) {
    let (s, v) = call(app, "DELETE", &format!("/api/tickets/{ticket}/gift"), token, None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
}

async fn ticket_state(pool: &PgPool, ticket: Uuid) -> (Uuid, String) {
    sqlx::query_as("SELECT owner_id, status FROM tickets WHERE id = $1")
        .bind(ticket)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn count(pool: &PgPool, sql: &str, id: Uuid) -> i64 {
    sqlx::query_scalar(sql).bind(id).fetch_one(pool).await.unwrap()
}

async fn gifts(pool: &PgPool, ticket: Uuid) -> i64 {
    count(pool, "SELECT count(*) FROM ticket_gifts WHERE ticket_id = $1", ticket).await
}

async fn place_offer_alerts(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM notifications WHERE kind = 'place_offer'")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn bucket_hits(pool: &PgPool, sender: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM rate_events WHERE bucket = $1")
        .bind(format!("transfer:{sender}"))
        .fetch_one(pool)
        .await
        .unwrap()
}

fn assert_refused(out: (StatusCode, Value), what: &str) {
    assert_eq!(out.0, StatusCode::UNPROCESSABLE_ENTITY, "{what}: {}", out.1);
    assert_eq!(out.1, json!({ "error": OFFER_TO_SELF_REFUSED }), "{what}");
}

#[test]
fn the_message_is_the_one_the_web_translates() {
    // web/lib/server-messages.mjs keys its translations on this exact text.
    assert_eq!(OFFER_TO_SELF_REFUSED, "квиток уже ваш — надішліть його іншій людині");
}

#[sqlx::test(migrations = "./migrations")]
async fn offer_to_own_email_is_refused_and_changes_nothing(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "org@x.co").await;
    let (owner, ot) = user(&pool, "o@x.co").await;
    let tid = ticket(&app, &org, &ot).await;

    for (n, kind) in ["gift", "transfer"].into_iter().enumerate() {
        assert_refused(offer(&app, kind, tid, &ot, "o@x.co").await, kind);
        assert_eq!(ticket_state(&pool, tid).await, (owner, "active".into()), "{kind}");
        assert_eq!(gifts(&pool, tid).await, 0, "{kind}: no offer row");
        assert_eq!(place_offer_alerts(&pool).await, 0, "{kind}: no alert");
        assert_eq!(bucket_hits(&pool, owner).await, n as i64 + 1, "{kind}: still counted");
    }
    // The ticket is still usable: it can be returned like any active ticket.
    let (s, v) = call(&app, "POST", &format!("/api/tickets/{tid}/return"), &ot, None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
}

#[sqlx::test(migrations = "./migrations")]
async fn case_and_whitespace_variants_of_own_email_are_refused(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "org@x.co").await;
    // Stored with capitals (an older account): the comparison is case-blind on both sides.
    let (owner, ot) = user(&pool, "Mixed@X.co").await;
    let tid = ticket(&app, &org, &ot).await;

    for kind in ["gift", "transfer"] {
        for email in [" MIXED@x.co ", "mixed@x.co", "Mixed@X.co", "\tmixed@X.CO\n"] {
            assert_refused(offer(&app, kind, tid, &ot, email).await, &format!("{kind} {email:?}"));
        }
        assert_eq!(ticket_state(&pool, tid).await, (owner, "active".into()), "{kind}");
        assert_eq!(gifts(&pool, tid).await, 0, "{kind}");
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn non_owner_gets_404_even_with_their_own_email(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "org@x.co").await;
    let (owner, ot) = user(&pool, "o@x.co").await;
    let (_, bt) = user(&pool, "b@x.co").await;
    let tid = ticket(&app, &org, &ot).await;

    for kind in ["gift", "transfer"] {
        for email in ["b@x.co", " B@X.co", "o@x.co"] {
            let (s, v) = offer(&app, kind, tid, &bt, email).await;
            assert_eq!(s, StatusCode::NOT_FOUND, "[SEC] {kind} {email:?}: {v}");
        }
    }
    assert_eq!(ticket_state(&pool, tid).await, (owner, "active".into()));
    assert_eq!(gifts(&pool, tid).await, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn every_other_address_keeps_its_answer(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "org@x.co").await;
    let (owner, ot) = user(&pool, "o@x.co").await;
    let (r, rt) = user(&pool, "r@x.co").await;
    let tid = ticket(&app, &org, &ot).await;

    // Another account: 201 with a link (gift) / 202 (transfer), and that account is alerted.
    let (s, v) = offer(&app, "gift", tid, &ot, "R@x.co").await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    assert_eq!(v["to_email"], "r@x.co");
    assert_eq!(ticket_state(&pool, tid).await, (owner, "pending_claim".into()));
    let alerts = |who| count(&pool, "SELECT count(*) FROM notifications WHERE user_id = $1", who);
    assert_eq!(alerts(r).await, 1);
    revoke(&app, tid, &ot).await;
    let (s, v) = offer(&app, "transfer", tid, &ot, "r@x.co").await;
    assert_eq!((s, v), (StatusCode::ACCEPTED, json!({"status":"sent"})));
    assert_eq!(alerts(r).await, 2);
    revoke(&app, tid, &ot).await;

    // Unknown addresses, including ones that only look like the sender's (a plus tag, dots):
    // they are different addresses, so the R2 answer stands.
    for email in ["nobody@x.co", "o+tag@x.co", "o.@x.co", "o@x.com"] {
        let (s, v) = offer(&app, "transfer", tid, &ot, email).await;
        if s == StatusCode::UNPROCESSABLE_ENTITY {
            // Malformed text is refused for its form, never as "your own".
            assert_ne!(v["error"], OFFER_TO_SELF_REFUSED, "{email:?}");
            continue;
        }
        assert_eq!((s, v), (StatusCode::ACCEPTED, json!({"status":"sent"})), "{email:?}");
        revoke(&app, tid, &ot).await;
        let (s, v) = offer(&app, "gift", tid, &ot, email).await;
        assert_eq!(s, StatusCode::CREATED, "{email:?}: {v}");
        revoke(&app, tid, &ot).await;
    }
    // The recipient can still be offered the ticket and accept it.
    let (s, v) = offer(&app, "gift", tid, &ot, "r@x.co").await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    let token = v["claim_token"].as_str().unwrap();
    let (s, v) = call(&app, "POST", "/api/tickets/claim", &rt, Some(json!({ "token": token }))).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(ticket_state(&pool, tid).await, (r, "active".into()));
}

#[sqlx::test(migrations = "./migrations")]
async fn refused_self_offer_rolls_back_the_reclaim_of_an_expired_offer(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "org@x.co").await;
    let (owner, ot) = user(&pool, "o@x.co").await;
    let tid = ticket(&app, &org, &ot).await;
    let (s, v) = offer(&app, "gift", tid, &ot, "someone@x.co").await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    sqlx::query("UPDATE ticket_gifts SET expires_at = now() - interval '1 hour' WHERE ticket_id = $1")
        .bind(tid)
        .execute(&pool)
        .await
        .unwrap();

    for kind in ["gift", "transfer"] {
        // Both handlers mark the expired offer `expired` before the email is compared; the 422
        // must undo that, not leave a half-made re-offer.
        assert_refused(offer(&app, kind, tid, &ot, "o@x.co").await, kind);
        assert_eq!(
            ticket_state(&pool, tid).await,
            (owner, "pending_claim".into()),
            "{kind}"
        );
        let statuses: Vec<String> = sqlx::query_scalar("SELECT status FROM ticket_gifts WHERE ticket_id = $1")
            .bind(tid)
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(statuses, ["pending"], "{kind}");
    }
    // A real re-offer still works afterwards.
    let (s, v) = offer(&app, "transfer", tid, &ot, "other@x.co").await;
    assert_eq!((s, v), (StatusCode::ACCEPTED, json!({"status":"sent"})));
    assert_eq!(gifts(&pool, tid).await, 2);
}

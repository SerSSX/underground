//! Structural-bets backend: password reset, dispute-from-ticket DTO enrichment, and the
//! moderation audit-trail.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use chrono::{Duration, Utc};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use underground_api::passwords::token_hash;
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
            Some(json!({"email": email, "password": "password1", "name": "U"})),
        ),
    )
    .await;
    (
        v["token"].as_str().unwrap().to_string(),
        v["user"]["id"].as_str().unwrap().to_string(),
    )
}

async fn make_admin(app: &Router, pool: &PgPool, email: &str) -> String {
    let (token, _) = register(app, email).await;
    sqlx::query("UPDATE users SET role = 'ADMIN' WHERE email = $1")
        .bind(email)
        .execute(pool)
        .await
        .unwrap();
    token
}

// ---------- password reset ----------

#[sqlx::test(migrations = "./migrations")]
async fn forgot_password_creates_one_reset_and_is_silent_for_unknown(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, uid) = register(&app, "reset@x.co").await;

    // Known email → 204 and exactly one reset row.
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/auth/forgot-password",
            None,
            Some(json!({"email": "reset@x.co"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM password_resets WHERE user_id = $1::uuid")
        .bind(&uid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);

    // Unknown email → still 204 (no account enumeration), no new rows anywhere.
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/auth/forgot-password",
            None,
            Some(json!({"email": "nobody@x.co"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM password_resets")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(total, 1, "no reset row for an unknown email");
}

#[sqlx::test(migrations = "./migrations")]
async fn reset_password_rotates_credentials_and_consumes_token(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, uid) = register(&app, "rot@x.co").await;

    // Seed a valid reset row with a known raw token (hash only is stored).
    let raw = "raw-token-abc";
    sqlx::query("INSERT INTO password_resets (user_id, token_hash, expires_at) VALUES ($1::uuid, $2, $3)")
        .bind(&uid)
        .bind(token_hash(raw))
        .bind(Utc::now() + Duration::minutes(30))
        .execute(&pool)
        .await
        .unwrap();

    // Too-short password is rejected before any token work.
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/auth/reset-password",
            None,
            Some(json!({"token": raw, "password": "short"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);

    // Valid reset → 204.
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/auth/reset-password",
            None,
            Some(json!({"token": raw, "password": "newpassword1"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::NO_CONTENT);

    // New password logs in; old one no longer works.
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/auth/login",
            None,
            Some(json!({"email": "rot@x.co", "password": "newpassword1"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/auth/login",
            None,
            Some(json!({"email": "rot@x.co", "password": "password1"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);

    // The token is single-use: replay is rejected.
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/auth/reset-password",
            None,
            Some(json!({"token": raw, "password": "another12"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT);
}

#[sqlx::test(migrations = "./migrations")]
async fn reset_password_rejects_expired_token(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, uid) = register(&app, "exp@x.co").await;
    let raw = "expired-token";
    sqlx::query("INSERT INTO password_resets (user_id, token_hash, expires_at) VALUES ($1::uuid, $2, $3)")
        .bind(&uid)
        .bind(token_hash(raw))
        .bind(Utc::now() - Duration::minutes(1))
        .execute(&pool)
        .await
        .unwrap();
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/auth/reset-password",
            None,
            Some(json!({"token": raw, "password": "newpassword1"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "expired token cannot reset");
}

// ---------- dispute-from-ticket: DTO carries order + escrow status ----------

#[sqlx::test(migrations = "./migrations")]
async fn ticket_dto_exposes_order_and_escrow_for_dispute(pool: PgPool) {
    let app = make_app(pool.clone());
    let (maker, _) = register(&app, "maker@x.co").await;
    let (fan, _) = register(&app, "fan@x.co").await;

    // Paid event → checkout → pay-test → an order with escrow held, linked to the ticket.
    let (_, e) = send(
        &app,
        build(
            "POST",
            "/api/events",
            Some(&maker),
            Some(json!({"title": "Paid", "category": "Rock", "price": "50", "tickets_total": 5, "cost_type": "paid"})),
        ),
    )
    .await;
    let event_id = e["id"].as_str().unwrap();
    let (_, c) = send(
        &app,
        build("POST", &format!("/api/events/{event_id}/checkout"), Some(&fan), None),
    )
    .await;
    let order_id = c["order_id"].as_str().unwrap();
    send(
        &app,
        build("POST", &format!("/api/orders/{order_id}/pay-test"), Some(&fan), None),
    )
    .await;

    // My tickets now carry the order id + escrow status so the client can offer "dispute".
    let (st, tickets) = send(&app, build("GET", "/api/me/tickets", Some(&fan), None)).await;
    assert_eq!(st, StatusCode::OK);
    let t = &tickets[0];
    assert_eq!(t["order_id"].as_str(), Some(order_id));
    assert_eq!(t["escrow_status"].as_str(), Some("held"));

    // And the buyer can dispute that order.
    let (st, _) = send(
        &app,
        build("POST", &format!("/api/orders/{order_id}/dispute"), Some(&fan), None),
    )
    .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
}

// ---------- moderation audit-trail ----------

#[sqlx::test(migrations = "./migrations")]
async fn admin_actions_are_logged_and_listed(pool: PgPool) {
    let app = make_app(pool.clone());
    let admin = make_admin(&app, &pool, "admin@x.co").await;
    let (_, victim) = register(&app, "victim@x.co").await;

    // A privileged action: block the user.
    let (st, _) = send(
        &app,
        build(
            "PATCH",
            &format!("/api/admin/users/{victim}"),
            Some(&admin),
            Some(json!({"blocked": true})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);

    // The audit trail records who/what/whom.
    let (st, actions) = send(&app, build("GET", "/api/admin/actions", Some(&admin), None)).await;
    assert_eq!(st, StatusCode::OK);
    let row = actions
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["action"] == "user_update")
        .expect("block is logged");
    assert_eq!(row["target_type"], "user");
    assert_eq!(row["target_id"].as_str(), Some(victim.as_str()));
    assert_eq!(row["admin_email"], "admin@x.co");
    assert_eq!(row["detail"]["blocked"], true);

    // A non-admin cannot read the trail.
    let (other, _) = register(&app, "other@x.co").await;
    let (st, _) = send(&app, build("GET", "/api/admin/actions", Some(&other), None)).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "./migrations")]
async fn audit_trail_survives_admin_deletion_and_is_newest_first(pool: PgPool) {
    let app = make_app(pool.clone());
    let admin1 = make_admin(&app, &pool, "admin1@x.co").await;
    let admin1_id: String = sqlx::query_scalar("SELECT id::text FROM users WHERE email = 'admin1@x.co'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let (_, victim) = register(&app, "victim@x.co").await;

    // Two ordered actions by admin1 (explicit timestamps make DESC ordering deterministic).
    send(
        &app,
        build(
            "PATCH",
            &format!("/api/admin/users/{victim}"),
            Some(&admin1),
            Some(json!({"verified": true})),
        ),
    )
    .await;
    send(
        &app,
        build(
            "PATCH",
            &format!("/api/admin/users/{victim}"),
            Some(&admin1),
            Some(json!({"blocked": true})),
        ),
    )
    .await;
    // Force distinct, known created_at so newest-first ordering is testable without a same-second tie.
    sqlx::query("UPDATE admin_actions SET created_at = now() - interval '2 hours' WHERE detail->>'verified' = 'true'")
        .execute(&pool)
        .await
        .unwrap();

    // SET NULL on delete: the trail outlives the admin account.
    let admin2 = make_admin(&app, &pool, "admin2@x.co").await;
    sqlx::query("DELETE FROM users WHERE id = $1::uuid")
        .bind(&admin1_id)
        .execute(&pool)
        .await
        .unwrap();

    let (st, actions) = send(&app, build("GET", "/api/admin/actions", Some(&admin2), None)).await;
    assert_eq!(st, StatusCode::OK);
    let arr = actions.as_array().unwrap();
    // Newest first.
    assert!(
        arr[0]["created_at"].as_str() > arr[1]["created_at"].as_str(),
        "DESC by created_at"
    );
    // admin1's rows survive with a null actor.
    let orphan = arr
        .iter()
        .find(|a| a["detail"]["blocked"] == true)
        .expect("deleted admin's action retained");
    assert!(orphan["admin_email"].is_null(), "actor nulled, row kept");
}

// ---------- session invalidation + reset edge cases ----------

#[sqlx::test(migrations = "./migrations")]
async fn token_watermark_rejects_tokens_issued_before_it(pool: PgPool) {
    let app = make_app(pool.clone());
    let (token, uid) = register(&app, "wm@x.co").await;

    // Baseline: the fresh token works.
    let (st, _) = send(&app, build("GET", "/api/me", Some(&token), None)).await;
    assert_eq!(st, StatusCode::OK);

    // Move the watermark into the future → the existing token (older iat) is rejected.
    sqlx::query("UPDATE users SET tokens_valid_after = now() + interval '1 hour' WHERE id = $1::uuid")
        .bind(&uid)
        .execute(&pool)
        .await
        .unwrap();
    let (st, _) = send(&app, build("GET", "/api/me", Some(&token), None)).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED, "token predates watermark");

    // Reset the watermark to the epoch → the same token is accepted again.
    sqlx::query("UPDATE users SET tokens_valid_after = to_timestamp(0) WHERE id = $1::uuid")
        .bind(&uid)
        .execute(&pool)
        .await
        .unwrap();
    let (st, _) = send(&app, build("GET", "/api/me", Some(&token), None)).await;
    assert_eq!(st, StatusCode::OK);
}

#[sqlx::test(migrations = "./migrations")]
async fn reset_password_bumps_watermark_and_invalidates_all_tokens(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, uid) = register(&app, "multi@x.co").await;

    // Two outstanding reset rows for the same user.
    let raw = "multi-raw-token";
    for t in [raw, "second-token"] {
        sqlx::query("INSERT INTO password_resets (user_id, token_hash, expires_at) VALUES ($1::uuid, $2, $3)")
            .bind(&uid)
            .bind(token_hash(t))
            .bind(Utc::now() + Duration::minutes(30))
            .execute(&pool)
            .await
            .unwrap();
    }

    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/auth/reset-password",
            None,
            Some(json!({"token": raw, "password": "freshpass1"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::NO_CONTENT);

    // Both reset rows are now consumed (the one used + the sibling invalidated).
    let unused: i64 =
        sqlx::query_scalar("SELECT count(*) FROM password_resets WHERE user_id = $1::uuid AND used_at IS NULL")
            .bind(&uid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(unused, 0, "all outstanding reset tokens invalidated");

    // The watermark advanced past the epoch default.
    let bumped: bool = sqlx::query_scalar("SELECT tokens_valid_after > to_timestamp(1) FROM users WHERE id = $1::uuid")
        .bind(&uid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(bumped, "reset bumps tokens_valid_after");
}

#[sqlx::test(migrations = "./migrations")]
async fn reset_and_forgot_reject_blocked_accounts(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, uid) = register(&app, "blk@x.co").await;
    sqlx::query("UPDATE users SET blocked = true WHERE id = $1::uuid")
        .bind(&uid)
        .execute(&pool)
        .await
        .unwrap();

    // forgot-password stays silent (204) AND creates no reset row for a blocked account.
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/auth/forgot-password",
            None,
            Some(json!({"email": "blk@x.co"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM password_resets WHERE user_id = $1::uuid")
        .bind(&uid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 0, "no reset issued for blocked user");

    // Even with a (manually seeded) valid token, reset is refused for a blocked account.
    let raw = "blocked-token";
    sqlx::query("INSERT INTO password_resets (user_id, token_hash, expires_at) VALUES ($1::uuid, $2, $3)")
        .bind(&uid)
        .bind(token_hash(raw))
        .bind(Utc::now() + Duration::minutes(30))
        .execute(&pool)
        .await
        .unwrap();
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/auth/reset-password",
            None,
            Some(json!({"token": raw, "password": "newpass12"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "blocked account cannot reset");
}

#[sqlx::test(migrations = "./migrations")]
async fn transferred_ticket_hides_previous_owner_order(pool: PgPool) {
    let app = make_app(pool.clone());
    let (maker, _) = register(&app, "mk@x.co").await;
    let (buyer, _) = register(&app, "buyer@x.co").await;
    let (recipient, _) = register(&app, "recip@x.co").await;

    let (_, e) = send(
        &app,
        build(
            "POST",
            "/api/events",
            Some(&maker),
            Some(json!({"title": "P", "category": "Rock", "price": "30", "tickets_total": 5, "cost_type": "paid"})),
        ),
    )
    .await;
    let event_id = e["id"].as_str().unwrap();
    let (_, c) = send(
        &app,
        build("POST", &format!("/api/events/{event_id}/checkout"), Some(&buyer), None),
    )
    .await;
    let order_id = c["order_id"].as_str().unwrap();
    let (_, t) = send(
        &app,
        build("POST", &format!("/api/orders/{order_id}/pay-test"), Some(&buyer), None),
    )
    .await;
    let ticket_id = t["id"].as_str().unwrap();

    // Buyer gifts the paid ticket to the recipient.
    let (st, _) = send(
        &app,
        build(
            "POST",
            &format!("/api/tickets/{ticket_id}/transfer"),
            Some(&buyer),
            Some(json!({"email": "recip@x.co"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);

    // The recipient now owns the ticket but must NOT see the original buyer's order / escrow.
    let (st, tickets) = send(&app, build("GET", "/api/me/tickets", Some(&recipient), None)).await;
    assert_eq!(st, StatusCode::OK);
    let t = &tickets[0];
    assert!(t["order_id"].is_null(), "recipient cannot see the buyer's order_id");
    assert!(
        t["escrow_status"].is_null(),
        "recipient cannot see the buyer's escrow state"
    );

    // And the recipient cannot dispute an order they didn't buy.
    let (st, _) = send(
        &app,
        build(
            "POST",
            &format!("/api/orders/{order_id}/dispute"),
            Some(&recipient),
            None,
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "non-buyer cannot dispute");
}

#[sqlx::test(migrations = "./migrations")]
async fn cannot_transfer_a_disputed_ticket(pool: PgPool) {
    let app = make_app(pool.clone());
    let (maker, _) = register(&app, "mk2@x.co").await;
    let (buyer, _) = register(&app, "buyer2@x.co").await;
    register(&app, "recip2@x.co").await;

    let (_, e) = send(
        &app,
        build(
            "POST",
            "/api/events",
            Some(&maker),
            Some(json!({"title": "D", "category": "Rock", "price": "40", "tickets_total": 3, "cost_type": "paid"})),
        ),
    )
    .await;
    let event_id = e["id"].as_str().unwrap();
    let (_, c) = send(
        &app,
        build("POST", &format!("/api/events/{event_id}/checkout"), Some(&buyer), None),
    )
    .await;
    let order_id = c["order_id"].as_str().unwrap();
    let (_, t) = send(
        &app,
        build("POST", &format!("/api/orders/{order_id}/pay-test"), Some(&buyer), None),
    )
    .await;
    let ticket_id = t["id"].as_str().unwrap();

    let (st, _) = send(
        &app,
        build("POST", &format!("/api/orders/{order_id}/dispute"), Some(&buyer), None),
    )
    .await;
    assert_eq!(st, StatusCode::NO_CONTENT);

    let (st, _) = send(
        &app,
        build(
            "POST",
            &format!("/api/tickets/{ticket_id}/transfer"),
            Some(&buyer),
            Some(json!({"email": "recip2@x.co"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "disputed ticket cannot be transferred");
}

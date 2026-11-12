//! Organizer cabinet + platform admin APIs (TASK-014, Фази 3-4).

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
            Some(json!({"email":email,"password":"password1","name":email.split('@').next().unwrap()})),
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

/// maker creates a paid event; fan buys via the test flow → paid order + escrow held.
async fn sold_event(app: &Router, maker: &str, fan: &str) -> (String, String) {
    let (_, e) = send(
        app,
        build(
            "POST",
            "/api/events",
            Some(maker),
            Some(json!({"title":"Cabinet Gig","category":"Rock","price":"100","tickets_total":5,"cost_type":"paid"})),
        ),
    )
    .await;
    let event_id = e["id"].as_str().unwrap().to_string();
    let (_, c) = send(
        &app.clone(),
        build("POST", &format!("/api/events/{event_id}/checkout"), Some(fan), None),
    )
    .await;
    let order_id = c["order_id"].as_str().unwrap().to_string();
    let (status, _) = send(
        app,
        build("POST", &format!("/api/orders/{order_id}/pay-test"), Some(fan), None),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    (event_id, order_id)
}

#[sqlx::test(migrations = "./migrations")]
async fn organizer_stats_and_tickets(pool: PgPool) {
    let app = make_app(pool);
    let (maker, _) = register(&app, "maker@x.co").await;
    let (fan, _) = register(&app, "fan@x.co").await;
    let (event_id, _) = sold_event(&app, &maker, &fan).await;

    let (status, _) = send(&app, build("GET", "/api/organizer/stats", None, None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "[SEC] stats need auth");

    let (status, stats) = send(&app, build("GET", "/api/organizer/stats", Some(&maker), None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(stats["events_count"], 1);
    assert_eq!(stats["tickets_sold"], 1);
    assert_eq!(stats["tickets_validated"], 0);
    assert_eq!(stats["revenue"], "100.00");
    assert_eq!(stats["escrow_held"], "100.00");
    assert_eq!(stats["escrow_cleared"], "0");

    // Attendee list: owner sees the buyer; the buyer themselves gets 403.
    let (status, tickets) = send(
        &app,
        build("GET", &format!("/api/events/{event_id}/tickets"), Some(&maker), None),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    // TASK-302 Change E: the organiser no longer receives attendee emails (was == "fan@x.co").
    assert!(
        tickets[0].get("owner_email").is_none(),
        "[PII] no email in the attendee list"
    );
    assert_eq!(tickets[0]["status"], "active");

    let (status, _) = send(
        &app,
        build("GET", &format!("/api/events/{event_id}/tickets"), Some(&fan), None),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "[SEC] not the organizer → 403");

    let ghost = "00000000-0000-0000-0000-000000000000";
    let (status, _) = send(
        &app,
        build("GET", &format!("/api/events/{ghost}/tickets"), Some(&maker), None),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // A fresh account has a clean dashboard.
    let (newbie, _) = register(&app, "new@x.co").await;
    let (_, zero) = send(&app, build("GET", "/api/organizer/stats", Some(&newbie), None)).await;
    assert_eq!(zero["events_count"], 0);
    assert_eq!(zero["revenue"], "0");
}

#[sqlx::test(migrations = "./migrations")]
async fn admin_endpoints_guarded(pool: PgPool) {
    let app = make_app(pool);
    let (user, _) = register(&app, "u@x.co").await;
    for path in [
        "/api/admin/stats",
        "/api/admin/users",
        "/api/admin/events",
        "/api/admin/orders",
    ] {
        let (status, _) = send(&app, build("GET", path, Some(&user), None)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "[SEC] non-admin → 403 for {path}");
        let (status, _) = send(&app, build("GET", path, None, None)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "[SEC] anonymous → 401 for {path}");
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn admin_stats_users_events(pool: PgPool) {
    let app = make_app(pool.clone());
    let admin = make_admin(&app, &pool, "admin@x.co").await;
    let (maker, maker_id) = register(&app, "maker@x.co").await;
    let (fan, _) = register(&app, "fan@x.co").await;
    let (event_id, _) = sold_event(&app, &maker, &fan).await;

    let (_, stats) = send(&app, build("GET", "/api/admin/stats", Some(&admin), None)).await;
    assert_eq!(stats["users_count"], 3);
    assert_eq!(stats["events_count"], 1);
    assert_eq!(stats["tickets_count"], 1);
    assert_eq!(stats["gmv"], "100.00");
    assert_eq!(stats["platform_fee"], "10.00", "default 1000 bps = 10%");

    // Users: search + block + role validation.
    let (_, users) = send(&app, build("GET", "/api/admin/users?q=maker", Some(&admin), None)).await;
    assert_eq!(users.as_array().unwrap().len(), 1);
    // `%` is escaped to a literal — it must NOT wildcard-match every user (enumeration guard).
    let (_, pct) = send(&app, build("GET", "/api/admin/users?q=%25", Some(&admin), None)).await;
    assert!(
        pct.as_array().unwrap().is_empty(),
        "[SEC] LIKE metachar matched literally"
    );
    let (status, blocked) = send(
        &app,
        build(
            "PATCH",
            &format!("/api/admin/users/{maker_id}"),
            Some(&admin),
            Some(json!({"blocked": true})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(blocked["blocked"], true);
    let (status, _) = send(
        &app,
        build(
            "PATCH",
            &format!("/api/admin/users/{maker_id}"),
            Some(&admin),
            Some(json!({"role": "GOD"})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let ghost = "00000000-0000-0000-0000-000000000000";
    let (status, _) = send(
        &app,
        build(
            "PATCH",
            &format!("/api/admin/users/{ghost}"),
            Some(&admin),
            Some(json!({"blocked": true})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Events moderation: block removes from public афіша.
    let (_, list) = send(
        &app,
        build("GET", "/api/admin/events?status=published", Some(&admin), None),
    )
    .await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    let (_, found) = send(&app, build("GET", "/api/admin/events?q=Cabinet", Some(&admin), None)).await;
    assert_eq!(found.as_array().unwrap().len(), 1, "title search works");
    let (_, none) = send(&app, build("GET", "/api/admin/events?q=zzz", Some(&admin), None)).await;
    assert!(none.as_array().unwrap().is_empty());
    let (status, moderated) = send(
        &app,
        build(
            "PATCH",
            &format!("/api/admin/events/{event_id}"),
            Some(&admin),
            Some(json!({"status":"blocked"})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(moderated["status"], "blocked");
    let (_, afisha) = send(&app, build("GET", "/api/events", None, None)).await;
    assert!(
        afisha.as_array().unwrap().is_empty(),
        "blocked event hidden from public"
    );
    let (status, _) = send(
        &app,
        build(
            "PATCH",
            &format!("/api/admin/events/{event_id}"),
            Some(&admin),
            Some(json!({"status":"vanished"})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "./migrations")]
async fn admin_orders_and_escrow_actions(pool: PgPool) {
    let app = make_app(pool.clone());
    let admin = make_admin(&app, &pool, "admin@x.co").await;
    let (maker, _) = register(&app, "maker@x.co").await;
    let (fan, _) = register(&app, "fan@x.co").await;
    let (event_id, order_id) = sold_event(&app, &maker, &fan).await;

    let (_, orders) = send(&app, build("GET", "/api/admin/orders?status=paid", Some(&admin), None)).await;
    assert_eq!(orders[0]["escrow_status"], "held");
    assert_eq!(orders[0]["buyer_email"], "fan@x.co");

    let escrow = |action: &str| {
        build(
            "POST",
            &format!("/api/admin/orders/{order_id}/escrow"),
            Some(&admin),
            Some(json!({"action": action})),
        )
    };
    // Normal release: a held order clears.
    let (status, o) = send(&app, escrow("release")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(o["escrow_status"], "cleared");

    // A disputed (frozen) order is protected — release without force is rejected.
    let (status, o) = send(&app, escrow("freeze")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(o["escrow_status"], "frozen");
    let (status, _) = send(&app, escrow("release")).await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "[SEC] frozen order needs force to release"
    );
    // Explicit force override clears the dispute in the organizer's favour.
    let (status, o) = send(
        &app,
        build(
            "POST",
            &format!("/api/admin/orders/{order_id}/escrow"),
            Some(&admin),
            Some(json!({"action": "release", "force": true})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(o["escrow_status"], "cleared", "force clears a frozen order");

    let (status, _) = send(&app, escrow("explode")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "unknown action rejected");

    let (_, o) = send(&app, escrow("refund")).await;
    assert_eq!(o["escrow_status"], "refunded");
    assert_eq!(o["status"], "refunded");
    // Seat freed + the fan's ticket returned.
    let (_, event) = send(&app, build("GET", &format!("/api/events/{event_id}"), None, None)).await;
    assert_eq!(event["tickets_left"], 5, "seat freed after refund");
    let (_, tickets) = send(&app, build("GET", "/api/me/tickets", Some(&fan), None)).await;
    assert_eq!(tickets[0]["status"], "returned");

    // Refund is idempotent: a repeated refund must NOT double-free the seat.
    let (_, o) = send(&app, escrow("refund")).await;
    assert_eq!(o["status"], "refunded");
    let (_, event) = send(&app, build("GET", &format!("/api/events/{event_id}"), None, None)).await;
    assert_eq!(
        event["tickets_left"], 5,
        "repeated refund does not double-free the seat"
    );

    // paid_out is terminal.
    sqlx::query("UPDATE orders SET escrow_status = 'paid_out' WHERE id = $1::uuid")
        .bind(&order_id)
        .execute(&pool)
        .await
        .unwrap();
    let (status, _) = send(&app, escrow("refund")).await;
    assert_eq!(status, StatusCode::CONFLICT, "paid_out is final");

    let ghost = "00000000-0000-0000-0000-000000000000";
    let (status, _) = send(
        &app,
        build(
            "POST",
            &format!("/api/admin/orders/{ghost}/escrow"),
            Some(&admin),
            Some(json!({"action":"freeze"})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Refund of a pending order (checkout without payment → no ticket minted yet).
    let (_, c) = send(
        &app,
        build("POST", &format!("/api/events/{event_id}/checkout"), Some(&fan), None),
    )
    .await;
    let pending_id = c["order_id"].as_str().unwrap();
    let (status, o) = send(
        &app,
        build(
            "POST",
            &format!("/api/admin/orders/{pending_id}/escrow"),
            Some(&admin),
            Some(json!({"action":"refund"})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "pending order refundable without a ticket");
    assert_eq!(o["status"], "refunded");
}

#[sqlx::test(migrations = "./migrations")]
async fn blocked_user_loses_jwt_access(pool: PgPool) {
    let app = make_app(pool.clone());
    let (token, id) = register(&app, "evil@x.co").await;

    // Baseline: the still-valid token works on a CurrentUser endpoint that does not itself
    // re-filter on `blocked`.
    let (status, _) = send(&app, build("GET", "/api/organizer/stats", Some(&token), None)).await;
    assert_eq!(status, StatusCode::OK);

    // Block the account *after* the token was issued.
    sqlx::query("UPDATE users SET blocked = true WHERE id = $1::uuid")
        .bind(&id)
        .execute(&pool)
        .await
        .unwrap();

    // The same unrotated token is now rejected everywhere the central extractor runs.
    let (status, _) = send(&app, build("GET", "/api/organizer/stats", Some(&token), None)).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "[SEC] blocked user loses write access"
    );
    let (status, _) = send(&app, build("GET", "/api/me", Some(&token), None)).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "[SEC] check is centralized in CurrentUser"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn admin_moderation_removes_content(pool: PgPool) {
    let app = make_app(pool.clone());
    let admin = make_admin(&app, &pool, "admin@x.co").await;
    let (user, user_id) = register(&app, "u@x.co").await;

    // Author posts; a non-admin can't moderate, an admin can; a second delete is idempotent (204).
    let (st, p) = send(
        &app,
        build("POST", "/api/posts", Some(&user), Some(json!({"text":"spam"}))),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    let post_id = p["id"].as_str().unwrap().to_string();
    let (st, _) = send(
        &app,
        build("DELETE", &format!("/api/admin/posts/{post_id}"), Some(&user), None),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN, "[SEC] non-admin can't moderate");
    let (st, _) = send(
        &app,
        build("DELETE", &format!("/api/admin/posts/{post_id}"), Some(&admin), None),
    )
    .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    let (st, _) = send(
        &app,
        build("DELETE", &format!("/api/admin/posts/{post_id}"), Some(&admin), None),
    )
    .await;
    assert_eq!(st, StatusCode::NO_CONTENT, "idempotent re-delete");

    // A review (inserted directly) can be moderated away too.
    let (_, ev) = send(
        &app,
        build(
            "POST",
            "/api/events",
            Some(&user),
            Some(json!({"title":"E","category":"Rock","price":"0","tickets_total":1,"cost_type":"free"})),
        ),
    )
    .await;
    let event_id = ev["id"].as_str().unwrap();
    let review_id: String = sqlx::query_scalar(
        "INSERT INTO reviews (event_id, author_id, rating, text) VALUES ($1::uuid, $2::uuid, 5, 'bad') RETURNING id::text",
    )
    .bind(event_id)
    .bind(&user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let (st, _) = send(
        &app,
        build("DELETE", &format!("/api/admin/reviews/{review_id}"), Some(&admin), None),
    )
    .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    let (st, _) = send(
        &app,
        build("DELETE", &format!("/api/admin/reviews/{review_id}"), Some(&admin), None),
    )
    .await;
    assert_eq!(st, StatusCode::NO_CONTENT, "idempotent re-delete");
}

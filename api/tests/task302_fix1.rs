//! TASK-302 fix batch 1 (reviews of the TASK-302 API work). Each test names its item.
//!
//! This binary also exercises free-only mode, a process-wide switch; every event created here is
//! free anyway, so turning it on in one test cannot change another test's outcome.

use std::time::Instant;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use underground_api::routes::router;
use underground_api::state::AppState;
use uuid::Uuid;

const SECRET: &str = "test-secret";

fn make_app(pool: PgPool) -> Router {
    router(AppState {
        db: pool,
        jwt_secret: SECRET.into(),
    })
}

async fn user(pool: &PgPool, email: &str, name: &str) -> (Uuid, String) {
    let id: Uuid = sqlx::query_scalar("INSERT INTO users (email, name) VALUES ($1, $2) RETURNING id")
        .bind(email)
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap();
    (id, underground_api::auth::issue_token(id, SECRET).unwrap())
}

/// A token for `id` issued `age_secs` ago.
fn token_aged(id: Uuid, age_secs: i64) -> String {
    let now = chrono::Utc::now().timestamp();
    let claims = underground_api::auth::Claims {
        sub: id.to_string(),
        exp: (now + 3600) as usize,
        iat: now - age_secs,
    };
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(SECRET.as_bytes()),
    )
    .unwrap()
}

async fn send(app: &Router, req: Request<Body>) -> (StatusCode, String) {
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

fn req(method: &str, path: &str, token: Option<&str>, xff: Option<&str>, body: Option<Value>) -> Request<Body> {
    let mut b = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    if let Some(ip) = xff {
        b = b.header("x-forwarded-for", ip);
    }
    b.body(body.map(|v| Body::from(v.to_string())).unwrap_or_else(Body::empty))
        .unwrap()
}

async fn call(app: &Router, method: &str, path: &str, token: Option<&str>, body: Option<Value>) -> (StatusCode, Value) {
    let (s, t) = send(app, req(method, path, token, None, body)).await;
    (s, serde_json::from_str(&t).unwrap_or(Value::Null))
}

async fn free_event(app: &Router, token: &str, extra: Value) -> Uuid {
    let mut body = json!({"title":"Gig","category":"Poetry","price":"0","tickets_total":30,"cost_type":"free"});
    for (k, v) in extra.as_object().unwrap() {
        body[k] = v.clone();
    }
    let (s, v) = call(app, "POST", "/api/events", Some(token), Some(body)).await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    v["id"].as_str().unwrap().parse().unwrap()
}

async fn reserve(app: &Router, token: &str, event: Uuid, q: i64) -> (StatusCode, Value) {
    call(
        app,
        "POST",
        &format!("/api/events/{event}/reserve"),
        Some(token),
        Some(json!({ "quantity": q })),
    )
    .await
}

async fn active_ids(pool: &PgPool, owner: Uuid, event: Uuid) -> Vec<Uuid> {
    sqlx::query_scalar("SELECT id FROM tickets WHERE owner_id = $1 AND event_id = $2 AND status = 'active' ORDER BY id")
        .bind(owner)
        .bind(event)
        .fetch_all(pool)
        .await
        .unwrap()
}

async fn holdings(pool: &PgPool, owner: Uuid, event: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM tickets WHERE owner_id = $1 AND event_id = $2 \
         AND status IN ('active','pending_claim','used')",
    )
    .bind(owner)
    .bind(event)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn tickets_left(pool: &PgPool, event: Uuid) -> i32 {
    sqlx::query_scalar("SELECT tickets_left FROM events WHERE id = $1")
        .bind(event)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn qr(app: &Router, token: &str, ticket: Uuid) -> String {
    let (s, v) = call(app, "GET", &format!("/api/tickets/{ticket}/qr"), Some(token), None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    v["token"].as_str().unwrap().to_string()
}

async fn gift(app: &Router, token: &str, ticket: Uuid, email: &str) -> String {
    let (s, v) = call(
        app,
        "POST",
        &format!("/api/tickets/{ticket}/gift"),
        Some(token),
        Some(json!({ "email": email })),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    v["claim_token"].as_str().unwrap().to_string()
}

// ── Item 1: a password-less account's name is never its email local part ─────────────

#[sqlx::test(migrations = "./migrations")]
async fn item1_email_code_account_never_shows_its_local_part(pool: PgPool) {
    const LOCAL: &str = "loc.part.x7q";
    let app = make_app(pool.clone());
    let email = format!("{LOCAL}@x.co");
    sqlx::query("INSERT INTO login_codes (email, code_hash, expires_at) VALUES ($1, $2, now() + interval '5 minutes')")
        .bind(&email)
        .bind(underground_api::emailcode::code_hash("123456"))
        .execute(&pool)
        .await
        .unwrap();
    let (s, auth) = call(
        &app,
        "POST",
        "/api/auth/email/verify-code",
        None,
        Some(json!({"email": email, "code": "123456"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{auth}");
    assert_eq!(auth["user"]["name"], "Гість");
    let me = auth["token"].as_str().unwrap().to_string();
    let me_id: Uuid = auth["user"]["id"].as_str().unwrap().parse().unwrap();

    let (org_id, org) = user(&pool, "o@x.co", "Org").await;
    let event = free_event(&app, &org, json!({})).await;
    assert_eq!(reserve(&app, &me, event, 2).await.0, StatusCode::CREATED);
    call(&app, "POST", &format!("/api/events/{event}/save"), Some(&me), None).await;
    // Mutual follow with the organiser, so the organiser's social view lists this person.
    call(&app, "POST", &format!("/api/users/{org_id}/follow"), Some(&me), None).await;
    call(&app, "POST", &format!("/api/users/{me_id}/follow"), Some(&org), None).await;
    let claim = gift(&app, &me, active_ids(&pool, me_id, event).await[0], "friend@x.co").await;
    let (s, _) = call(
        &app,
        "POST",
        "/api/messages",
        Some(&me),
        Some(json!({"recipient_id": org_id, "text": "hi"})),
    )
    .await;
    assert!(s.is_success(), "message sent: {s}");
    sqlx::query("INSERT INTO reviews (event_id, author_id, rating, text) VALUES ($1, $2, 5, 'ok')")
        .bind(event)
        .bind(me_id)
        .execute(&pool)
        .await
        .unwrap();
    let staff_event = free_event(&app, &org, json!({})).await;
    let (_, inv) = call(
        &app,
        "POST",
        &format!("/api/events/{staff_event}/staff/invite"),
        Some(&org),
        None,
    )
    .await;
    let (s, _) = call(
        &app,
        "POST",
        "/api/staff/accept",
        Some(&me),
        Some(json!({"token": inv["invite_token"]})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    let views = [
        ("profile", format!("/api/users/{me_id}"), None),
        ("reviews", format!("/api/events/{event}/reviews"), None),
        ("conversations", "/api/me/conversations".to_string(), Some(org.clone())),
        ("gift preview", format!("/api/gifts/{claim}"), None),
        ("event social", format!("/api/events/{event}/social"), Some(org.clone())),
        (
            "staff list",
            format!("/api/events/{staff_event}/staff"),
            Some(org.clone()),
        ),
    ];
    for (label, path, token) in views {
        let (s, text) = send(&app, req("GET", &path, token.as_deref(), None, None)).await;
        assert_eq!(s, StatusCode::OK, "{label}: {text}");
        assert!(text.contains("Гість"), "{label} shows the neutral name: {text}");
        assert!(
            !text.to_lowercase().contains(LOCAL),
            "[PII] {label} leaks the local part: {text}"
        );
    }
}

/// Item 1 backfill + item 5 idempotency: re-running migration 0048 on a database that already
/// has it is safe, and it renames auto-named password-less accounts only.
#[sqlx::test(migrations = "./migrations")]
async fn item1_item5_migration_backfill_is_idempotent(pool: PgPool) {
    let (auto, _) = user(&pool, "auto.name@x.co", "auto.name").await;
    let (typed, _) = user(&pool, "typed@x.co", "typed").await;
    sqlx::query("UPDATE users SET password_hash = 'x' WHERE id = $1")
        .bind(typed)
        .execute(&pool)
        .await
        .unwrap();
    let (real, _) = user(&pool, "real@x.co", "Олена").await;
    for _ in 0..2 {
        sqlx::raw_sql(include_str!("../migrations/0048_multi_place_booking.sql"))
            .execute(&pool)
            .await
            .expect("0048 re-runs cleanly");
    }
    let name = |id: Uuid| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, String>("SELECT name FROM users WHERE id = $1")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    assert_eq!(name(auto).await, "Гість");
    assert_eq!(name(typed).await, "typed", "a password account's own choice is kept");
    assert_eq!(name(real).await, "Олена");
}

// ── Item 2: password-less erasure needs a recent sign-in ─────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn item2_passwordless_delete_needs_a_recent_login(pool: PgPool) {
    let app = make_app(pool.clone());
    let pl = underground_api::users::create_passwordless_user(&pool, "nopw@x.co")
        .await
        .unwrap();
    let stale = token_aged(pl.id, 11 * 60);
    let (s, text) = send(
        &app,
        req(
            "DELETE",
            "/api/me",
            Some(&stale),
            None,
            Some(json!({"confirm":"DELETE"})),
        ),
    )
    .await;
    assert_eq!(
        s,
        StatusCode::UNAUTHORIZED,
        "[SEC] an 11-minute-old sign-in cannot erase: {text}"
    );
    assert!(text.contains("увійдіть"), "tells the user to sign in again: {text}");
    let blocked: bool = sqlx::query_scalar("SELECT blocked FROM users WHERE id = $1")
        .bind(pl.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!blocked, "nothing was erased");
    let fresh = token_aged(pl.id, 60);
    let (s, _) = send(
        &app,
        req(
            "DELETE",
            "/api/me",
            Some(&fresh),
            None,
            Some(json!({"confirm":"DELETE"})),
        ),
    )
    .await;
    assert_eq!(s, StatusCode::NO_CONTENT);

    // Password accounts are unchanged: an old token plus the password still erases.
    let (s, v) = call(
        &app,
        "POST",
        "/api/auth/register",
        None,
        Some(json!({"email":"pw@x.co","password":"password1","name":"Pat"})),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    let pw_id: Uuid = v["user"]["id"].as_str().unwrap().parse().unwrap();
    let old = token_aged(pw_id, 3 * 24 * 3600);
    let (s, _) = send(
        &app,
        req(
            "DELETE",
            "/api/me",
            Some(&old),
            None,
            Some(json!({"password":"password1"})),
        ),
    )
    .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
}

// ── Item 3: register and code requests are throttled; no hash for a taken email ───────

fn register_req(email: &str, ip: &str) -> Request<Body> {
    req(
        "POST",
        "/api/auth/register",
        None,
        Some(ip),
        Some(json!({"email": email, "password": "password1", "name": "Reg"})),
    )
}

#[sqlx::test(migrations = "./migrations")]
async fn item3_register_is_rate_limited_per_ip_and_per_email(pool: PgPool) {
    let app = make_app(pool.clone());
    for i in 0..10 {
        let (s, t) = send(&app, register_req(&format!("r{i}@x.co"), "203.0.113.5")).await;
        assert!(
            s == StatusCode::CREATED || s == StatusCode::CONFLICT || s == StatusCode::UNPROCESSABLE_ENTITY,
            "{s} {t}"
        );
    }
    let (s, _) = send(&app, register_req("r10@x.co", "203.0.113.5")).await;
    assert_eq!(
        s,
        StatusCode::TOO_MANY_REQUESTS,
        "[SEC] 11th sign-up from one IP in an hour"
    );
    // A client cannot escape by prepending its own entries: the proxy's (rightmost) one counts.
    let (s, _) = send(&app, register_req("r11@x.co", "1.2.3.4, 203.0.113.5")).await;
    assert_eq!(
        s,
        StatusCode::TOO_MANY_REQUESTS,
        "[SEC] spoofed left entries are ignored"
    );
    // The email-code request (also the password-less LOGIN) has its own bucket (fix batch 2, N4;
    // was: shared with register, 429 here).
    let (s, _) = send(
        &app,
        req(
            "POST",
            "/api/auth/email/request-code",
            None,
            Some("203.0.113.5"),
            Some(json!({"email":"code@x.co"})),
        ),
    )
    .await;
    assert_eq!(
        s,
        StatusCode::NO_CONTENT,
        "sign-up throttling does not lock out code logins"
    );
    // Another IP is unaffected.
    assert_eq!(
        send(&app, register_req("other@x.co", "198.51.100.1")).await.0,
        StatusCode::CREATED
    );

    // Per email: 5 attempts an hour, whatever the IP (1 created + 4 conflicts, then 429).
    for i in 0..5 {
        let (s, _) = send(&app, register_req("victim@x.co", &format!("192.0.2.{i}"))).await;
        assert!(s == StatusCode::CREATED || s == StatusCode::CONFLICT, "{s}");
    }
    let (s, _) = send(&app, register_req("victim@x.co", "192.0.2.99")).await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS, "[SEC] 6th attempt on one email");
}

/// A taken email is refused before the password is hashed (argon2 dominates a sign-up).
#[sqlx::test(migrations = "./migrations")]
async fn item3_taken_email_is_refused_before_hashing(pool: PgPool) {
    let app = make_app(pool.clone());
    let started = Instant::now();
    assert_eq!(
        send(&app, register_req("taken@x.co", "192.0.2.1")).await.0,
        StatusCode::CREATED
    );
    let with_hash = started.elapsed();
    let started = Instant::now();
    for i in 2..5 {
        let (s, _) = send(&app, register_req("taken@x.co", &format!("192.0.2.{i}"))).await;
        assert_eq!(s, StatusCode::CONFLICT);
    }
    let per_conflict = started.elapsed() / 3;
    assert!(
        per_conflict * 3 < with_hash,
        "a 409 must not pay for argon2: {per_conflict:?} per 409 vs {with_hash:?} for a sign-up"
    );
}

// ── Item 4: deletion also returns outstanding gifts, by event cost type ──────────────

#[sqlx::test(migrations = "./migrations")]
async fn item4_delete_returns_gifted_places_and_revokes_gifts(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let (u, ut) = user(&pool, "u@x.co", "Uma").await;
    let event = free_event(&app, &org, json!({})).await;
    assert_eq!(reserve(&app, &ut, event, 3).await.0, StatusCode::CREATED);
    let ids = active_ids(&pool, u, event).await;
    let claim = gift(&app, &ut, ids[0], "friend@x.co").await;
    assert_eq!(tickets_left(&pool, event).await, 27);
    let (s, _) = call(&app, "DELETE", "/api/me", Some(&ut), Some(json!({"confirm":"DELETE"}))).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    assert_eq!(
        tickets_left(&pool, event).await,
        30,
        "all three places, the gifted one too"
    );
    let gift_status: String = sqlx::query_scalar("SELECT status FROM ticket_gifts WHERE ticket_id = $1")
        .bind(ids[0])
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(gift_status, "revoked");
    let (_, friend) = user(&pool, "friend@x.co", "Fred").await;
    let (s, _) = call(
        &app,
        "POST",
        "/api/tickets/claim",
        Some(&friend),
        Some(json!({ "token": claim })),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND, "a revoked gift cannot be claimed");
}

// ── Item 6: concurrency at the door and at the cap ───────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn item6_same_qr_at_both_doors_admits_once(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let (u, ut) = user(&pool, "u@x.co", "Uma").await;
    let event = free_event(&app, &org, json!({})).await;
    assert_eq!(reserve(&app, &ut, event, 5).await.0, StatusCode::CREATED);
    for ticket in active_ids(&pool, u, event).await {
        let code = qr(&app, &ut, ticket).await;
        let scan_path = format!("/api/events/{event}/scan");
        let (a, b) = tokio::join!(
            call(&app, "POST", &scan_path, Some(&org), Some(json!({"qr_token": code}))),
            call(
                &app,
                "POST",
                "/api/tickets/validate",
                Some(&org),
                Some(json!({"qr_token": code}))
            ),
        );
        let valid = [a.1["valid"].as_bool(), b.1["valid"].as_bool()]
            .iter()
            .filter(|v| **v == Some(true))
            .count();
        assert_eq!(valid, 1, "[SEC] exactly one admit: {} / {}", a.1, b.1);
        let other = if a.1["valid"] == true { &b.1 } else { &a.1 };
        assert_eq!(other["reason"], "already_used");
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn item6_concurrent_reserve_and_claim_respect_the_cap(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    for round in 0..5 {
        let event = free_event(&app, &org, json!({})).await;
        let (v, vt) = user(&pool, &format!("v{round}@x.co"), "Vik").await;
        let (u, ut) = user(&pool, &format!("u{round}@x.co"), "Uma").await;
        assert_eq!(reserve(&app, &vt, event, 4).await.0, StatusCode::CREATED);
        assert_eq!(reserve(&app, &ut, event, 1).await.0, StatusCode::CREATED);
        let claim = gift(
            &app,
            &ut,
            active_ids(&pool, u, event).await[0],
            &format!("v{round}@x.co"),
        )
        .await;
        let (r, c) = tokio::join!(
            reserve(&app, &vt, event, 1),
            call(
                &app,
                "POST",
                "/api/tickets/claim",
                Some(&vt),
                Some(json!({ "token": claim }))
            ),
        );
        let wins = [r.0 == StatusCode::CREATED, c.0 == StatusCode::OK]
            .iter()
            .filter(|w| **w)
            .count();
        assert_eq!(wins, 1, "[SEC] round {round}: reserve {} / claim {}", r.0, c.0);
        assert_eq!(holdings(&pool, v, event).await, 5);
    }
}

// ── Item 7: pending staff invites are capped ─────────────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn item7_pending_invites_are_capped_at_twenty(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let event = free_event(&app, &org, json!({})).await;
    let path = format!("/api/events/{event}/staff/invite");
    let mut set = tokio::task::JoinSet::new();
    for _ in 0..25 {
        let (app, org, path) = (app.clone(), org.clone(), path.clone());
        set.spawn(async move { call(&app, "POST", &path, Some(&org), None).await.0 });
    }
    let mut created = 0;
    while let Some(s) = set.join_next().await {
        match s.unwrap() {
            StatusCode::CREATED => created += 1,
            StatusCode::CONFLICT => {}
            other => panic!("unexpected {other}"),
        }
    }
    assert_eq!(created, 20, "[SEC] 25 parallel invites, cap 20");
    let (_, list) = call(&app, "GET", &format!("/api/events/{event}/staff"), Some(&org), None).await;
    let invites = list["invites"].as_array().unwrap();
    assert_eq!(invites.len(), 20, "every live invite is listed");
    let id = invites[0]["id"].as_str().unwrap();
    let (s, _) = call(
        &app,
        "DELETE",
        &format!("/api/events/{event}/staff/invites/{id}"),
        Some(&org),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    assert_eq!(
        call(&app, "POST", &path, Some(&org), None).await.0,
        StatusCode::CREATED,
        "room after a revoke"
    );
}

// ── Item 10: no social data for an event the public cannot see ───────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn item10_social_is_404_unless_published(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let event = free_event(&app, &org, json!({})).await;
    let path = format!("/api/events/{event}/social");
    assert_eq!(call(&app, "GET", &path, None, None).await.0, StatusCode::OK);
    for status in ["hidden", "draft", "blocked", "cancelled"] {
        sqlx::query("UPDATE events SET status = $2 WHERE id = $1")
            .bind(event)
            .bind(status)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            call(&app, "GET", &path, Some(&org), None).await.0,
            StatusCode::NOT_FOUND,
            "{status}"
        );
    }
}

// ── Item 11a: the ticket carries the event's end ─────────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn item11a_ticket_carries_event_ends_at(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let (_, ut) = user(&pool, "u@x.co", "Uma").await;
    let event = free_event(
        &app,
        &org,
        json!({"starts_at":"2031-05-01T18:00:00Z","ends_at":"2031-05-02T02:00:00Z"}),
    )
    .await;
    let (s, t) = reserve(&app, &ut, event, 1).await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(t["event_ends_at"], "2031-05-02T02:00:00Z");
    let (_, mine) = call(&app, "GET", "/api/me/tickets", Some(&ut), None).await;
    assert_eq!(mine[0]["event_ends_at"], "2031-05-02T02:00:00Z");
}

// ── Item 11b: only http(s) links are stored ──────────────────────────────────────────

#[test]
fn item11b_url_rule() {
    use underground_api::events::is_safe_url;
    for ok in ["https://youtu.be/x", "HTTP://Example.com/a?b#c", "https://x.co", ""] {
        assert!(is_safe_url(ok, false), "{ok}");
    }
    assert!(is_safe_url("/uploads/abc.jpg", true));
    for bad in [
        "javascript:alert(1)",
        "JavaScript:alert(1)",
        " javascript:alert(1)",
        "data:text/html,<script>1</script>",
        "vbscript:x",
        "//evil.example/x",
        "/uploads/abc.jpg",
        "https://",
        "https://@evil",
        "http:/x",
        "ftp://x.co",
        "https://x.co/a b",
        "https://x.co\\@evil",
        "java\tscript:alert(1)",
    ] {
        assert!(!is_safe_url(bad, false), "[SEC] {bad:?}");
    }
    for bad in [
        "/uploads/../etc/passwd",
        "/uploads//x",
        "/uploadsx",
        "/other/x.jpg",
        "/uploads/%2e%2e/etc/passwd",
        "/uploads/.%2e/x",
        "/uploads/%2E%2E/x",
        "/uploads/a%2fb",
    ] {
        assert!(!is_safe_url(bad, true), "[SEC] {bad:?}");
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn item11b_event_urls_must_be_http(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let base = json!({"title":"Gig","category":"Poetry","price":"0","tickets_total":5,"cost_type":"free"});
    for (field, value) in [
        ("links", json!(["javascript:alert(document.cookie)"])),
        ("video_url", json!("data:text/html,x")),
        ("poster_url", json!("//evil.example/p.jpg")),
        ("gallery_urls", json!(["javascript:alert(1)"])),
    ] {
        let mut body = base.clone();
        body[field] = value;
        let (s, _) = call(&app, "POST", "/api/events", Some(&org), Some(body)).await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "[SEC] create with bad {field}");
    }
    let mut good = base.clone();
    good["links"] = json!(["https://t.me/gig"]);
    good["poster_url"] = json!("/uploads/p.jpg");
    good["gallery_urls"] = json!(["/uploads/g.jpg", "https://cdn.example/g.png"]);
    good["video_url"] = json!("https://youtu.be/x");
    let (s, ev) = call(&app, "POST", "/api/events", Some(&org), Some(good)).await;
    assert_eq!(s, StatusCode::CREATED, "{ev}");
    let id = ev["id"].as_str().unwrap();
    let (s, _) = call(
        &app,
        "PUT",
        &format!("/api/events/{id}"),
        Some(&org),
        Some(json!({"links":["javascript:alert(1)"]})),
    )
    .await;
    assert_eq!(
        s,
        StatusCode::UNPROCESSABLE_ENTITY,
        "[SEC] update with a javascript: link"
    );
    let stored: Vec<String> = sqlx::query_scalar("SELECT links FROM events WHERE id = $1::uuid")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, vec!["https://t.me/gig".to_string()]);
}

fn is_zero(v: &Value) -> bool {
    v.as_str().and_then(|s| s.parse::<f64>().ok()) == Some(0.0)
}

// ── Item 11c: free-only mode ─────────────────────────────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn item11c_free_only_mode_coerces_paid_events(pool: PgPool) {
    std::env::remove_var("FREE_EVENTS_ONLY");
    assert!(underground_api::events::free_events_only_from_env(), "on by default");
    std::env::set_var("FREE_EVENTS_ONLY", "0");
    assert!(!underground_api::events::free_events_only_from_env());
    std::env::set_var("FREE_EVENTS_ONLY", "1");
    assert!(underground_api::events::free_events_only_from_env());
    std::env::remove_var("FREE_EVENTS_ONLY");

    underground_api::events::set_free_events_only(true);
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let (_, ut) = user(&pool, "u@x.co", "Uma").await;
    // An old client sends a paid event; another sends no cost type (the default used to be "paid").
    for body in [
        json!({"title":"Old app","category":"Rock","price":"250","tickets_total":5,"cost_type":"paid"}),
        json!({"title":"No type","category":"Rock","price":"99","tickets_total":5}),
    ] {
        let (s, ev) = call(&app, "POST", "/api/events", Some(&org), Some(body)).await;
        assert_eq!(s, StatusCode::CREATED);
        assert_eq!(ev["cost_type"], "free", "{ev}");
        assert!(is_zero(&ev["price"]), "{ev}");
        let id: Uuid = ev["id"].as_str().unwrap().parse().unwrap();
        assert_eq!(
            reserve(&app, &ut, id, 1).await.0,
            StatusCode::CREATED,
            "bookable for free"
        );
        let (s, upd) = call(
            &app,
            "PUT",
            &format!("/api/events/{id}"),
            Some(&org),
            Some(json!({"price":"50"})),
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert!(is_zero(&upd["price"]), "an edit cannot set a price either: {upd}");
    }
}

// ══ Fix batch 2 ══════════════════════════════════════════════════════════════════════

/// N1: the organiser moves the start a week later and empties the end field. A null `ends_at`
/// keeps the old end, so the web sends `clear_ends_at: true`; without it the edit is refused
/// instead of leaving an end before the start.
#[sqlx::test(migrations = "./migrations")]
async fn n1_end_must_follow_start_and_can_be_cleared(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let event = free_event(
        &app,
        &org,
        json!({"starts_at":"2031-11-20T19:00:00Z","ends_at":"2031-11-20T23:00:00Z"}),
    )
    .await;
    let path = format!("/api/events/{event}");

    // Moving the start past the kept end: refused, nothing changes.
    let (s, body) = call(
        &app,
        "PUT",
        &path,
        Some(&org),
        Some(json!({"starts_at":"2031-11-27T19:00:00Z","ends_at":null})),
    )
    .await;
    assert_eq!(
        s,
        StatusCode::UNPROCESSABLE_ENTITY,
        "[N1] start moved past the kept end: {body}"
    );
    let (starts, ends): (chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>) =
        sqlx::query_as("SELECT starts_at, ends_at FROM events WHERE id = $1")
            .bind(event)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(starts.to_rfc3339(), "2031-11-20T19:00:00+00:00");
    assert!(ends.is_some());

    // The same edit with the explicit clear: the end is gone.
    let (s, ev) = call(
        &app,
        "PUT",
        &path,
        Some(&org),
        Some(json!({"starts_at":"2031-11-27T19:00:00Z","ends_at":null,"clear_ends_at":true})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{ev}");
    assert_eq!(ev["starts_at"], "2031-11-27T19:00:00Z");
    assert!(ev["ends_at"].is_null(), "{ev}");

    // An end before the start, or both a value and the clear flag: 422.
    let (s, _) = call(
        &app,
        "PUT",
        &path,
        Some(&org),
        Some(json!({"ends_at":"2031-11-27T18:00:00Z"})),
    )
    .await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "[N1] end before the kept start");
    let (s, _) = call(
        &app,
        "PUT",
        &path,
        Some(&org),
        Some(json!({"ends_at":"2031-11-28T01:00:00Z","clear_ends_at":true})),
    )
    .await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "value and clear together");
    let (s, ev) = call(
        &app,
        "PUT",
        &path,
        Some(&org),
        Some(json!({"ends_at":"2031-11-28T01:00:00Z"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(ev["ends_at"], "2031-11-28T01:00:00Z");

    // Create: end not after start → 422 (equal counts as not after).
    for end in ["2031-05-01T17:00:00Z", "2031-05-01T18:00:00Z"] {
        let (s, _) = call(
            &app,
            "POST",
            "/api/events",
            Some(&org),
            Some(
                json!({"title":"Gig","category":"Poetry","price":"0","tickets_total":5,"cost_type":"free",
                        "starts_at":"2031-05-01T18:00:00Z","ends_at":end}),
            ),
        )
        .await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "[N1] create with end {end}");
    }
}

/// N3: the gift preview carries the event's time zone.
#[sqlx::test(migrations = "./migrations")]
async fn n3_gift_preview_has_event_timezone(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let (u, ut) = user(&pool, "u@x.co", "Uma").await;
    let event = free_event(&app, &org, json!({"timezone":"Europe/Warsaw"})).await;
    assert_eq!(reserve(&app, &ut, event, 1).await.0, StatusCode::CREATED);
    let token = gift(&app, &ut, active_ids(&pool, u, event).await[0], "f@x.co").await;
    let (s, v) = call(&app, "GET", &format!("/api/gifts/{token}"), None, None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["event_timezone"], "Europe/Warsaw");
}

/// N4: code requests have their own per-IP bucket (60/h), and existing emails are not exempt.
#[sqlx::test(migrations = "./migrations")]
async fn n4_code_requests_have_their_own_ip_bucket(pool: PgPool) {
    let app = make_app(pool.clone());
    user(&pool, "known@x.co", "Kim").await;
    for i in 0..60 {
        let email = if i % 2 == 0 {
            "known@x.co".to_string()
        } else {
            format!("new{i}@x.co")
        };
        let (s, _) = send(
            &app,
            req(
                "POST",
                "/api/auth/email/request-code",
                None,
                Some("203.0.113.9"),
                Some(json!({ "email": email })),
            ),
        )
        .await;
        assert_eq!(s, StatusCode::NO_CONTENT, "request {i}");
    }
    for email in ["known@x.co", "brand-new@x.co"] {
        let (s, _) = send(
            &app,
            req(
                "POST",
                "/api/auth/email/request-code",
                None,
                Some("203.0.113.9"),
                Some(json!({ "email": email })),
            ),
        )
        .await;
        assert_eq!(
            s,
            StatusCode::TOO_MANY_REQUESTS,
            "[SEC] 61st from one IP, {email}: same answer"
        );
    }
    // Register from that IP is unaffected by the code bucket.
    assert_eq!(
        send(&app, register_req("fresh@x.co", "203.0.113.9")).await.0,
        StatusCode::CREATED
    );
}

/// N6: a gift to someone already holding 5 places is created like any other gift (the sender
/// learns nothing); the cap bites at claim time, for the claimer only.
#[sqlx::test(migrations = "./migrations")]
async fn n6_gift_reveals_nothing_and_the_cap_holds_at_claim(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let (u, ut) = user(&pool, "u@x.co", "Uma").await;
    let (v, vt) = user(&pool, "v@x.co", "Vik").await;
    let event = free_event(&app, &org, json!({})).await;
    assert_eq!(reserve(&app, &vt, event, 5).await.0, StatusCode::CREATED);
    assert_eq!(reserve(&app, &ut, event, 1).await.0, StatusCode::CREATED);
    let ticket = active_ids(&pool, u, event).await[0];
    let (s, g) = call(
        &app,
        "POST",
        &format!("/api/tickets/{ticket}/gift"),
        Some(&ut),
        Some(json!({"email":"v@x.co"})),
    )
    .await;
    assert_eq!(
        s,
        StatusCode::CREATED,
        "[PII] the gift does not reveal the recipient's holdings: {g}"
    );
    let (s, _) = call(
        &app,
        "POST",
        "/api/tickets/claim",
        Some(&vt),
        Some(json!({"token": g["claim_token"]})),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT, "[SEC] the cap holds at claim");
    assert_eq!(holdings(&pool, v, event).await, 5);
}

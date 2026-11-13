//! TASK-305 security findings from the live DEV probe (F1–F10, P). Each test names the finding it
//! guards; the security ones were mutation-checked (control removed → the test goes red).

use std::time::{Duration, Instant};

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
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

/// A user inserted directly (no password hashing, fast), with a valid bearer token.
async fn user(pool: &PgPool, email: &str, name: &str) -> (Uuid, String) {
    let id: Uuid = sqlx::query_scalar("INSERT INTO users (email, name) VALUES ($1, $2) RETURNING id")
        .bind(email)
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap();
    (id, underground_api::auth::issue_token(id, SECRET).unwrap())
}

async fn set_dob(pool: &PgPool, id: Uuid, dob: &str) {
    sqlx::query("UPDATE users SET dob = $2::date WHERE id = $1")
        .bind(id)
        .bind(dob)
        .execute(pool)
        .await
        .unwrap();
}

struct Resp {
    status: StatusCode,
    headers: axum::http::HeaderMap,
    text: String,
}

impl Resp {
    fn json(&self) -> Value {
        serde_json::from_str(&self.text).unwrap_or(Value::Null)
    }
}

async fn raw(app: &Router, req: Request<Body>) -> Resp {
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    Resp {
        status,
        headers,
        text: String::from_utf8_lossy(&bytes).into_owned(),
    }
}

fn request(method: &str, path: &str, token: Option<&str>, body: Option<String>, ip: Option<&str>) -> Request<Body> {
    let mut b = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    if let Some(ip) = ip {
        b = b.header("x-forwarded-for", ip);
    }
    b.body(body.map(Body::from).unwrap_or_else(Body::empty)).unwrap()
}

async fn call(app: &Router, method: &str, path: &str, token: Option<&str>, body: Option<Value>) -> (StatusCode, Value) {
    let r = raw(app, request(method, path, token, body.map(|v| v.to_string()), None)).await;
    (r.status, r.json())
}

/// The same call, the raw response text kept (to compare whole bodies).
async fn call_text(app: &Router, method: &str, path: &str, token: Option<&str>, body: Value) -> (StatusCode, String) {
    let r = raw(app, request(method, path, token, Some(body.to_string()), None)).await;
    (r.status, r.text)
}

async fn event(app: &Router, token: &str, extra: Value) -> Uuid {
    let mut body = json!({"title":"Gig","category":"Poetry","price":"0","tickets_total":30,"cost_type":"free"});
    for (k, v) in extra.as_object().unwrap() {
        body[k] = v.clone();
    }
    let (s, v) = call(app, "POST", "/api/events", Some(token), Some(body)).await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    v["id"].as_str().unwrap().parse().unwrap()
}

async fn reserve(app: &Router, token: &str, event: Uuid) -> Uuid {
    let (s, v) = call(app, "POST", &format!("/api/events/{event}/reserve"), Some(token), None).await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    v["id"].as_str().unwrap().parse().unwrap()
}

async fn owner_of(pool: &PgPool, ticket: Uuid) -> (Uuid, String) {
    sqlx::query_as("SELECT owner_id, status FROM tickets WHERE id = $1")
        .bind(ticket)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// The event started an hour ago (reviews open; booking closed). Setup for review tests (R4b:
/// an event without a start date takes no reviews).
async fn start_now(pool: &PgPool, event: Uuid) {
    sqlx::query("UPDATE events SET starts_at = now() - interval '1 hour', ends_at = NULL WHERE id = $1")
        .bind(event)
        .execute(pool)
        .await
        .unwrap();
}

/// An active place for `owner` at `event`, inserted directly (setup).
async fn give_place(pool: &PgPool, event: Uuid, owner: Uuid) {
    sqlx::query("INSERT INTO tickets (event_id, owner_id, qr_token, price, status) VALUES ($1, $2, 'UG-' || gen_random_uuid(), 0, 'active')")
        .bind(event)
        .bind(owner)
        .execute(pool)
        .await
        .unwrap();
}

async fn make_past(pool: &PgPool, event: Uuid) {
    sqlx::query(
        "UPDATE events SET starts_at = now() - interval '2 days', ends_at = now() - interval '1 day' WHERE id = $1",
    )
    .bind(event)
    .execute(pool)
    .await
    .unwrap();
}

// ------------------------------------------------------------------------------------- F2

/// F2: without owning the ticket, an unknown and a registered email get the same 404. The
/// owner's answers are covered by `task305_round2.rs` (R2: transfer is an offer, owner decision
/// 2026-11-14; this test's owner half used to expect one shared 422 refusal and a direct move).
#[sqlx::test(migrations = "./migrations")]
async fn f2_transfer_checks_the_ticket_before_the_recipient(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "org@x.co", "Org").await;
    let (owner, ot) = user(&pool, "owner@x.co", "Owner").await;
    let (_, stranger) = user(&pool, "stranger@x.co", "Stranger").await;
    let (minor, _) = user(&pool, "minor@x.co", "Minor").await;
    let (adult, _) = user(&pool, "adult@x.co", "Adult").await;
    set_dob(&pool, owner, "1990-01-01").await;
    set_dob(&pool, adult, "1991-01-01").await;
    set_dob(&pool, minor, "2015-01-01").await;
    let ev = event(&app, &org, json!({"age_rating":"18"})).await;
    let ticket = reserve(&app, &ot, ev).await;
    let path = format!("/api/tickets/{ticket}/transfer");

    // Not the caller's ticket: nothing about the email is looked at.
    let unknown = call_text(&app, "POST", &path, Some(&stranger), json!({"email":"nobody@x.co"})).await;
    let known = call_text(&app, "POST", &path, Some(&stranger), json!({"email":"adult@x.co"})).await;
    let minor_ = call_text(&app, "POST", &path, Some(&stranger), json!({"email":"minor@x.co"})).await;
    assert_eq!(unknown.0, StatusCode::NOT_FOUND, "[SEC] {}", unknown.1);
    assert_eq!(known, unknown, "[SEC] a registered email answers like an unknown one");
    assert_eq!(minor_, unknown, "[SEC] a minor answers like an unknown email");
    assert_eq!(owner_of(&pool, ticket).await, (owner, "active".into()));

    // The owner: "sent".
    let (s, v) = call(&app, "POST", &path, Some(&ot), Some(json!({"email":"nobody@x.co"}))).await;
    assert_eq!((s, v), (StatusCode::ACCEPTED, json!({"status":"sent"})));
}

/// F2 (event status): a gift is not claimed and a staff invite is not accepted for an event that
/// is cancelled or over; both stay pending.
#[sqlx::test(migrations = "./migrations")]
async fn f2_claim_and_accept_refuse_cancelled_or_finished_events(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "org@x.co", "Org").await;
    let (owner, ot) = user(&pool, "owner@x.co", "Owner").await;
    let (_, ft) = user(&pool, "friend@x.co", "Friend").await;
    for gone in ["cancelled", "past"] {
        let ev = event(&app, &org, json!({})).await;
        let ticket = reserve(&app, &ot, ev).await;
        let (s, g) = call(
            &app,
            "POST",
            &format!("/api/tickets/{ticket}/gift"),
            Some(&ot),
            Some(json!({"email":"friend@x.co"})),
        )
        .await;
        assert_eq!(s, StatusCode::CREATED, "{g}");
        let (s, inv) = call(
            &app,
            "POST",
            &format!("/api/events/{ev}/staff/invite"),
            Some(&org),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::CREATED, "{inv}");
        if gone == "cancelled" {
            let (s, _) = call(&app, "DELETE", &format!("/api/events/{ev}"), Some(&org), None).await;
            assert_eq!(s, StatusCode::NO_CONTENT);
        } else {
            make_past(&pool, ev).await;
        }
        let (s, v) = call(
            &app,
            "POST",
            "/api/tickets/claim",
            Some(&ft),
            Some(json!({"token": g["claim_token"]})),
        )
        .await;
        assert_eq!(s, StatusCode::CONFLICT, "[SEC] {gone}: claim refused: {v}");
        assert_eq!(owner_of(&pool, ticket).await, (owner, "pending_claim".into()));
        let (s, v) = call(
            &app,
            "POST",
            "/api/staff/accept",
            Some(&ft),
            Some(json!({"token": inv["invite_token"]})),
        )
        .await;
        assert_eq!(s, StatusCode::CONFLICT, "[SEC] {gone}: accept refused: {v}");
        let pending: i64 =
            sqlx::query_scalar("SELECT count(*) FROM staff_invites WHERE event_id = $1 AND status = 'pending'")
                .bind(ev)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(pending, 1, "{gone}: the invite stays pending");
    }
    // A live event: both still work.
    let ev = event(&app, &org, json!({})).await;
    let ticket = reserve(&app, &ot, ev).await;
    let (_, g) = call(
        &app,
        "POST",
        &format!("/api/tickets/{ticket}/gift"),
        Some(&ot),
        Some(json!({"email":"friend@x.co"})),
    )
    .await;
    let (s, _) = call(
        &app,
        "POST",
        "/api/tickets/claim",
        Some(&ft),
        Some(json!({"token": g["claim_token"]})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, inv) = call(
        &app,
        "POST",
        &format!("/api/events/{ev}/staff/invite"),
        Some(&org),
        None,
    )
    .await;
    let (s, _) = call(
        &app,
        "POST",
        "/api/staff/accept",
        Some(&ft),
        Some(json!({"token": inv["invite_token"]})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
}

// ------------------------------------------------------------------------------------- F1

/// F1: every free-text event field has a cap counted in characters; over it is a 422.
#[sqlx::test(migrations = "./migrations")]
async fn f1_event_text_is_capped_in_characters(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "org@x.co", "Org").await;
    let ok = event(
        &app,
        &org,
        json!({"title": "ї".repeat(200), "description": "я".repeat(10_000), "venue_address": "в".repeat(300),
               "city": "к".repeat(120), "category": "c".repeat(80)}),
    )
    .await;
    let too_long = [
        json!({"title": "a".repeat(201)}),
        json!({"description": "d".repeat(10_001)}),
        json!({"venue_address": "v".repeat(301)}),
        json!({"city": "c".repeat(121)}),
        json!({"category": "c".repeat(81)}),
        json!({"links": vec!["https://x.co"; 21]}),
        json!({"gallery_urls": vec!["/uploads/a.webp"; 21]}),
        json!({"links": [format!("https://x.co/{}", "p".repeat(2040))]}),
        json!({"poster_url": format!("https://x.co/{}", "p".repeat(2040))}),
    ];
    for extra in &too_long {
        let mut body = json!({"title":"Gig","category":"Poetry","price":"0","tickets_total":5,"cost_type":"free"});
        for (k, v) in extra.as_object().unwrap() {
            body[k] = v.clone();
        }
        let (s, _) = call(&app, "POST", "/api/events", Some(&org), Some(body)).await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "[SEC] create {extra}");
        let (s, _) = call(
            &app,
            "PUT",
            &format!("/api/events/{ok}"),
            Some(&org),
            Some(extra.clone()),
        )
        .await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "[SEC] edit {extra}");
    }
}

/// F1: creating and publishing events share one per-user budget (30 an hour).
#[sqlx::test(migrations = "./migrations")]
async fn f1_event_create_and_publish_are_rate_limited(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "org@x.co", "Org").await;
    let mut drafts = Vec::new();
    for _ in 0..3 {
        drafts.push(event(&app, &org, json!({"status":"draft"})).await);
    }
    for _ in 0..26 {
        event(&app, &org, json!({})).await;
    }
    let publish = |id: Uuid| format!("/api/events/{id}");
    let (s, _) = call(
        &app,
        "PUT",
        &publish(drafts[0]),
        Some(&org),
        Some(json!({"status":"published"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "the 30th: a publish");
    let body = json!({"title":"Gig","category":"Poetry","price":"0","tickets_total":5,"cost_type":"free"});
    let (s, v) = call(&app, "POST", "/api/events", Some(&org), Some(body.clone())).await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS, "[SEC] 31st create");
    assert_eq!(v["error"], "забагато спроб, спробуйте пізніше");
    let (s, _) = call(
        &app,
        "PUT",
        &publish(drafts[1]),
        Some(&org),
        Some(json!({"status":"published"})),
    )
    .await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS, "[SEC] a publish over the budget");
    let status: String = sqlx::query_scalar("SELECT status FROM events WHERE id = $1")
        .bind(drafts[1])
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "draft");
    // An ordinary edit of a published event is not a publish.
    let (s, _) = call(
        &app,
        "PUT",
        &publish(drafts[0]),
        Some(&org),
        Some(json!({"title":"New"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    // Another organiser is not affected.
    let (_, other) = user(&pool, "other@x.co", "Other").await;
    let (s, _) = call(&app, "POST", "/api/events", Some(&other), Some(body)).await;
    assert_eq!(s, StatusCode::CREATED);
}

/// F1: a long event title is cut when copied into notifications (follower, interest, reminder).
#[sqlx::test(migrations = "./migrations")]
async fn f1_notification_titles_are_truncated(pool: PgPool) {
    let app = make_app(pool.clone());
    let (org_id, org) = user(&pool, "org@x.co", "Org").await;
    let (fan, fan_t) = user(&pool, "fan@x.co", "Fan").await;
    let (local, _) = user(&pool, "local@x.co", "Local").await;
    let (s, _) = call(&app, "POST", &format!("/api/users/{org_id}/follow"), Some(&fan_t), None).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    sqlx::query("INSERT INTO user_prefs (user_id, categories) VALUES ($1, ARRAY['Poetry'])")
        .bind(local)
        .execute(&pool)
        .await
        .unwrap();
    let title = "Т".repeat(200);
    let soon = (chrono::Utc::now() + chrono::Duration::hours(2)).to_rfc3339();
    let ev = event(&app, &org, json!({"title": title, "starts_at": soon})).await;
    reserve(&app, &fan_t, ev).await;
    underground_api::reminders::sweep(&pool, 24).await.unwrap();
    let rows: Vec<(Uuid, String, String, Option<String>)> =
        sqlx::query_as("SELECT user_id, kind, title, body FROM notifications WHERE event_id = $1")
            .bind(ev)
            .fetch_all(&pool)
            .await
            .unwrap();
    let kinds: Vec<&str> = rows.iter().map(|r| r.1.as_str()).collect();
    // TASK-305 UX7 adds the organiser's `booking` alert (was: only fan and local got alerts).
    for kind in ["organizer_event", "event_match", "event_reminder", "booking"] {
        assert!(kinds.contains(&kind), "{kind} sent: {kinds:?}");
    }
    for (user, kind, title, body) in &rows {
        assert!(*user == fan || *user == local || (*user == org_id && kind == "booking"));
        for text in [Some(title), body.as_ref()].into_iter().flatten() {
            let t_count = text.chars().filter(|c| *c == 'Т').count();
            assert!(t_count <= 120, "[SEC] {kind}: {t_count} title characters copied");
        }
    }
}

// ------------------------------------------------------------------------------------- F3

/// F3: the avatar URL follows the event-image rule; a refused one leaves the stored value.
#[sqlx::test(migrations = "./migrations")]
async fn f3_avatar_url_must_be_safe(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, t) = user(&pool, "u@x.co", "U").await;
    // TASK-305 R4b: only our own uploads (was: any https URL too; see task305_round2.rs).
    for good in ["/uploads/a.webp"] {
        let (s, v) = call(&app, "PATCH", "/api/me", Some(&t), Some(json!({ "avatar_url": good }))).await;
        assert_eq!(s, StatusCode::OK, "{good}");
        assert_eq!(v["avatar_url"], good);
    }
    for bad in [
        "https://cdn.example.com/a.png",
        "javascript:alert(1)",
        "data:image/svg+xml,<svg onload=alert(1)>",
        "//evil.example/a.png",
        "/uploads/../secret",
        "/api/me",
        "https://",
    ] {
        let (s, _) = call(&app, "PATCH", "/api/me", Some(&t), Some(json!({ "avatar_url": bad }))).await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "[SEC] {bad}");
    }
    let (_, me) = call(&app, "GET", "/api/me", Some(&t), None).await;
    assert_eq!(me["avatar_url"], "/uploads/a.webp", "a refused avatar changes nothing");
}

// ------------------------------------------------------------------------------------- F4

async fn login(app: &Router, email: &str, ip: &str) -> StatusCode {
    let body = json!({"email": email, "password": "wrong-password"}).to_string();
    raw(app, request("POST", "/api/auth/login", None, Some(body), Some(ip)))
        .await
        .status
}

/// F4: one client IP gets 30 login attempts per 10 minutes across all emails.
#[sqlx::test(migrations = "./migrations")]
async fn f4_login_is_limited_per_ip(pool: PgPool) {
    let app = make_app(pool);
    for i in 0..30 {
        assert_eq!(
            login(&app, &format!("spray{i}@x.co"), "203.0.113.7").await,
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        login(&app, "spray-last@x.co", "203.0.113.7").await,
        StatusCode::TOO_MANY_REQUESTS,
        "[SEC] 31st attempt from one IP"
    );
    assert_eq!(
        login(&app, "spray-last@x.co", "203.0.113.8").await,
        StatusCode::UNAUTHORIZED
    );
}

async fn median_login(app: &Router, emails: &[String], ip_prefix: &str) -> Duration {
    let mut times = Vec::new();
    for (i, email) in emails.iter().enumerate() {
        let start = Instant::now();
        assert_eq!(
            login(app, email, &format!("{ip_prefix}.{i}")).await,
            StatusCode::UNAUTHORIZED
        );
        times.push(start.elapsed());
    }
    times.sort();
    times[times.len() / 2]
}

/// F4: a login for an account that does not exist, or has no password, runs the same argon2
/// verify as a wrong password, so it takes about as long (no account oracle by timing).
#[sqlx::test(migrations = "./migrations")]
async fn f4_unknown_account_takes_as_long_as_a_wrong_password(pool: PgPool) {
    let app = make_app(pool.clone());
    let hash = underground_api::auth::hash_password("right-password").unwrap();
    for i in 0..5 {
        sqlx::query("INSERT INTO users (email, name, password_hash) VALUES ($1, 'K', $2)")
            .bind(format!("known{i}@x.co"))
            .bind(&hash)
            .execute(&pool)
            .await
            .unwrap();
        user(&pool, &format!("nopass{i}@x.co"), "N").await;
    }
    // Warm-up: the dummy hash is made on first use.
    login(&app, "warm@x.co", "198.51.100.1").await;
    let emails = |p: &str| (0..5).map(|i| format!("{p}{i}@x.co")).collect::<Vec<_>>();
    let known = median_login(&app, &emails("known"), "192.0.2").await;
    let unknown = median_login(&app, &emails("ghost"), "198.51.100").await;
    let nopass = median_login(&app, &emails("nopass"), "203.0.113").await;
    assert!(
        unknown * 3 >= known && nopass * 3 >= known,
        "[SEC] known {known:?}, unknown {unknown:?}, no password {nopass:?}"
    );
}

// ------------------------------------------------------------------------------------- F5

/// F5: every API answer carries the security headers, `/uploads` files and errors included.
#[sqlx::test(migrations = "./migrations")]
async fn f5_api_sets_security_headers(pool: PgPool) {
    let app = make_app(pool);
    let expected = [
        ("strict-transport-security", "max-age=15552000"),
        ("x-frame-options", "DENY"),
        (
            "content-security-policy",
            "frame-ancestors 'none'; object-src 'none'; base-uri 'self'",
        ),
        ("x-content-type-options", "nosniff"),
        ("referrer-policy", "strict-origin-when-cross-origin"),
    ];
    for path in [
        "/health",
        "/api/events",
        "/api/me",
        "/uploads/missing.webp",
        "/api/nope",
    ] {
        let r = raw(&app, request("GET", path, None, None, None)).await;
        for (name, value) in expected {
            assert_eq!(
                r.headers.get(name).and_then(|v| v.to_str().ok()),
                Some(value),
                "[SEC] {path} ({}): {name}",
                r.status
            );
        }
    }
}

// ------------------------------------------------------------------------------------- F6

async fn dm(app: &Router, token: &str, to: Uuid, text: &str) -> (StatusCode, Value) {
    call(
        app,
        "POST",
        "/api/messages",
        Some(token),
        Some(json!({"recipient_id": to, "text": text})),
    )
    .await
}

/// F6: 30 messages a minute per sender, then the shared 429.
#[sqlx::test(migrations = "./migrations")]
async fn f6_messages_are_rate_limited_per_minute(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, a) = user(&pool, "a@x.co", "A").await;
    let (b, bt) = user(&pool, "b@x.co", "B").await;
    let (a_id, _) = user(&pool, "c@x.co", "C").await;
    for i in 0..30 {
        assert_eq!(dm(&app, &a, b, &format!("hi {i}")).await.0, StatusCode::CREATED);
    }
    let (s, v) = dm(&app, &a, b, "one more").await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS, "[SEC] 31st in a minute");
    assert_eq!(v["error"], "забагато спроб, спробуйте пізніше");
    assert_eq!(
        dm(&app, &bt, a_id, "reply").await.0,
        StatusCode::CREATED,
        "other senders unaffected"
    );
}

/// F6: 300 messages per rolling day per sender; older ones do not count.
#[sqlx::test(migrations = "./migrations")]
async fn f6_messages_are_capped_per_day(pool: PgPool) {
    let app = make_app(pool.clone());
    let (a, at) = user(&pool, "a@x.co", "A").await;
    let (b, bt) = user(&pool, "b@x.co", "B").await;
    for (sender, age) in [(a, "23 hours"), (b, "25 hours")] {
        sqlx::query(
            "INSERT INTO messages (sender_id, recipient_id, text, created_at) \
             SELECT $1, $2, 'old', now() - $3::interval FROM generate_series(1, 300)",
        )
        .bind(sender)
        .bind(if sender == a { b } else { a })
        .bind(age)
        .execute(&pool)
        .await
        .unwrap();
    }
    assert_eq!(
        dm(&app, &at, b, "301st today").await.0,
        StatusCode::TOO_MANY_REQUESTS,
        "[SEC]"
    );
    assert_eq!(
        dm(&app, &bt, a, "yesterday's do not count").await.0,
        StatusCode::CREATED
    );
}

/// F6: a message with nothing visible (whitespace, zero-width, bidi controls) is refused.
#[sqlx::test(migrations = "./migrations")]
async fn f6_blank_messages_are_refused(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, a) = user(&pool, "a@x.co", "A").await;
    let (b, _) = user(&pool, "b@x.co", "B").await;
    for text in [
        "\u{200B}",
        " \u{200C}\u{200D}\u{2060}\u{FEFF}\n\t",
        "\u{202E}\u{2066}",
        "\u{0007}",
    ] {
        assert_eq!(
            dm(&app, &a, b, text).await.0,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{text:?}"
        );
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM messages")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

// ------------------------------------------------------------------------------------- F7

/// F7: NUL in any text input is a 422 before the database (it used to be a 500).
#[sqlx::test(migrations = "./migrations")]
async fn f7_nul_is_refused_before_the_database(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, t) = user(&pool, "u@x.co", "U").await;
    let (b, _) = user(&pool, "b@x.co", "B").await;
    let event_body = json!({"title":"a\u{0}b","category":"Poetry","price":"0","tickets_total":5,"cost_type":"free"});
    let cases = [
        ("POST", "/api/events".to_string(), event_body),
        ("PATCH", "/api/me".to_string(), json!({"bio": "x\u{0}"})),
        (
            "POST",
            "/api/messages".to_string(),
            json!({"recipient_id": b, "text": "\u{0}"}),
        ),
        ("POST", "/api/posts".to_string(), json!({"text": "p\u{0}"})),
        (
            "POST",
            "/api/reports".to_string(),
            json!({"target_type":"user","target_id": b, "reason":"r\u{0}"}),
        ),
        (
            "POST",
            "/api/auth/register".to_string(),
            json!({"email":"n@x.co","password":"password1","name":"N\u{0}"}),
        ),
    ];
    for (method, path, body) in cases {
        let (s, v) = call(&app, method, &path, Some(&t), Some(body)).await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "[SEC] {method} {path}: {v}");
        assert_eq!(v["error"], underground_api::text::NUL_REFUSED);
    }
    for path in ["/api/events?q=%00", "/api/geo/search?q=a%00b", "/api/gifts/abc%00"] {
        let (s, _) = call(&app, "GET", path, None, None).await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "[SEC] {path}");
    }
    // The six characters "\u0000" typed as text are not a NUL.
    let (s, v) = dm(&app, &t, b, "literal \\u0000 text").await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(v["text"], "literal \\u0000 text");
}

// ------------------------------------------------------------------------------------- F8

/// F8: control and bidi characters are removed from names, titles and messages; newlines stay
/// in descriptions and messages.
#[sqlx::test(migrations = "./migrations")]
async fn f8_controls_and_bidi_are_stripped(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, t) = user(&pool, "u@x.co", "U").await;
    let (b, _) = user(&pool, "b@x.co", "B").await;
    let (_, me) = call(
        &app,
        "PATCH",
        "/api/me",
        Some(&t),
        Some(json!({"name":"Ali\u{202E}ce\u{0007}\u{0085}"})),
    )
    .await;
    assert_eq!(me["name"], "Alice");
    let ev = event(
        &app,
        &t,
        json!({"title":"Gig\u{202D}\u{2066}x\u{001B}", "description":"line 1\nline 2\u{202E}\u{0008}"}),
    )
    .await;
    let (_, e) = call(&app, "GET", &format!("/api/events/{ev}"), None, None).await;
    assert_eq!(e["title"], "Gigx");
    assert_eq!(e["description"], "line 1\nline 2");
    let (_, e) = call(
        &app,
        "PUT",
        &format!("/api/events/{ev}"),
        Some(&t),
        Some(json!({"title":"\u{202E}New"})),
    )
    .await;
    assert_eq!(e["title"], "New");
    let (_, m) = dm(&app, &t, b, "hi\u{202E}\nthere\u{0007}").await;
    assert_eq!(m["text"], "hi\nthere");
    let (s, _) = call(
        &app,
        "PATCH",
        "/api/me",
        Some(&t),
        Some(json!({"name":"\u{202E}\u{0007}"})),
    )
    .await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "a name of controls only is blank");
}

// ------------------------------------------------------------------------------------- F9

async fn review(app: &Router, token: &str, event: Uuid, rating: i32) -> (StatusCode, Value) {
    call(
        app,
        "POST",
        "/api/reviews",
        Some(token),
        Some(json!({"event_id": event, "rating": rating})),
    )
    .await
}

/// F9: only a guest with a live place reviews; never the organiser.
#[sqlx::test(migrations = "./migrations")]
async fn f9_only_guests_review(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "org@x.co", "Org").await;
    let (fan_id, fan) = user(&pool, "fan@x.co", "Fan").await;
    let (_, gone) = user(&pool, "gone@x.co", "Gone").await;
    let ev = event(&app, &org, json!({})).await;
    // Setup (R4b: reviews need a start date, and booking closes at the start): every place is
    // booked first, then the event starts; the fan's place is added after the "no ticket" check.
    reserve(&app, &org, ev).await;
    let returned = reserve(&app, &gone, ev).await;
    let (s, _) = call(
        &app,
        "POST",
        &format!("/api/tickets/{returned}/return"),
        Some(&gone),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    start_now(&pool, ev).await;
    assert_eq!(
        review(&app, &fan, ev, 5).await.0,
        StatusCode::UNPROCESSABLE_ENTITY,
        "[SEC] no ticket"
    );
    give_place(&pool, ev, fan_id).await;
    assert_eq!(review(&app, &fan, ev, 5).await.0, StatusCode::CREATED);
    assert_eq!(
        review(&app, &org, ev, 5).await.0,
        StatusCode::UNPROCESSABLE_ENTITY,
        "[SEC] organiser"
    );
    assert_eq!(
        review(&app, &gone, ev, 1).await.0,
        StatusCode::UNPROCESSABLE_ENTITY,
        "[SEC] returned"
    );
    let (_, list) = call(&app, "GET", &format!("/api/events/{ev}/reviews"), None, None).await;
    assert_eq!(list["count"], 1);
    // Not yet started: still refused with the earlier message.
    let soon = (chrono::Utc::now() + chrono::Duration::days(3)).to_rfc3339();
    let later = event(&app, &org, json!({"starts_at": soon})).await;
    reserve(&app, &fan, later).await;
    let (s, v) = review(&app, &fan, later, 5).await;
    assert_eq!(
        (s, v["error"].clone()),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!("відгуки відкриються після початку події")
        )
    );
}

/// F9: reviews of a cancelled event leave the organiser's rating.
#[sqlx::test(migrations = "./migrations")]
async fn f9_cancelled_events_leave_the_rating(pool: PgPool) {
    let app = make_app(pool.clone());
    let (org_id, org) = user(&pool, "org@x.co", "Org").await;
    let (_, fan) = user(&pool, "fan@x.co", "Fan").await;
    let good = event(&app, &org, json!({})).await;
    let bad = event(&app, &org, json!({})).await;
    for (ev, rating) in [(good, 5), (bad, 1)] {
        reserve(&app, &fan, ev).await;
        start_now(&pool, ev).await;
        assert_eq!(review(&app, &fan, ev, rating).await.0, StatusCode::CREATED);
    }
    let (_, p) = call(&app, "GET", &format!("/api/users/{org_id}"), None, None).await;
    assert_eq!(
        (p["rating_avg"].clone(), p["rating_count"].clone()),
        (json!(3.0), json!(2))
    );
    let (s, _) = call(&app, "DELETE", &format!("/api/events/{bad}"), Some(&org), None).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (_, p) = call(&app, "GET", &format!("/api/users/{org_id}"), None, None).await;
    assert_eq!(
        (p["rating_avg"].clone(), p["rating_count"].clone()),
        (json!(5.0), json!(1))
    );
}

// ------------------------------------------------------------------------------------ F10

/// F10: text caps count characters, so Cyrillic text gets the full length.
#[sqlx::test(migrations = "./migrations")]
async fn f10_caps_count_characters(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "org@x.co", "Org").await;
    let (_, t) = user(&pool, "u@x.co", "U").await;
    let (b, _) = user(&pool, "b@x.co", "B").await;
    let ok = |s: StatusCode| assert!(s.is_success(), "{s}");
    let (s, _) = call(
        &app,
        "PATCH",
        "/api/me",
        Some(&t),
        Some(json!({"name": "ї".repeat(80), "bio": "б".repeat(500)})),
    )
    .await;
    ok(s);
    let (s, _) = call(
        &app,
        "PATCH",
        "/api/me",
        Some(&t),
        Some(json!({"name": "ї".repeat(81)})),
    )
    .await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    let (s, _) = call(
        &app,
        "PATCH",
        "/api/me",
        Some(&t),
        Some(json!({"bio": "б".repeat(501)})),
    )
    .await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    ok(dm(&app, &t, b, &"я".repeat(2000)).await.0);
    assert_eq!(
        dm(&app, &t, b, &"я".repeat(2001)).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let (s, v) = call(
        &app,
        "POST",
        "/api/posts",
        Some(&t),
        Some(json!({"text": "п".repeat(2000)})),
    )
    .await;
    ok(s);
    let post = v["id"].as_str().unwrap().to_string();
    let (s, _) = call(
        &app,
        "POST",
        &format!("/api/posts/{post}/comments"),
        Some(&t),
        Some(json!({"text": "к".repeat(2000)})),
    )
    .await;
    ok(s);
    let ev = event(&app, &org, json!({})).await;
    reserve(&app, &t, ev).await;
    start_now(&pool, ev).await;
    let (s, _) = call(
        &app,
        "POST",
        "/api/reviews",
        Some(&t),
        Some(json!({"event_id": ev, "rating": 4, "text": "в".repeat(2000)})),
    )
    .await;
    ok(s);
}

// --------------------------------------------------------------------------------------- P

/// P (TASK-305 R1 rule, docs/decisions.md 2026-11-14): an account whose stored name is blank or
/// «Гість» (any case, spaces around) shows as «Гість» to everyone else, on every surface that
/// shows a name; a name the person typed is shown even when it equals their email's local part;
/// no email ever reaches the other user. (Was: a name equal to the email's local part or the whole
/// email showed as «Гість».)
#[sqlx::test(migrations = "./migrations")]
async fn p_placeholder_names_never_show_the_email(pool: PgPool) {
    let app = make_app(pool.clone());
    let (jane, jt) = user(&pool, "jane.doe@x.co", "").await;
    let (jim, jimt) = user(&pool, "jim.roe@x.co", " ГІСТЬ ").await;
    let (kyiv, kt) = user(&pool, "kyivjazz@x.co", "kyivjazz").await;
    let (viewer, vt) = user(&pool, "viewer@x.co", "Viewer").await;
    let (_, org) = user(&pool, "org@x.co", "Org").await;
    let ev = event(&app, &org, json!({})).await;
    let mut seen = String::new();
    // Placeholder accounts, and one with a typed name equal to its local part, act toward the viewer.
    for (id, t) in [(jane, &jt), (jim, &jimt), (kyiv, &kt)] {
        call(&app, "POST", &format!("/api/users/{viewer}/follow"), Some(t), None).await;
        call(&app, "POST", &format!("/api/users/{id}/follow"), Some(&vt), None).await;
        assert_eq!(dm(&app, t, viewer, "hello").await.0, StatusCode::CREATED);
        call(&app, "POST", "/api/posts", Some(t), Some(json!({"text":"my wall"}))).await;
        reserve(&app, t, ev).await;
        call(&app, "POST", &format!("/api/events/{ev}/save"), Some(t), None).await;
    }
    let gift_ticket = reserve(&app, &jt, ev).await;
    // Reviews open once the event has started (R4b: never without a start date).
    start_now(&pool, ev).await;
    for t in [&jt, &jimt, &kt] {
        assert_eq!(review(&app, t, ev, 4).await.0, StatusCode::CREATED);
    }
    let (_, vpost) = call(
        &app,
        "POST",
        "/api/posts",
        Some(&vt),
        Some(json!({"text":"viewer post"})),
    )
    .await;
    let vpost = vpost["id"].as_str().unwrap().to_string();
    call(
        &app,
        "POST",
        &format!("/api/posts/{vpost}/comments"),
        Some(&jt),
        Some(json!({"text":"nice"})),
    )
    .await;
    let (_, gift) = call(
        &app,
        "POST",
        &format!("/api/tickets/{gift_ticket}/gift"),
        Some(&jt),
        Some(json!({"email":"viewer@x.co"})),
    )
    .await;
    // What the viewer (and an anonymous visitor) sees.
    let mut views = vec![
        format!("/api/users/{jane}"),
        format!("/api/users/{jim}"),
        format!("/api/users/{viewer}/followers"),
        format!("/api/users/{viewer}/following"),
        format!("/api/users/{jane}/posts"),
        format!("/api/posts/{vpost}/comments"),
        format!("/api/events/{ev}/reviews"),
        format!("/api/events/{ev}/social"),
        format!("/api/gifts/{}", gift["claim_token"].as_str().unwrap()),
        "/api/me/conversations".into(),
        "/api/me/notifications".into(),
        "/api/feed".into(),
    ];
    views.push(format!("/api/users/{jim}/posts"));
    for path in &views {
        let (s, v) = call(&app, "GET", path, Some(&vt), None).await;
        assert_eq!(s, StatusCode::OK, "{path}");
        let text = v.to_string();
        assert!(text.contains("Гість"), "{path} shows the fallback name: {text}");
        seen.push_str(&text);
    }
    let (s, jim_profile) = call(&app, "GET", &format!("/api/users/{jim}"), None, None).await;
    assert_eq!((s, jim_profile["name"].clone()), (StatusCode::OK, json!("Гість")));
    // The typed name is shown where the account appears.
    for path in [
        format!("/api/users/{kyiv}"),
        format!("/api/users/{kyiv}/posts"),
        format!("/api/users/{viewer}/followers"),
        format!("/api/events/{ev}/reviews"),
        "/api/me/conversations".into(),
    ] {
        let (_, v) = call(&app, "GET", &path, Some(&vt), None).await;
        let text = v.to_string();
        assert!(text.contains("\"kyivjazz\""), "{path} shows the chosen name: {text}");
        seen.push_str(&text);
    }
    let lower = seen.to_lowercase();
    for leak in ["jane", "doe", "jim", "roe", "@x.co"] {
        assert!(!lower.contains(leak), "[SEC] '{leak}' reached another user: {seen}");
    }
}

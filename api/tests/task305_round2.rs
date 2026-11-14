//! TASK-305 fix round 2 (docs/decisions.md 2026-11-14): display names (R1), transfer as an offer
//! (R2), telemetry limits (R3), which reviews count (R4), own-upload avatars (R4b), text cleaning
//! (R5), gifts of gone events (R6), the NUL scan (R7), no emails in logs (R10), the door check-in
//! window (UX2) and the new alerts (UX7). Each test names its item; the security ones were
//! mutation-checked (control removed → the test goes red).

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

async fn raw(app: &Router, req: Request<Body>) -> (StatusCode, String) {
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

async fn call(app: &Router, method: &str, path: &str, token: Option<&str>, body: Option<Value>) -> (StatusCode, Value) {
    let (s, t) = raw(app, request(method, path, token, body.map(|v| v.to_string()), None)).await;
    (s, serde_json::from_str(&t).unwrap_or(Value::Null))
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

async fn reserve_n(app: &Router, token: &str, event: Uuid, n: i64) -> Uuid {
    let (s, v) = call(
        app,
        "POST",
        &format!("/api/events/{event}/reserve"),
        Some(token),
        Some(json!({ "quantity": n })),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    v["id"].as_str().unwrap().parse().unwrap()
}

async fn reserve(app: &Router, token: &str, event: Uuid) -> Uuid {
    reserve_n(app, token, event, 1).await
}

async fn ticket_state(pool: &PgPool, ticket: Uuid) -> (Uuid, String) {
    sqlx::query_as("SELECT owner_id, status FROM tickets WHERE id = $1")
        .bind(ticket)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn start_now(pool: &PgPool, event: Uuid) {
    sqlx::query("UPDATE events SET starts_at = now() - interval '1 hour', ends_at = NULL WHERE id = $1")
        .bind(event)
        .execute(pool)
        .await
        .unwrap();
}

async fn alerts(app: &Router, token: &str) -> Vec<Value> {
    let (s, v) = call(app, "GET", "/api/me/notifications", Some(token), None).await;
    assert_eq!(s, StatusCode::OK);
    v.as_array().unwrap().clone()
}

/// The claim token in the newest place-offer alert's link, if the offer is open.
async fn offer_token(app: &Router, token: &str) -> Option<String> {
    alerts(app, token)
        .await
        .into_iter()
        .find(|n| n["kind"] == "place_offer")
        .and_then(|n| {
            n["link"]
                .as_str()
                .map(|l| l.strip_prefix("/g/").expect("claim link").to_string())
        })
}

async fn claim(app: &Router, token: &str, claim: &str) -> (StatusCode, Value) {
    call(
        app,
        "POST",
        "/api/tickets/claim",
        Some(token),
        Some(json!({ "token": claim })),
    )
    .await
}

async fn my_tickets(app: &Router, token: &str) -> Vec<Value> {
    let (_, v) = call(app, "GET", "/api/me/tickets", Some(token), None).await;
    v.as_array().unwrap().clone()
}

// ------------------------------------------------------------------------------------- R1

/// R1: one copy of the display-name rule. Every query takes it from `users::display_name_sql!` /
/// `DISPLAY_NAME_SQL`; nothing reads the old generated column.
#[test]
fn r1_display_name_rule_has_one_source() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
    let mut copies = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        assert!(!text.contains("u.name_is_placeholder"), "{name} reads the old column");
        if text.contains("THEN 'Гість'") {
            copies.push(name);
        }
    }
    assert_eq!(copies, ["users.rs"], "the rule is spelled out only in users.rs");
}

/// R1: «Гість» only for a blank name or «Гість» itself (trimmed, any case); everything else as
/// typed, a name equal to the email's local part included.
#[sqlx::test(migrations = "./migrations")]
async fn r1_guest_only_for_blank_or_guest_names(pool: PgPool) {
    let app = make_app(pool.clone());
    let cases = [
        ("a@x.co", "", "Гість"),
        ("b@x.co", "   ", "Гість"),
        ("c@x.co", "гість", "Гість"),
        ("d@x.co", " ГІСТЬ ", "Гість"),
        ("e@x.co", "гІсТь", "Гість"),
        ("kyivjazz@x.co", "kyivjazz", "kyivjazz"),
        ("f@x.co", "F@X.CO", "F@X.CO"),
        ("g@x.co", "Гість Петро", "Гість Петро"),
        ("h@x.co", "Guest", "Guest"),
    ];
    for (email, name, shown) in cases {
        let (id, _) = user(&pool, email, name).await;
        let (s, v) = call(&app, "GET", &format!("/api/users/{id}"), None, None).await;
        assert_eq!((s, v["name"].clone()), (StatusCode::OK, json!(shown)), "{name:?}");
    }
}

/// R1 backfill (migration 0052): rows from before TASK-302 whose name the old rule matched become
/// «Гість»; a typed name and anything newer stay. Re-running the migration changes nothing more.
#[sqlx::test(migrations = "./migrations")]
async fn r1_backfill_renames_only_old_auto_names(pool: PgPool) {
    let rows = [
        ("old.local@x.co", "old.local", "2026-11-01", "Гість"),
        ("old.whole@x.co", "Old.Whole@x.co", "2026-11-11T23:59:00Z", "Гість"),
        ("old.blank@x.co", " ", "2026-10-01", "Гість"),
        ("old.typed@x.co", "Olena", "2026-11-01", "Olena"),
        ("new.local@x.co", "new.local", "2026-11-12T00:00:01Z", "new.local"),
        // Round 3 (B5): a password account typed its name, even one equal to its local part.
        ("old.pw@x.co", "old.pw", "2026-11-01", "old.pw"),
    ];
    for (email, name, at, _) in rows {
        let password = email.starts_with("old.pw").then_some("$argon2id$typed");
        sqlx::query("INSERT INTO users (email, name, created_at, password_hash) VALUES ($1, $2, $3::timestamptz, $4)")
            .bind(email)
            .bind(name)
            .bind(at)
            .bind(password)
            .execute(&pool)
            .await
            .unwrap();
    }
    for _ in 0..2 {
        sqlx::raw_sql(include_str!("../migrations/0052_names_offers_alerts.sql"))
            .execute(&pool)
            .await
            .unwrap();
    }
    for (email, _, _, want) in rows {
        let name: String = sqlx::query_scalar("SELECT name FROM users WHERE email = $1")
            .bind(email)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(name, want, "{email}");
    }
}

// ------------------------------------------------------------------------------------- R2

/// R2: after the ownership check the sender's answer is the same 202 body for every valid address —
/// unknown, a minor, an adult, the sender — and takes about as long. (Malformed addresses: B1.)
#[sqlx::test(migrations = "./migrations")]
async fn r2_transfer_answer_never_depends_on_the_email(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "org@x.co", "Org").await;
    let (owner, ot) = user(&pool, "owner@x.co", "Owner").await;
    let (minor, _) = user(&pool, "minor@x.co", "Minor").await;
    user(&pool, "nodob@x.co", "NoDob").await;
    let (adult, _) = user(&pool, "adult@x.co", "Adult").await;
    set_dob(&pool, owner, "1990-01-01").await;
    set_dob(&pool, adult, "1991-01-01").await;
    set_dob(&pool, minor, "2015-01-01").await;
    let ev = event(&app, &org, json!({"age_rating":"18"})).await;
    let ticket = reserve(&app, &ot, ev).await;
    let path = format!("/api/tickets/{ticket}/transfer");
    let send = |email: &str| {
        let req = request(
            "POST",
            &path,
            Some(&ot),
            Some(json!({ "email": email }).to_string()),
            None,
        );
        let app = app.clone();
        async move {
            let started = Instant::now();
            let out = raw(&app, req).await;
            (out, started.elapsed())
        }
    };
    let cancel_path = format!("/api/tickets/{ticket}/gift");
    let cancel = || call(&app, "DELETE", &cancel_path, Some(&ot), None);

    let emails = ["nobody@x.co", "minor@x.co", "nodob@x.co", "adult@x.co", "ADULT@x.co "];
    // The sender's own address is refused (owner decision 2026-11-14), so it is not one of the
    // emails whose answer must match; it costs one call from the bucket.
    let ((s, body), _) = send("owner@x.co").await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(ticket_state(&pool, ticket).await, (owner, "active".into()), "own email");
    // Every email once (5), then ten timed pairs (20), plus the call above: 26 calls, inside the
    // 30/h sender bucket.
    let mut times = std::collections::HashMap::<&str, Vec<Duration>>::new();
    let timed = std::iter::repeat_n(["nobody@x.co", "adult@x.co"], 10).flatten();
    for (i, email) in emails.into_iter().chain(timed).enumerate() {
        let ((s, body), took) = send(email).await;
        assert_eq!(
            (s, body.as_str()),
            (StatusCode::ACCEPTED, r#"{"status":"sent"}"#),
            "[SEC] {email:?}"
        );
        assert_eq!(
            ticket_state(&pool, ticket).await,
            (owner, "pending_claim".into()),
            "{email:?}"
        );
        assert_eq!(cancel().await.0, StatusCode::OK, "the sender cancels the offer");
        if i >= emails.len() {
            times.entry(email).or_default().push(took);
        }
    }
    let median = |e: &str| {
        let mut t = times[e].clone();
        t.sort();
        t[t.len() / 2]
    };
    let (unknown, known) = (median("nobody@x.co"), median("adult@x.co"));
    assert!(
        unknown * 3 >= known && known * 3 >= unknown,
        "[SEC] unknown {unknown:?} vs account {known:?}"
    );
    assert_eq!(ticket_state(&pool, ticket).await, (owner, "active".into()));
}

/// R2: the recipient's list is unchanged until they accept; the alert carries the claim link; the
/// sender sees the offer as a pending gift; accepting moves the ticket.
#[sqlx::test(migrations = "./migrations")]
async fn r2_offer_moves_the_ticket_only_on_acceptance(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "org@x.co", "Org").await;
    let (owner, ot) = user(&pool, "owner@x.co", "Olha").await;
    let (friend, ft) = user(&pool, "friend@x.co", "Friend").await;
    let ev = event(&app, &org, json!({"title":"Jazz night"})).await;
    let ticket = reserve(&app, &ot, ev).await;
    let (s, _) = call(
        &app,
        "POST",
        &format!("/api/tickets/{ticket}/transfer"),
        Some(&ot),
        Some(json!({"email":"Friend@x.co"})),
    )
    .await;
    assert_eq!(s, StatusCode::ACCEPTED);
    assert!(
        my_tickets(&app, &ft).await.is_empty(),
        "[SEC] nothing lands in a stranger's list"
    );
    let mine = my_tickets(&app, &ot).await;
    assert_eq!(
        (mine[0]["id"].clone(), mine[0]["status"].clone()),
        (json!(ticket), json!("pending_claim")),
        "the sender sees the pending offer"
    );
    let list = alerts(&app, &ft).await;
    assert_eq!(list.len(), 1, "{list:?}");
    assert_eq!(list[0]["kind"], "place_offer");
    assert_eq!(list[0]["title"], "Olha пропонує вам місце");
    assert_eq!(list[0]["body"], "Jazz night");
    assert_eq!(list[0]["event_id"], json!(ev));
    let token = offer_token(&app, &ft).await.expect("claim link");
    let (s, preview) = call(&app, "GET", &format!("/api/gifts/{token}"), None, None).await;
    assert_eq!((s, preview["from_name"].clone()), (StatusCode::OK, json!("Olha")));
    // Nothing for the sender: no alert to themself.
    assert!(alerts(&app, &ot).await.is_empty());
    let (s, v) = claim(&app, &ft, &token).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(ticket_state(&pool, ticket).await, (friend, "active".into()));
    assert!(my_tickets(&app, &ot).await.is_empty());
    assert_eq!(
        offer_token(&app, &ft).await,
        None,
        "the link goes once the offer is taken"
    );
    let _ = owner;
}

/// R2: a minor accepting an 18+ offer is refused at the claim (only they see it); the ticket
/// stays the sender's, still on offer.
#[sqlx::test(migrations = "./migrations")]
async fn r2_minor_is_refused_at_acceptance(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "org@x.co", "Org").await;
    let (owner, ot) = user(&pool, "owner@x.co", "Owner").await;
    let (minor, mt) = user(&pool, "minor@x.co", "Minor").await;
    set_dob(&pool, owner, "1990-01-01").await;
    set_dob(&pool, minor, "2015-01-01").await;
    let ev = event(&app, &org, json!({"age_rating":"18"})).await;
    let ticket = reserve(&app, &ot, ev).await;
    let (s, _) = call(
        &app,
        "POST",
        &format!("/api/tickets/{ticket}/transfer"),
        Some(&ot),
        Some(json!({"email":"minor@x.co"})),
    )
    .await;
    assert_eq!(s, StatusCode::ACCEPTED);
    let token = offer_token(&app, &mt).await.expect("the minor is told about the offer");
    let (s, v) = claim(&app, &mt, &token).await;
    assert_eq!(
        (s, v["error"].clone()),
        (
            StatusCode::CONFLICT,
            json!("ви не відповідаєте віковому обмеженню події")
        ),
        "[SEC]"
    );
    assert_eq!(ticket_state(&pool, ticket).await, (owner, "pending_claim".into()));
    assert!(my_tickets(&app, &mt).await.is_empty());
}

/// R2: the sender cancels an offer like a gift; the recipient's link goes and the token is dead.
#[sqlx::test(migrations = "./migrations")]
async fn r2_offer_can_be_revoked(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "org@x.co", "Org").await;
    let (owner, ot) = user(&pool, "owner@x.co", "Owner").await;
    let (_, ft) = user(&pool, "friend@x.co", "Friend").await;
    let ev = event(&app, &org, json!({})).await;
    let ticket = reserve(&app, &ot, ev).await;
    call(
        &app,
        "POST",
        &format!("/api/tickets/{ticket}/transfer"),
        Some(&ot),
        Some(json!({"email":"friend@x.co"})),
    )
    .await;
    let token = offer_token(&app, &ft).await.unwrap();
    let (s, t) = call(&app, "DELETE", &format!("/api/tickets/{ticket}/gift"), Some(&ot), None).await;
    assert_eq!((s, t["status"].clone()), (StatusCode::OK, json!("active")));
    assert_eq!(offer_token(&app, &ft).await, None, "no link to a cancelled offer");
    assert_eq!(
        claim(&app, &ft, &token).await.0,
        StatusCode::NOT_FOUND,
        "[SEC] dead token"
    );
    assert_eq!(ticket_state(&pool, ticket).await, (owner, "active".into()));
}

// ------------------------------------------------------------------------------------ UX7

/// UX7: a gift to an account's email alerts that account with the claim link (the same link the
/// sender got); to an unknown email or to oneself, no alert. Chat messages make no alerts.
#[sqlx::test(migrations = "./migrations")]
async fn ux7_gift_offers_and_chats(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "org@x.co", "Org").await;
    let (_, ot) = user(&pool, "owner@x.co", "Owner").await;
    let (friend, ft) = user(&pool, "friend@x.co", "Friend").await;
    let ev = event(&app, &org, json!({})).await;
    let first = reserve_n(&app, &ot, ev, 3).await;
    let ids: Vec<Uuid> = my_tickets(&app, &ot)
        .await
        .iter()
        .map(|t| t["id"].as_str().unwrap().parse().unwrap())
        .collect();
    assert!(ids.contains(&first));
    let gift = |t: Uuid, email: &'static str| {
        let (app, ot) = (app.clone(), ot.clone());
        async move {
            call(
                &app,
                "POST",
                &format!("/api/tickets/{t}/gift"),
                Some(&ot),
                Some(json!({ "email": email })),
            )
            .await
        }
    };
    let (s, g) = gift(ids[0], "friend@x.co").await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(offer_token(&app, &ft).await.as_deref(), g["claim_token"].as_str());
    assert_eq!(gift(ids[1], "nobody@x.co").await.0, StatusCode::CREATED);
    // Owner decision 2026-11-14: a gift to one's own email is refused (was 201).
    assert_eq!(
        gift(ids[2], "owner@x.co").await,
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({ "error": underground_api::tickets::OFFER_TO_SELF_REFUSED })
        )
    );
    assert_eq!(alerts(&app, &ft).await.len(), 1);
    assert!(alerts(&app, &ot).await.is_empty(), "no alert to oneself");
    let (s, _) = call(
        &app,
        "POST",
        "/api/messages",
        Some(&ot),
        Some(json!({"recipient_id": friend, "text": "hi"})),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(alerts(&app, &ft).await.len(), 1, "a chat message is not an alert");
}

/// UX7: a booking alerts the organiser with a short title and the manage-page link; the
/// organiser booking their own event does not.
#[sqlx::test(migrations = "./migrations")]
async fn ux7_booking_alerts_the_organiser(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "org@x.co", "Org").await;
    let (_, fan) = user(&pool, "fan@x.co", "Fan").await;
    let long = "Д".repeat(200);
    let ev = event(&app, &org, json!({ "title": long })).await;
    reserve_n(&app, &fan, ev, 2).await;
    reserve(&app, &org, ev).await;
    let list = alerts(&app, &org).await;
    assert_eq!(list.len(), 1, "{list:?}");
    let n = &list[0];
    assert_eq!(n["kind"], "booking");
    assert_eq!(n["link"], format!("/organizer/{ev}"));
    assert_eq!(n["event_id"], json!(ev));
    assert_eq!(n["body"], "Місць: 2");
    let title = n["title"].as_str().unwrap();
    assert!(title.starts_with("Нове бронювання: ДДД"), "{title}");
    assert!(title.ends_with('…') && title.chars().count() <= 140, "{title}");
    assert!(alerts(&app, &fan).await.is_empty());
}

// ------------------------------------------------------------------------------------- R3

/// R3: the telemetry endpoint caps the body (413), `props` (422) and calls per IP (429).
#[sqlx::test(migrations = "./migrations")]
async fn r3_telemetry_is_bounded(pool: PgPool) {
    let app = make_app(pool.clone());
    let post = |body: String, ip: &str| raw(&app, request("POST", "/api/telemetry", None, Some(body), Some(ip)));
    let big = json!({"name":"x","props":{"p":"a".repeat(17 * 1024)}}).to_string();
    assert_eq!(
        post(big, "192.0.2.1").await.0,
        StatusCode::PAYLOAD_TOO_LARGE,
        "[SEC] body cap"
    );
    let wide = json!({"name":"x","props":{"p":"a".repeat(3000)}}).to_string();
    assert_eq!(
        post(wide, "192.0.2.2").await.0,
        StatusCode::UNPROCESSABLE_ENTITY,
        "[SEC] props cap"
    );
    let ok = json!({"name":"share","props":{"p":"a".repeat(1500)}}).to_string();
    assert_eq!(post(ok.clone(), "192.0.2.2").await.0, StatusCode::NO_CONTENT);
    let stored: i64 = sqlx::query_scalar("SELECT count(*) FROM telemetry_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, 1, "refused events are not stored");
    for i in 0..119 {
        assert_eq!(post(ok.clone(), "198.51.100.9").await.0, StatusCode::NO_CONTENT, "{i}");
    }
    assert_eq!(
        post(ok.clone(), "198.51.100.9").await.0,
        StatusCode::NO_CONTENT,
        "120th"
    );
    assert_eq!(
        post(ok.clone(), "198.51.100.9").await.0,
        StatusCode::TOO_MANY_REQUESTS,
        "[SEC] 121st in a minute"
    );
    assert_eq!(post(ok, "198.51.100.10").await.0, StatusCode::NO_CONTENT, "another IP");
}

// ------------------------------------------------------------------------------------- R4

async fn review(app: &Router, token: &str, event: Uuid, rating: i32, text: Option<&str>) -> (StatusCode, Value) {
    call(
        app,
        "POST",
        "/api/reviews",
        Some(token),
        Some(json!({"event_id": event, "rating": rating, "text": text})),
    )
    .await
}

/// R4a: a review counts (event list, its average, the organiser's rating) only while its author
/// holds an active or used place at the event.
#[sqlx::test(migrations = "./migrations")]
async fn r4a_reviews_count_only_with_a_live_place(pool: PgPool) {
    let app = make_app(pool.clone());
    let (org_id, org) = user(&pool, "org@x.co", "Org").await;
    let (_, a) = user(&pool, "a@x.co", "A").await;
    let (_, b) = user(&pool, "b@x.co", "B").await;
    let (_, c) = user(&pool, "c@x.co", "C").await;
    let ev = event(&app, &org, json!({})).await;
    let ta = reserve(&app, &a, ev).await;
    reserve(&app, &b, ev).await;
    let tc = reserve(&app, &c, ev).await;
    start_now(&pool, ev).await;
    for (t, r) in [(&a, 1), (&b, 5), (&c, 3)] {
        assert_eq!(review(&app, t, ev, r, None).await.0, StatusCode::CREATED);
    }
    let (_, list) = call(&app, "GET", &format!("/api/events/{ev}/reviews"), None, None).await;
    assert_eq!(
        (list["count"].clone(), list["avg_rating"].clone()),
        (json!(3), json!(3.0))
    );
    // A returns the place; C puts theirs on offer (pending, not live).
    assert_eq!(
        call(&app, "POST", &format!("/api/tickets/{ta}/return"), Some(&a), None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            "POST",
            &format!("/api/tickets/{tc}/gift"),
            Some(&c),
            Some(json!({"email":"z@x.co"}))
        )
        .await
        .0,
        StatusCode::CREATED
    );
    let (_, list) = call(&app, "GET", &format!("/api/events/{ev}/reviews"), None, None).await;
    assert_eq!(
        (list["count"].clone(), list["avg_rating"].clone()),
        (json!(1), json!(5.0)),
        "[SEC] {list}"
    );
    assert_eq!(list["reviews"].as_array().unwrap().len(), 1);
    let (_, p) = call(&app, "GET", &format!("/api/users/{org_id}"), None, None).await;
    assert_eq!(
        (p["rating_avg"].clone(), p["rating_count"].clone()),
        (json!(5.0), json!(1)),
        "[SEC]"
    );
}

/// R4b: an event without a start date takes no reviews.
#[sqlx::test(migrations = "./migrations")]
async fn r4b_no_reviews_without_a_start(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "org@x.co", "Org").await;
    let (_, fan) = user(&pool, "fan@x.co", "Fan").await;
    let ev = event(&app, &org, json!({})).await;
    reserve(&app, &fan, ev).await;
    let (s, v) = review(&app, &fan, ev, 5, None).await;
    assert_eq!(
        (s, v["error"].clone()),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!("у події немає дати початку — відгуки недоступні")
        ),
        "[SEC]"
    );
}

/// R4b: an avatar is one of our own uploads, relative or under `PUBLIC_BASE_URL`.
#[sqlx::test(migrations = "./migrations")]
async fn r4b_avatar_only_from_our_uploads(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, t) = user(&pool, "u@x.co", "U").await;
    let base = underground_api::uploads::public_base_url();
    let absolute = format!("{}/uploads/0d6c-480.webp", base.trim_end_matches('/'));
    for good in ["/uploads/a.webp", absolute.as_str()] {
        let (s, v) = call(&app, "PATCH", "/api/me", Some(&t), Some(json!({ "avatar_url": good }))).await;
        assert_eq!((s, v["avatar_url"].clone()), (StatusCode::OK, json!(good)));
    }
    for bad in [
        "https://cdn.example.com/a.png",
        "https://evil.example/uploads/a.webp",
        "/uploads/a.webp?x",
    ] {
        let (s, v) = call(&app, "PATCH", "/api/me", Some(&t), Some(json!({ "avatar_url": bad }))).await;
        assert_eq!(
            (s, v["error"].clone()),
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                json!(underground_api::auth::AVATAR_REFUSED)
            ),
            "[SEC] {bad}"
        );
    }
}

// ------------------------------------------------------------------------------------- R5

/// R5: post, comment, review and report text lose controls and bidi controls; whitespace or
/// zero-width only is refused (or, for an optional text, dropped).
#[sqlx::test(migrations = "./migrations")]
async fn r5_text_is_cleaned(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "org@x.co", "Org").await;
    let (b, t) = user(&pool, "u@x.co", "U").await;
    let blank = "\u{200B} \u{FEFF}\u{2060}";
    let dirty = "a\u{202E}b\u{0007}\nc";
    let (s, _) = call(&app, "POST", "/api/posts", Some(&t), Some(json!({ "text": blank }))).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "blank post");
    let (s, post) = call(&app, "POST", "/api/posts", Some(&t), Some(json!({ "text": dirty }))).await;
    assert_eq!((s, post["text"].clone()), (StatusCode::CREATED, json!("ab\nc")));
    let post_id = post["id"].as_str().unwrap();
    let path = format!("/api/posts/{post_id}/comments");
    let (s, _) = call(&app, "POST", &path, Some(&t), Some(json!({ "text": blank }))).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "blank comment");
    let (s, c) = call(&app, "POST", &path, Some(&t), Some(json!({ "text": dirty }))).await;
    assert_eq!((s, c["text"].clone()), (StatusCode::CREATED, json!("ab\nc")));
    let ev = event(&app, &org, json!({})).await;
    reserve(&app, &t, ev).await;
    start_now(&pool, ev).await;
    let (s, r) = review(&app, &t, ev, 4, Some(dirty)).await;
    assert_eq!((s, r["text"].clone()), (StatusCode::CREATED, json!("ab\nc")));
    let (s, r) = review(&app, &t, ev, 4, Some(blank)).await;
    assert_eq!(
        (s, r["text"].clone()),
        (StatusCode::CREATED, Value::Null),
        "blank text dropped"
    );
    let report = |reason: &str, details: &str| {
        call(
            &app,
            "POST",
            "/api/reports",
            Some(&t),
            Some(json!({"target_type":"user","target_id": b, "reason": reason, "details": details})),
        )
    };
    assert_eq!(
        report(blank, "x").await.0,
        StatusCode::UNPROCESSABLE_ENTITY,
        "blank reason"
    );
    assert!(report("spam\u{202E}", dirty).await.0.is_success());
    let (reason, details): (String, Option<String>) =
        sqlx::query_as("SELECT reason, details FROM reports ORDER BY created_at DESC LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((reason.as_str(), details.as_deref()), ("spam", Some("ab\nc")));
}

// ------------------------------------------------------------------------------------- R6

/// R6: a ticket of a cancelled or finished event cannot be gifted (as for a transfer).
#[sqlx::test(migrations = "./migrations")]
async fn r6_no_gifts_for_gone_events(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "org@x.co", "Org").await;
    let (owner, ot) = user(&pool, "owner@x.co", "Owner").await;
    for gone in ["cancelled", "past"] {
        let ev = event(&app, &org, json!({})).await;
        let ticket = reserve(&app, &ot, ev).await;
        if gone == "cancelled" {
            assert_eq!(
                call(&app, "DELETE", &format!("/api/events/{ev}"), Some(&org), None)
                    .await
                    .0,
                StatusCode::NO_CONTENT
            );
        } else {
            sqlx::query(
                "UPDATE events SET starts_at = now() - interval '2 days', ends_at = now() - interval '1 day' WHERE id = $1",
            )
            .bind(ev)
            .execute(&pool)
            .await
            .unwrap();
        }
        let (s, v) = call(
            &app,
            "POST",
            &format!("/api/tickets/{ticket}/gift"),
            Some(&ot),
            Some(json!({"email":"friend@x.co"})),
        )
        .await;
        assert_eq!(
            (s, v["error"].clone()),
            (StatusCode::CONFLICT, json!(underground_api::tickets::EVENT_GONE)),
            "[SEC] {gone}"
        );
        let status = ticket_state(&pool, ticket).await;
        assert_eq!(status.0, owner);
        assert_ne!(status.1, "pending_claim", "{gone}");
    }
}

// ------------------------------------------------------------------------------------- R7

/// R7: a near-2 MB body nested far past serde_json's recursion limit, with a NUL deep inside, is
/// still refused by the NUL guard (the old `Value` parse gave up and let it through).
#[sqlx::test(migrations = "./migrations")]
async fn r7_nul_guard_reads_deep_bodies(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, t) = user(&pool, "u@x.co", "U").await;
    let depth = 900_000;
    let mut body = String::with_capacity(2 * depth + 32);
    body.push_str(r#"{"text":"#);
    body.push_str(&"[".repeat(depth));
    body.push_str(r#""x\u0000""#);
    body.push_str(&"]".repeat(depth));
    body.push('}');
    let (s, v) = raw(&app, request("POST", "/api/posts", Some(&t), Some(body), None)).await;
    let v: Value = serde_json::from_str(&v).unwrap();
    assert_eq!(
        (s, v["error"].clone()),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!(underground_api::text::NUL_REFUSED)
        ),
        "[SEC]"
    );
}

// ------------------------------------------------------------------------------------ R10

/// R10: the delivery logs carry no recipient address.
#[test]
fn r10_no_recipient_email_in_logs() {
    let main = include_str!("../src/main.rs");
    for needle in ["p.to_email", "to_email ="] {
        assert!(!main.contains(needle), "[PII] main.rs logs {needle}");
    }
}

// ------------------------------------------------------------------------------------ UX2

async fn qr(app: &Router, token: &str, ticket: Uuid) -> String {
    let (s, v) = call(app, "GET", &format!("/api/tickets/{ticket}/qr"), Some(token), None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    v["token"].as_str().unwrap().to_string()
}

/// UX2: scanning and validating are refused (409, with the start date) more than 6 hours before
/// the start, and change nothing; inside the window they work.
#[sqlx::test(migrations = "./migrations")]
async fn ux2_door_opens_six_hours_before(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "org@x.co", "Org").await;
    let (_, fan) = user(&pool, "fan@x.co", "Fan").await;
    let start = chrono::Utc::now() + chrono::Duration::hours(7);
    let ev = event(
        &app,
        &org,
        json!({ "starts_at": start.to_rfc3339(), "timezone": "Europe/Kyiv" }),
    )
    .await;
    let t1 = reserve(&app, &fan, ev).await;
    let code = qr(&app, &fan, t1).await;
    let local = start.with_timezone(&chrono_tz::Europe::Kyiv).format("%d.%m.%Y о %H:%M");
    let too_early = json!(format!("Ще рано: подія починається {local}"));
    let (s, v) = call(
        &app,
        "POST",
        &format!("/api/events/{ev}/scan"),
        Some(&org),
        Some(json!({"qr_token": code})),
    )
    .await;
    assert_eq!(
        (s, v["error"].clone()),
        (StatusCode::CONFLICT, too_early.clone()),
        "scan"
    );
    let (s, v) = call(
        &app,
        "POST",
        "/api/tickets/validate",
        Some(&org),
        Some(json!({"qr_token": code})),
    )
    .await;
    assert_eq!((s, v["error"].clone()), (StatusCode::CONFLICT, too_early), "validate");
    assert_eq!(ticket_state(&pool, t1).await.1, "active", "[SEC] nothing changed");

    sqlx::query("UPDATE events SET starts_at = now() + interval '5 hours' WHERE id = $1")
        .bind(ev)
        .execute(&pool)
        .await
        .unwrap();
    let (s, v) = call(
        &app,
        "POST",
        &format!("/api/events/{ev}/scan"),
        Some(&org),
        Some(json!({"qr_token": code})),
    )
    .await;
    assert_eq!((s, v["valid"].clone()), (StatusCode::OK, json!(true)), "{v}");
    assert_eq!(ticket_state(&pool, t1).await.1, "used");
}

// ------------------------------------------------------------------------------- round 3

async fn gift_rows(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM ticket_gifts")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// B1: an offer's email is one valid address of at most 254 characters; anything else is the same
/// 422 for transfer and gift, whatever accounts exist, and nothing is stored (a 2 MB email included).
#[sqlx::test(migrations = "./migrations")]
async fn b1_offer_email_is_one_bounded_address(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "org@x.co", "Org").await;
    let (owner, ot) = user(&pool, "owner@x.co", "Owner").await;
    user(&pool, "friend@x.co", "Friend").await;
    let ev = event(&app, &org, json!({})).await;
    let ticket = reserve(&app, &ot, ev).await;
    let huge = format!("{}@x.co", "a".repeat(2_000_000));
    let long = format!("{}@x.co", "b".repeat(250));
    let bad = [
        "",
        "   ",
        "not an email",
        "friend@x.co,owner@x.co",
        "friend@x.co owner@x.co",
        "<friend@x.co>",
        long.as_str(),
        huge.as_str(),
    ];
    let refused = (
        StatusCode::UNPROCESSABLE_ENTITY,
        json!({"error": underground_api::tickets::OFFER_EMAIL_REFUSED}),
    );
    for action in ["transfer", "gift"] {
        let path = format!("/api/tickets/{ticket}/{action}");
        for email in bad {
            let got = call(&app, "POST", &path, Some(&ot), Some(json!({ "email": email }))).await;
            assert_eq!(got, refused, "[SEC] {action} {:?}", &email[..email.len().min(40)]);
        }
        assert_eq!(gift_rows(&pool).await, 0, "[SEC] {action}: nothing stored");
        assert_eq!(ticket_state(&pool, ticket).await, (owner, "active".into()));
    }
    // A valid address still works, and its stored form is trimmed and lower-cased.
    let (s, _) = call(
        &app,
        "POST",
        &format!("/api/tickets/{ticket}/gift"),
        Some(&ot),
        Some(json!({"email":"  Friend@X.co "})),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    let stored: String = sqlx::query_scalar("SELECT to_email FROM ticket_gifts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, "friend@x.co");
}

/// B2a: events older than the retention go, newer ones stay; the hourly worker calls the prune.
#[sqlx::test(migrations = "./migrations")]
async fn b2_telemetry_retention(pool: PgPool) {
    sqlx::query(
        "INSERT INTO telemetry_events (name, created_at) \
         SELECT 'old', now() - interval '31 days' FROM generate_series(1, 12001) \
         UNION ALL SELECT 'new', now() - interval '29 days' FROM generate_series(1, 3)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let deleted = underground_api::telemetry::prune(&pool, 30, Duration::from_secs(30))
        .await
        .unwrap();
    assert_eq!(deleted, 12001, "all old rows, across several batches");
    let left: Vec<(String, i64)> =
        sqlx::query_as("SELECT name, count(*) FROM telemetry_events GROUP BY name ORDER BY name")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(left, [("new".to_string(), 3)]);
    // N3: the daily counters of pruned days go too.
    sqlx::query(
        "INSERT INTO telemetry_daily (day, total) VALUES \
         ((now() AT TIME ZONE 'UTC')::date - 31, 5), ((now() AT TIME ZONE 'UTC')::date - 29, 5)",
    )
    .execute(&pool)
    .await
    .unwrap();
    underground_api::telemetry::prune(&pool, 30, Duration::from_secs(30))
        .await
        .unwrap();
    let days: i64 = sqlx::query_scalar("SELECT count(*) FROM telemetry_daily")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(days, 1, "only the counter inside the retention stays");
    assert_eq!(underground_api::telemetry::retention_days(), 30);
    let main = include_str!("../src/main.rs");
    assert!(
        main.contains("prune_telemetry(&pool).await;") && main.contains("telemetry::prune(pool, days"),
        "the escrow worker prunes telemetry"
    );
}

async fn set_today(pool: &PgPool, total: i32, anon: i32) {
    sqlx::query(
        "INSERT INTO telemetry_daily (day, total, anon) VALUES ((now() AT TIME ZONE 'UTC')::date, $1, $2) \
         ON CONFLICT (day) DO UPDATE SET total = $1, anon = $2",
    )
    .bind(total)
    .bind(anon)
    .execute(pool)
    .await
    .unwrap();
}

async fn stored(pool: &PgPool, name: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM telemetry_events WHERE name = $1")
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// B2b / N1 / N3: over today's caps a call answers as before but stores nothing — the anonymous
/// sub-cap (20 000) for anonymous callers, and the global cap (40 000) for everyone, signed-in
/// callers included. (Round 3 said "a signed-in event is not capped"; the closure check, N1,
/// changed that.) The caps come from the day's counter row, not from counting events.
#[sqlx::test(migrations = "./migrations")]
async fn b2_telemetry_daily_caps(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, t) = user(&pool, "u@x.co", "U").await;
    let body = json!({"name":"share"}).to_string();
    let post = |token: Option<&str>, ip: &str| {
        raw(
            &app,
            request("POST", "/api/telemetry", token, Some(body.clone()), Some(ip)),
        )
    };
    // Events already stored today do not slow the call down or count by themselves (N3: no scan).
    sqlx::query("INSERT INTO telemetry_events (name) SELECT 'filler' FROM generate_series(1, 30000)")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(post(None, "192.0.2.1").await.0, StatusCode::NO_CONTENT);
    assert_eq!(
        stored(&pool, "share").await,
        1,
        "under the caps: stored, whatever the table holds"
    );

    set_today(&pool, 100, 20_000).await;
    assert_eq!(post(None, "192.0.2.2").await.0, StatusCode::NO_CONTENT, "same answer");
    assert_eq!(
        stored(&pool, "share").await,
        1,
        "[SEC] anonymous sub-cap: nothing stored"
    );
    assert_eq!(post(Some(&t), "192.0.2.3").await.0, StatusCode::NO_CONTENT);
    assert_eq!(
        stored(&pool, "share").await,
        2,
        "a signed-in event fits under the global cap"
    );

    set_today(&pool, 39_999, 0).await;
    assert_eq!(post(Some(&t), "192.0.2.4").await.0, StatusCode::NO_CONTENT);
    assert_eq!(stored(&pool, "share").await, 3, "the last slot of the day");
    assert_eq!(
        post(Some(&t), "192.0.2.5").await.0,
        StatusCode::NO_CONTENT,
        "same answer"
    );
    assert_eq!(post(None, "192.0.2.6").await.0, StatusCode::NO_CONTENT);
    assert_eq!(
        stored(&pool, "share").await,
        3,
        "[SEC] global cap: signed-in and anonymous both refused"
    );
    let (total, anon): (i32, i32) = sqlx::query_as("SELECT total, anon FROM telemetry_daily")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!((total, anon), (40_000, 0), "a refused call takes no slot");
}

/// N3: the cap holds under concurrency — 30 calls racing for the last 10 slots store exactly 10.
#[sqlx::test(migrations = "./migrations")]
async fn n3_daily_cap_holds_under_concurrency(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, t) = user(&pool, "u@x.co", "U").await;
    set_today(&pool, 39_990, 0).await;
    let calls = (0..30).map(|i| {
        let app = app.clone();
        let req = request(
            "POST",
            "/api/telemetry",
            Some(&t),
            Some(json!({"name":"race"}).to_string()),
            Some(&format!("198.51.100.{i}")),
        );
        tokio::spawn(async move { raw(&app, req).await.0 })
    });
    for call in calls.collect::<Vec<_>>() {
        assert_eq!(call.await.unwrap(), StatusCode::NO_CONTENT);
    }
    assert_eq!(stored(&pool, "race").await, 10, "[SEC] no overshoot");
}

/// B3: an open place offer stays in the alert list (with its link) however many newer alerts
/// there are.
#[sqlx::test(migrations = "./migrations")]
async fn b3_open_offer_is_always_listed(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "org@x.co", "Org").await;
    let (_, ot) = user(&pool, "owner@x.co", "Owner").await;
    let (friend, ft) = user(&pool, "friend@x.co", "Friend").await;
    let ev = event(&app, &org, json!({})).await;
    let ticket = reserve(&app, &ot, ev).await;
    let (s, _) = call(
        &app,
        "POST",
        &format!("/api/tickets/{ticket}/transfer"),
        Some(&ot),
        Some(json!({"email":"friend@x.co"})),
    )
    .await;
    assert_eq!(s, StatusCode::ACCEPTED);
    sqlx::query(
        "INSERT INTO notifications (user_id, kind, title, created_at) \
         SELECT $1, 'booking', 'n' || i, now() + i * interval '1 second' FROM generate_series(1, 120) i",
    )
    .bind(friend)
    .execute(&pool)
    .await
    .unwrap();
    let list = alerts(&app, &ft).await;
    assert_eq!(list.len(), 101, "the newest 100 and the open offer");
    assert!(
        offer_token(&app, &ft).await.is_some(),
        "[SEC] the claim link is still reachable"
    );
    // Once the offer is cancelled it is an ordinary old alert again.
    call(&app, "DELETE", &format!("/api/tickets/{ticket}/gift"), Some(&ot), None).await;
    assert_eq!(alerts(&app, &ft).await.len(), 100);
}

/// B6: a ticket whose earlier offer expired can be transferred again (as a gift can); while the
/// offer is open it cannot.
#[sqlx::test(migrations = "./migrations")]
async fn b6_transfer_after_an_expired_offer(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "org@x.co", "Org").await;
    let (owner, ot) = user(&pool, "owner@x.co", "Owner").await;
    let ev = event(&app, &org, json!({})).await;
    let ticket = reserve(&app, &ot, ev).await;
    let path = format!("/api/tickets/{ticket}/transfer");
    let send = |email: &'static str| call(&app, "POST", &path, Some(&ot), Some(json!({ "email": email })));
    assert_eq!(send("first@x.co").await.0, StatusCode::ACCEPTED);
    assert_eq!(
        send("second@x.co").await.0,
        StatusCode::NOT_FOUND,
        "the first offer is still open"
    );
    sqlx::query("UPDATE ticket_gifts SET expires_at = now() - interval '1 minute' WHERE ticket_id = $1")
        .bind(ticket)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(send("second@x.co").await.0, StatusCode::ACCEPTED);
    let gifts: Vec<(String, String)> =
        sqlx::query_as("SELECT to_email, status FROM ticket_gifts WHERE ticket_id = $1 ORDER BY created_at")
            .bind(ticket)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        gifts,
        [
            ("first@x.co".to_string(), "expired".to_string()),
            ("second@x.co".to_string(), "pending".to_string())
        ]
    );
    assert_eq!(ticket_state(&pool, ticket).await, (owner, "pending_claim".into()));
}

/// N2: an owner re-offering a ticket whose offer expired, racing the recipient trying that expired
/// offer, never ends in a server error (both lock the gift before the ticket, so no deadlock).
#[sqlx::test(migrations = "./migrations")]
async fn n2_reoffer_and_expired_claim_do_not_deadlock(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "org@x.co", "Org").await;
    let (_, ot) = user(&pool, "owner@x.co", "Owner").await;
    let (_, ft) = user(&pool, "friend@x.co", "Friend").await;
    // 14 rounds: each takes two slots of the sender's 30/h gift+transfer bucket.
    for round in 0..14 {
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
        sqlx::query("UPDATE ticket_gifts SET expires_at = now() - interval '1 minute' WHERE ticket_id = $1")
            .bind(ticket)
            .execute(&pool)
            .await
            .unwrap();
        let token = g["claim_token"].as_str().unwrap().to_string();
        let transfer = {
            let req = request(
                "POST",
                &format!("/api/tickets/{ticket}/transfer"),
                Some(&ot),
                Some(json!({"email":"other@x.co"}).to_string()),
                None,
            );
            let app = app.clone();
            tokio::spawn(async move { raw(&app, req).await })
        };
        let claim = {
            let req = request(
                "POST",
                "/api/tickets/claim",
                Some(&ft),
                Some(json!({ "token": token }).to_string()),
                None,
            );
            let app = app.clone();
            tokio::spawn(async move { raw(&app, req).await })
        };
        let (t, c) = (transfer.await.unwrap(), claim.await.unwrap());
        assert!(!t.0.is_server_error(), "[SEC] round {round}: transfer {t:?}");
        assert!(!c.0.is_server_error(), "[SEC] round {round}: claim {c:?}");
    }
}

/// N2, deterministic: while another transaction holds the pending gift (as a claim or a cancel
/// does, gift first), a re-offer of that ticket waits on the gift WITHOUT holding the ticket, so
/// the holder can still lock the ticket next (gift → ticket everywhere: no deadlock).
#[sqlx::test(migrations = "./migrations")]
async fn n2_reoffer_takes_the_gift_before_the_ticket(pool: PgPool) {
    reoffer_lock_order(pool, "transfer").await;
}

/// N2, the gift path: the same order when an expired offer is gifted again.
#[sqlx::test(migrations = "./migrations")]
async fn n2_gift_reoffer_takes_the_gift_before_the_ticket(pool: PgPool) {
    reoffer_lock_order(pool, "gift").await;
}

async fn reoffer_lock_order(pool: PgPool, action: &str) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "org@x.co", "Org").await;
    let (_, ot) = user(&pool, "owner@x.co", "Owner").await;
    let ev = event(&app, &org, json!({})).await;
    let ticket = reserve(&app, &ot, ev).await;
    let path = format!("/api/tickets/{ticket}/transfer");
    let (s, _) = call(&app, "POST", &path, Some(&ot), Some(json!({"email":"first@x.co"}))).await;
    assert_eq!(s, StatusCode::ACCEPTED);
    sqlx::query("UPDATE ticket_gifts SET expires_at = now() - interval '1 minute' WHERE ticket_id = $1")
        .bind(ticket)
        .execute(&pool)
        .await
        .unwrap();

    // The "claim": holds the gift row.
    let mut holder = pool.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM ticket_gifts WHERE ticket_id = $1 AND status = 'pending' FOR UPDATE")
        .bind(ticket)
        .execute(&mut *holder)
        .await
        .unwrap();
    let reoffer = {
        let again = format!("/api/tickets/{ticket}/{action}");
        let req = request(
            "POST",
            &again,
            Some(&ot),
            Some(json!({"email":"second@x.co"}).to_string()),
            None,
        );
        let app = app.clone();
        tokio::spawn(async move { raw(&app, req).await })
    };
    // Wait until the re-offer is blocked on a lock.
    let mut waited = 0;
    loop {
        let waiting: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_stat_activity WHERE datname = current_database() AND wait_event_type = 'Lock'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        if waiting > 0 {
            break;
        }
        waited += 1;
        assert!(waited < 500, "the re-offer never waited on the held gift");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    // The holder takes the ticket next, as a claim does: it must be free.
    let ticket_lock = sqlx::query("SELECT 1 FROM tickets WHERE id = $1 FOR UPDATE NOWAIT")
        .bind(ticket)
        .execute(&mut *holder)
        .await;
    assert!(
        ticket_lock.is_ok(),
        "[SEC] the re-offer locked the ticket before the gift (deadlock order): {ticket_lock:?}"
    );
    holder.rollback().await.unwrap();
    let (s, body) = reoffer.await.unwrap();
    assert!(s.is_success(), "{action}: {s} {body}");
}

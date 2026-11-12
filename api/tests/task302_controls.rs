//! TASK-302 controls (docs/backlog/TASK-302-abuse-cases.md): multi-place booking (Change C),
//! guest list without email, door-scan hygiene and staff management (Change E), and the
//! pre-existing issues fixed with them (P2, P5, P9, P12, P13, P14, P15, P16).
//!
//! Each test names the abuse-case rows and the control (CTn / Pn) it guards.

use std::collections::BTreeSet;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::response::IntoResponse;
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

async fn send_raw(app: &Router, req: Request<Body>) -> (StatusCode, String) {
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

fn parse(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or(Value::Null)
}

/// JSON request (`content-type: application/json`; no body when `body` is None).
async fn call(app: &Router, method: &str, path: &str, token: Option<&str>, body: Option<Value>) -> (StatusCode, Value) {
    let (s, t) = call_text(app, method, path, token, body).await;
    (s, parse(&t))
}

async fn call_text(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, String) {
    let mut b = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    let req = b
        .body(body.map(|v| Body::from(v.to_string())).unwrap_or_else(Body::empty))
        .unwrap();
    send_raw(app, req).await
}

/// Reserve with an exact raw body and optional content type.
async fn reserve_raw(
    app: &Router,
    token: &str,
    event: Uuid,
    content_type: Option<&str>,
    body: Vec<u8>,
) -> (StatusCode, Value) {
    let mut b =
        Request::post(format!("/api/events/{event}/reserve")).header("authorization", format!("Bearer {token}"));
    if let Some(ct) = content_type {
        b = b.header("content-type", ct);
    }
    let (s, t) = send_raw(app, b.body(Body::from(body)).unwrap()).await;
    (s, parse(&t))
}

async fn reserve(app: &Router, token: &str, event: Uuid, quantity: i64) -> (StatusCode, Value) {
    reserve_raw(
        app,
        token,
        event,
        Some("application/json"),
        json!({ "quantity": quantity }).to_string().into_bytes(),
    )
    .await
}

async fn free_event(app: &Router, token: &str, total: i32) -> Uuid {
    let (s, v) = call(
        app,
        "POST",
        "/api/events",
        Some(token),
        Some(json!({"title":"Gig","category":"Poetry","price":"0","tickets_total":total,"cost_type":"free"})),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    v["id"].as_str().unwrap().parse().unwrap()
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

async fn all_tickets(pool: &PgPool, owner: Uuid, event: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM tickets WHERE owner_id = $1 AND event_id = $2")
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

async fn ticket_ids(pool: &PgPool, owner: Uuid, event: Uuid, status: &str) -> Vec<Uuid> {
    sqlx::query_scalar(
        "SELECT id FROM tickets WHERE owner_id = $1 AND event_id = $2 AND status = $3 ORDER BY created_at, id",
    )
    .bind(owner)
    .bind(event)
    .bind(status)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn status_of(pool: &PgPool, ticket: Uuid) -> (String, Uuid) {
    sqlx::query_as("SELECT status, owner_id FROM tickets WHERE id = $1")
        .bind(ticket)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn qr(app: &Router, token: &str, ticket: Uuid) -> String {
    let (s, v) = call(app, "GET", &format!("/api/tickets/{ticket}/qr"), Some(token), None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    v["token"].as_str().unwrap().to_string()
}

async fn scan(app: &Router, token: &str, event: Uuid, qr: &str) -> (StatusCode, Value) {
    call(
        app,
        "POST",
        &format!("/api/events/{event}/scan"),
        Some(token),
        Some(json!({ "qr_token": qr })),
    )
    .await
}

async fn invite(app: &Router, owner: &str, event: Uuid) -> String {
    let (s, v) = call(
        app,
        "POST",
        &format!("/api/events/{event}/staff/invite"),
        Some(owner),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    v["invite_token"].as_str().unwrap().to_string()
}

async fn make_staff(app: &Router, owner: &str, event: Uuid, staff: &str) {
    let tok = invite(app, owner, event).await;
    let (s, v) = call(
        app,
        "POST",
        "/api/staff/accept",
        Some(staff),
        Some(json!({ "token": tok })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
}

fn keys(v: &Value) -> BTreeSet<String> {
    v.as_object().unwrap().keys().cloned().collect()
}

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn status_code(e: underground_api::error::AppError) -> StatusCode {
    e.into_response().status()
}

// ── CT1: strict, body-optional quantity ─────────────────────────────────────────────

/// Q01–Q10, Q13–Q16, Q19, Q20 at the parser: every malformed body is an error, never a silent 1.
#[test]
fn ct1_parse_quantity_table() {
    use underground_api::tickets::parse_quantity;
    let ok: &[(&[u8], i64)] = &[
        (b"", 1),
        (b"{}", 1),
        (b"{\"quantity\":null}", 1),
        (b"{\"quantity\":1}", 1),
        (b"{\"quantity\":5}", 5),
        (b" \n{\"quantity\":3}", 3),
        (
            b"{\"quantity\":2,\"owner_id\":\"x\",\"event_id\":\"y\",\"price\":\"50\",\"status\":\"used\",\"tickets_left\":999}",
            2,
        ),
    ];
    for (body, want) in ok {
        let got = parse_quantity(body).unwrap_or_else(|_| panic!("{:?} must parse", String::from_utf8_lossy(body)));
        assert_eq!(got, *want, "{:?}", String::from_utf8_lossy(body));
    }
    let deep_value = format!("{{\"quantity\":{}", "[".repeat(10_000));
    let deep_top = "[".repeat(10_000);
    let bad: Vec<(Vec<u8>, StatusCode)> = vec![
        (b"{\"quantity\":0}".to_vec(), StatusCode::UNPROCESSABLE_ENTITY),
        (b"{\"quantity\":-1}".to_vec(), StatusCode::UNPROCESSABLE_ENTITY),
        (b"{\"quantity\":6}".to_vec(), StatusCode::UNPROCESSABLE_ENTITY),
        (b"{\"quantity\":1000}".to_vec(), StatusCode::UNPROCESSABLE_ENTITY),
        (b"{\"quantity\":2147483648}".to_vec(), StatusCode::UNPROCESSABLE_ENTITY),
        (
            b"{\"quantity\":18446744073709551616}".to_vec(),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (b"{\"quantity\":1e309}".to_vec(), StatusCode::BAD_REQUEST),
        (b"{\"quantity\":2.5}".to_vec(), StatusCode::UNPROCESSABLE_ENTITY),
        (b"{\"quantity\":2.0}".to_vec(), StatusCode::UNPROCESSABLE_ENTITY),
        (b"{\"quantity\":1e0}".to_vec(), StatusCode::UNPROCESSABLE_ENTITY),
        (b"{\"quantity\":\"2\"}".to_vec(), StatusCode::UNPROCESSABLE_ENTITY),
        (b"{\"quantity\":\"five\"}".to_vec(), StatusCode::UNPROCESSABLE_ENTITY),
        (
            b"{\"quantity\":\"5 OR 1=1\"}".to_vec(),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (b"{\"quantity\":true}".to_vec(), StatusCode::UNPROCESSABLE_ENTITY),
        (b"{\"quantity\":[2]}".to_vec(), StatusCode::UNPROCESSABLE_ENTITY),
        (b"{\"quantity\":{\"$gt\":0}}".to_vec(), StatusCode::UNPROCESSABLE_ENTITY),
        (
            b"{\"quantity\":1,\"quantity\":5}".to_vec(),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            b"{\"quantity\":5,\"quantity\":1}".to_vec(),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (b"{\"quantity\":".to_vec(), StatusCode::BAD_REQUEST),
        (b"{quantity:2}".to_vec(), StatusCode::BAD_REQUEST),
        (b"{\"quantity\":1} trailing".to_vec(), StatusCode::BAD_REQUEST),
        (b"{\"quantity\":\"\xff\"}".to_vec(), StatusCode::BAD_REQUEST),
        (deep_value.into_bytes(), StatusCode::UNPROCESSABLE_ENTITY),
        (deep_top.into_bytes(), StatusCode::UNPROCESSABLE_ENTITY),
        (b"5".to_vec(), StatusCode::UNPROCESSABLE_ENTITY),
        (b"\"5\"".to_vec(), StatusCode::UNPROCESSABLE_ENTITY),
        (b"[2]".to_vec(), StatusCode::UNPROCESSABLE_ENTITY),
        (b"null".to_vec(), StatusCode::UNPROCESSABLE_ENTITY),
    ];
    for (body, want) in bad {
        let shown: String = String::from_utf8_lossy(&body).chars().take(40).collect();
        match parse_quantity(&body) {
            Ok(q) => panic!("[SEC] {shown:?} must be rejected, got quantity {q}"),
            Err(e) => assert_eq!(status_code(e), want, "{shown:?}"),
        }
    }
}

/// Q07, Q03, Q20, Q14 over HTTP: a rejected body books nothing and leaves capacity alone.
#[sqlx::test(migrations = "./migrations")]
async fn ct1_rejected_bodies_book_nothing(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let (u, ut) = user(&pool, "u@x.co", "Uma").await;
    let event = free_event(&app, &org, 30).await;
    for (body, want) in [
        (&b"{\"quantity\":\"2\"}"[..], StatusCode::UNPROCESSABLE_ENTITY),
        (b"{\"quantity\":6}", StatusCode::UNPROCESSABLE_ENTITY),
        (b"[2]", StatusCode::UNPROCESSABLE_ENTITY),
        (b"{\"quantity\":", StatusCode::BAD_REQUEST),
    ] {
        let (s, _) = reserve_raw(&app, &ut, event, Some("application/json"), body.to_vec()).await;
        assert_eq!(s, want, "{:?}", String::from_utf8_lossy(body));
    }
    assert_eq!(
        all_tickets(&pool, u, event).await,
        0,
        "[SEC] no ticket from a rejected body"
    );
    assert_eq!(tickets_left(&pool, event).await, 30);
    let (s, _) = call(&app, "GET", "/health", None, None).await;
    assert_eq!(s, StatusCode::OK);
}

/// Q09–Q12, Q17, CT16: the Android shape (no content type, no body), the web shape (JSON content
/// type, empty body), `null`, `{}` and a query string all book exactly one place, and the 201
/// body is still the pre-change Ticket DTO.
#[sqlx::test(migrations = "./migrations")]
async fn ct16_bodyless_clients_book_one_place_with_the_old_dto(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let (u, ut) = user(&pool, "u@x.co", "Uma").await;
    let event = free_event(&app, &org, 30).await;

    let (s, android) = reserve_raw(&app, &ut, event, None, Vec::new()).await;
    assert_eq!(s, StatusCode::CREATED, "{android}");
    for k in [
        "id",
        "event_id",
        "qr_token",
        "status",
        "price",
        "created_at",
        "event_title",
    ] {
        assert!(android.get(k).is_some(), "Ticket DTO keeps `{k}`: {android}");
    }
    assert_eq!(android["status"], "active");
    assert_eq!(holdings(&pool, u, event).await, 1);

    let (s, _) = reserve_raw(&app, &ut, event, Some("application/json"), Vec::new()).await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(holdings(&pool, u, event).await, 2);
    let (s, _) = reserve_raw(
        &app,
        &ut,
        event,
        Some("application/json"),
        b"{\"quantity\":null}".to_vec(),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(holdings(&pool, u, event).await, 3);
    let (s, _) = reserve_raw(&app, &ut, event, Some("application/json"), b"{}".to_vec()).await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(holdings(&pool, u, event).await, 4);

    let req = Request::post(format!("/api/events/{event}/reserve?quantity=5"))
        .header("authorization", format!("Bearer {ut}"))
        .body(Body::empty())
        .unwrap();
    let (s, _) = send_raw(&app, req).await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(holdings(&pool, u, event).await, 5, "the query string is ignored");
    assert_eq!(tickets_left(&pool, event).await, 25);
}

/// Q21, Q16: reserve(5) mints five distinct tickets, each with its own working QR; extra body
/// fields cannot pick the owner, the event, the price or the status.
#[sqlx::test(migrations = "./migrations")]
async fn ct2_ct3_five_places_are_five_distinct_tickets(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let (u, ut) = user(&pool, "u@x.co", "Uma").await;
    let (v, _) = user(&pool, "v@x.co", "Vik").await;
    let event = free_event(&app, &org, 30).await;
    let other = free_event(&app, &org, 30).await;

    let (s, first) = reserve(&app, &ut, event, 5).await;
    assert_eq!(s, StatusCode::CREATED, "{first}");
    let ids = ticket_ids(&pool, u, event, "active").await;
    assert_eq!(ids.len(), 5);
    assert!(
        ids.contains(&first["id"].as_str().unwrap().parse().unwrap()),
        "201 body is one of them"
    );
    let qr_tokens: BTreeSet<String> = sqlx::query_scalar("SELECT qr_token FROM tickets WHERE owner_id = $1")
        .bind(u)
        .fetch_all(&pool)
        .await
        .unwrap()
        .into_iter()
        .collect();
    assert_eq!(qr_tokens.len(), 5, "distinct qr_token per place");
    assert_eq!(tickets_left(&pool, event).await, 25);
    for id in &ids {
        let token = qr(&app, &ut, *id).await;
        let claims = underground_api::qr::verify_qr(&token, SECRET).unwrap();
        assert_eq!(claims.tid, *id);
        assert_eq!(claims.oid, u);
    }

    let (w, wt) = user(&pool, "w@x.co", "Wes").await;
    let body = json!({"quantity":2,"owner_id":v,"event_id":other,"price":"50","status":"used","tickets_left":999});
    let (s, _) = reserve_raw(
        &app,
        &wt,
        event,
        Some("application/json"),
        body.to_string().into_bytes(),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    let rows: Vec<(Uuid, Uuid, String, rust_decimal::Decimal)> =
        sqlx::query_as("SELECT owner_id, event_id, status, price FROM tickets WHERE owner_id = $1")
            .bind(w)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(rows.len(), 2);
    for (owner, ev, status, price) in rows {
        assert_eq!(
            (owner, ev, status.as_str(), price),
            (w, event, "active", rust_decimal::Decimal::ZERO)
        );
    }
    assert_eq!(
        all_tickets(&pool, v, event).await + all_tickets(&pool, v, other).await,
        0
    );
    assert_eq!(tickets_left(&pool, other).await, 30);
    assert_eq!(tickets_left(&pool, event).await, 23);
}

// ── CT2: per-person cap ─────────────────────────────────────────────────────────────

/// C01, C02, C09, C10: sequential cap, returns free it, it is per event.
#[sqlx::test(migrations = "./migrations")]
async fn ct2_cap_of_five_places_per_person(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let (u, ut) = user(&pool, "u@x.co", "Uma").await;
    let event = free_event(&app, &org, 30).await;
    let other = free_event(&app, &org, 30).await;

    assert_eq!(reserve(&app, &ut, event, 3).await.0, StatusCode::CREATED);
    let (s, body) = reserve(&app, &ut, event, 3).await;
    assert_eq!(s, StatusCode::CONFLICT, "[SEC] 3 + 3 > 5: {body}");
    assert_eq!(holdings(&pool, u, event).await, 3);
    assert_eq!(
        tickets_left(&pool, event).await,
        27,
        "a refused booking takes no capacity"
    );
    assert_eq!(reserve(&app, &ut, event, 2).await.0, StatusCode::CREATED);
    assert_eq!(
        reserve(&app, &ut, event, 1).await.0,
        StatusCode::CONFLICT,
        "[SEC] sixth place refused"
    );
    assert_eq!(holdings(&pool, u, event).await, 5);

    for id in ticket_ids(&pool, u, event, "active").await.into_iter().take(2) {
        let (s, _) = call(&app, "POST", &format!("/api/tickets/{id}/return"), Some(&ut), None).await;
        assert_eq!(s, StatusCode::OK);
    }
    assert_eq!(
        reserve(&app, &ut, event, 2).await.0,
        StatusCode::CREATED,
        "returned places free the cap"
    );
    assert_eq!(holdings(&pool, u, event).await, 5);

    assert_eq!(
        reserve(&app, &ut, other, 5).await.0,
        StatusCode::CREATED,
        "the cap is per event"
    );
}

/// C07, C08, T05 (P8): scanned places and outstanding gifts keep counting; revoking a gift at
/// the cap is not a 500.
#[sqlx::test(migrations = "./migrations")]
async fn ct2_used_and_gifted_places_count(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let (u, ut) = user(&pool, "u@x.co", "Uma").await;
    let (g, gt) = user(&pool, "g@x.co", "Gus").await;
    let event = free_event(&app, &org, 30).await;

    assert_eq!(reserve(&app, &ut, event, 5).await.0, StatusCode::CREATED);
    sqlx::query(
        "UPDATE tickets SET status = 'used' WHERE id IN \
         (SELECT id FROM tickets WHERE owner_id = $1 ORDER BY id LIMIT 2)",
    )
    .bind(u)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        reserve(&app, &ut, event, 1).await.0,
        StatusCode::CONFLICT,
        "[SEC] scanned places still count (D1)"
    );
    assert_eq!(all_tickets(&pool, u, event).await, 5);

    assert_eq!(reserve(&app, &gt, event, 5).await.0, StatusCode::CREATED);
    let gifts: Vec<Uuid> = ticket_ids(&pool, g, event, "active")
        .await
        .into_iter()
        .take(4)
        .collect();
    for (i, id) in gifts.iter().enumerate() {
        let (s, v) = call(
            &app,
            "POST",
            &format!("/api/tickets/{id}/gift"),
            Some(&gt),
            Some(json!({"email": format!("friend{i}@x.co")})),
        )
        .await;
        assert_eq!(s, StatusCode::CREATED, "{v}");
    }
    assert_eq!(
        reserve(&app, &gt, event, 1).await.0,
        StatusCode::CONFLICT,
        "[SEC] gifts still out count (D1: no gift-out-then-rebook)"
    );
    let (s, v) = call(
        &app,
        "DELETE",
        &format!("/api/tickets/{}/gift", gifts[0]),
        Some(&gt),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "revoking at the cap is not a 500: {v}");
    assert_eq!(status_of(&pool, gifts[0]).await.0, "active");
    assert_eq!(holdings(&pool, g, event).await, 5);
}

/// C03, C05: concurrent bookings by one person never exceed five places.
#[sqlx::test(migrations = "./migrations")]
async fn ct2_concurrent_bookings_never_exceed_the_cap(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    for round in 0..8 {
        let event = free_event(&app, &org, 30).await;
        let (u, ut) = user(&pool, &format!("u{round}@x.co"), "Uma").await;
        let mut set = tokio::task::JoinSet::new();
        for _ in 0..6 {
            let (app, ut) = (app.clone(), ut.clone());
            set.spawn(async move { reserve(&app, &ut, event, 1).await.0 });
        }
        let mut created = 0;
        let mut conflict = 0;
        while let Some(r) = set.join_next().await {
            match r.unwrap() {
                StatusCode::CREATED => created += 1,
                StatusCode::CONFLICT => conflict += 1,
                other => panic!("unexpected {other}"),
            }
        }
        assert_eq!(
            (created, conflict),
            (5, 1),
            "[SEC] round {round}: six parallel reserve(1)"
        );
        assert_eq!(holdings(&pool, u, event).await, 5);
        assert_eq!(tickets_left(&pool, event).await, 25);

        let event = free_event(&app, &org, 30).await;
        let (u, ut) = user(&pool, &format!("p{round}@x.co"), "Pat").await;
        let (a, b) = tokio::join!(reserve(&app, &ut, event, 3), reserve(&app, &ut, event, 3));
        let mut got = [a.0, b.0];
        got.sort();
        assert_eq!(
            got,
            [StatusCode::CREATED, StatusCode::CONFLICT],
            "[SEC] round {round}: 3 + 3"
        );
        assert_eq!(holdings(&pool, u, event).await, 3);
        assert_eq!(tickets_left(&pool, event).await, 27);
    }
}

// ── CT3: all-or-nothing capacity ────────────────────────────────────────────────────

/// K01, K04: no partial fill.
#[sqlx::test(migrations = "./migrations")]
async fn ct3_no_partial_fill(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let (u, ut) = user(&pool, "u@x.co", "Uma").await;
    let event = free_event(&app, &org, 2).await;
    assert_eq!(reserve(&app, &ut, event, 3).await.0, StatusCode::CONFLICT);
    assert_eq!(tickets_left(&pool, event).await, 2);
    assert_eq!(
        all_tickets(&pool, u, event).await,
        0,
        "[SEC] nothing booked on a short fill"
    );
    let empty = free_event(&app, &org, 0).await;
    assert_eq!(reserve(&app, &ut, empty, 1).await.0, StatusCode::CONFLICT);
    assert_eq!(tickets_left(&pool, empty).await, 0);
}

/// K02, K03: the last places under concurrency; capacity never goes negative.
#[sqlx::test(migrations = "./migrations")]
async fn ct3_last_places_under_concurrency(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    for round in 0..6 {
        let event = free_event(&app, &org, 5).await;
        let mut set = tokio::task::JoinSet::new();
        for i in 0..6 {
            let (_, t) = user(&pool, &format!("k{round}-{i}@x.co"), "Kim").await;
            let app = app.clone();
            set.spawn(async move { reserve(&app, &t, event, 1).await.0 });
        }
        let mut codes = Vec::new();
        while let Some(r) = set.join_next().await {
            codes.push(r.unwrap());
        }
        assert_eq!(
            codes.iter().filter(|c| **c == StatusCode::CREATED).count(),
            5,
            "round {round}: {codes:?}"
        );
        assert_eq!(codes.iter().filter(|c| **c == StatusCode::CONFLICT).count(), 1);
        assert_eq!(tickets_left(&pool, event).await, 0);

        let event = free_event(&app, &org, 4).await;
        let (_, a) = user(&pool, &format!("a{round}@x.co"), "Ann").await;
        let (_, b) = user(&pool, &format!("b{round}@x.co"), "Bob").await;
        let (ra, rb) = tokio::join!(reserve(&app, &a, event, 3), reserve(&app, &b, event, 3));
        let mut got = [ra.0, rb.0];
        got.sort();
        assert_eq!(
            got,
            [StatusCode::CREATED, StatusCode::CONFLICT],
            "[SEC] round {round}: 4 places, 3 + 3"
        );
        assert_eq!(tickets_left(&pool, event).await, 1);
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM tickets WHERE event_id = $1")
            .bind(event)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(n, 3);
    }
}

/// K05: a failure on the 3rd insert rolls back the whole booking (capacity and tickets).
#[sqlx::test(migrations = "./migrations")]
async fn ct3_failure_mid_booking_rolls_back_everything(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let (u, ut) = user(&pool, "u@x.co", "Uma").await;
    let event = free_event(&app, &org, 30).await;
    sqlx::query(
        "CREATE FUNCTION t302_fail_third() RETURNS trigger LANGUAGE plpgsql AS $$ \
         BEGIN IF (SELECT count(*) FROM tickets WHERE event_id = NEW.event_id) >= 2 THEN \
           RAISE EXCEPTION 'injected failure'; END IF; RETURN NEW; END $$",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TRIGGER t302_fail_third BEFORE INSERT ON tickets FOR EACH ROW EXECUTE FUNCTION t302_fail_third()",
    )
    .execute(&pool)
    .await
    .unwrap();
    let req = Request::post(format!("/api/events/{event}/reserve"))
        .header("authorization", format!("Bearer {ut}"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"quantity":5}"#))
        .unwrap();
    let (s, text) = send_raw(&app, req).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(text, r#"{"error":"database error"}"#);
    assert_eq!(tickets_left(&pool, event).await, 30, "[SEC] capacity rolled back");
    assert_eq!(all_tickets(&pool, u, event).await, 0, "[SEC] no ticket survives");
}

// ── CT4: recipient cap on transfer and claim ─────────────────────────────────────────

/// T01, T02, C11.
#[sqlx::test(migrations = "./migrations")]
async fn ct4_receiving_places_respects_the_recipient_cap(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let (u, ut) = user(&pool, "u@x.co", "Uma").await;
    let (v, vt) = user(&pool, "v@x.co", "Vik").await;
    let (x, _) = user(&pool, "x@x.co", "Xen").await;
    let event = free_event(&app, &org, 30).await;

    assert_eq!(reserve(&app, &vt, event, 5).await.0, StatusCode::CREATED);
    assert_eq!(reserve(&app, &ut, event, 5).await.0, StatusCode::CREATED);
    let mine = ticket_ids(&pool, u, event, "active").await;

    let (s, body) = call(
        &app,
        "POST",
        &format!("/api/tickets/{}/transfer", mine[0]),
        Some(&ut),
        Some(json!({"email":"v@x.co"})),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT, "[SEC] transfer to a full recipient: {body}");
    assert_eq!(status_of(&pool, mine[0]).await, ("active".to_string(), u));
    assert_eq!(holdings(&pool, v, event).await, 5);

    let (s, gift) = call(
        &app,
        "POST",
        &format!("/api/tickets/{}/gift", mine[1]),
        Some(&ut),
        Some(json!({"email":"v@x.co"})),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    let (s, body) = call(
        &app,
        "POST",
        "/api/tickets/claim",
        Some(&vt),
        Some(json!({"token": gift["claim_token"]})),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT, "[SEC] claim by a full recipient: {body}");
    assert_eq!(status_of(&pool, mine[1]).await, ("pending_claim".to_string(), u));
    let gift_status: String = sqlx::query_scalar("SELECT status FROM ticket_gifts WHERE ticket_id = $1")
        .bind(mine[1])
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(gift_status, "pending", "the sender can still revoke");
    assert_eq!(holdings(&pool, v, event).await, 5);

    // C11: a transfer out frees the sender's cap; the recipient below the cap accepts.
    let (s, _) = call(
        &app,
        "POST",
        &format!("/api/tickets/{}/transfer", mine[2]),
        Some(&ut),
        Some(json!({"email":"x@x.co"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(holdings(&pool, x, event).await, 1);
    assert_eq!(reserve(&app, &ut, event, 1).await.0, StatusCode::CREATED);
    assert_eq!(holdings(&pool, u, event).await, 5);
}

/// T03: two concurrent claims into the last free place — exactly one wins.
#[sqlx::test(migrations = "./migrations")]
async fn ct4_concurrent_claims_into_the_last_place(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    for round in 0..5 {
        let event = free_event(&app, &org, 30).await;
        let (v, vt) = user(&pool, &format!("v{round}@x.co"), "Vik").await;
        let (u, ut) = user(&pool, &format!("u{round}@x.co"), "Uma").await;
        assert_eq!(reserve(&app, &vt, event, 4).await.0, StatusCode::CREATED);
        assert_eq!(reserve(&app, &ut, event, 2).await.0, StatusCode::CREATED);
        let mut tokens = Vec::new();
        for id in ticket_ids(&pool, u, event, "active").await {
            let (s, g) = call(
                &app,
                "POST",
                &format!("/api/tickets/{id}/gift"),
                Some(&ut),
                Some(json!({"email": format!("v{round}@x.co")})),
            )
            .await;
            assert_eq!(s, StatusCode::CREATED);
            tokens.push(g["claim_token"].as_str().unwrap().to_string());
        }
        let claim = |t: String| {
            let (app, vt) = (app.clone(), vt.clone());
            async move {
                call(
                    &app,
                    "POST",
                    "/api/tickets/claim",
                    Some(&vt),
                    Some(json!({ "token": t })),
                )
                .await
                .0
            }
        };
        let (a, b) = tokio::join!(claim(tokens[0].clone()), claim(tokens[1].clone()));
        let mut got = [a, b];
        got.sort();
        assert_eq!(got, [StatusCode::OK, StatusCode::CONFLICT], "[SEC] round {round}");
        assert_eq!(holdings(&pool, v, event).await, 5);
    }
}

// ── CT6 / P5 / P13 / P14: rate limits ───────────────────────────────────────────────

/// RL01, X11, RL02, RL04: every reserve call counts, per event and across events, per user.
#[sqlx::test(migrations = "./migrations")]
async fn ct6_reserve_is_rate_limited_per_user(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let (u, ut) = user(&pool, "u@x.co", "Uma").await;
    let (v, vt) = user(&pool, "v@x.co", "Vik").await;
    let event = free_event(&app, &org, 30).await;
    for _ in 0..10 {
        assert_eq!(reserve(&app, &ut, event, 0).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    }
    let (s, _) = reserve(&app, &ut, event, 1).await;
    assert_eq!(
        s,
        StatusCode::TOO_MANY_REQUESTS,
        "[SEC] 11th call on one event within the hour"
    );
    assert_eq!(all_tickets(&pool, u, event).await, 0);
    assert_eq!(tickets_left(&pool, event).await, 30);

    // No cross-user lockout.
    assert_eq!(reserve(&app, &vt, event, 1).await.0, StatusCode::CREATED);
    assert_eq!(holdings(&pool, v, event).await, 1);

    // 30 calls across events (here: unknown events, 404 each), then the 31st is refused.
    let (w, wt) = user(&pool, "w@x.co", "Wes").await;
    for _ in 0..30 {
        assert_eq!(reserve(&app, &wt, Uuid::new_v4(), 1).await.0, StatusCode::NOT_FOUND);
    }
    assert_eq!(
        reserve(&app, &wt, event, 1).await.0,
        StatusCode::TOO_MANY_REQUESTS,
        "[SEC] 31st call across events"
    );
    assert_eq!(all_tickets(&pool, w, event).await, 0);
}

/// RL03 / P5: a parallel burst against one bucket lets exactly `max` through.
#[sqlx::test(migrations = "./migrations")]
async fn p5_rate_limiter_holds_under_a_parallel_burst(pool: PgPool) {
    for round in 0..5 {
        let bucket = format!("burst:{round}");
        let mut set = tokio::task::JoinSet::new();
        for _ in 0..20 {
            let (pool, bucket) = (pool.clone(), bucket.clone());
            set.spawn(async move { underground_api::ratelimit::hit(&pool, &bucket, 10, 3600).await.is_ok() });
        }
        let mut passed = 0;
        while let Some(r) = set.join_next().await {
            passed += r.unwrap() as i32;
        }
        assert_eq!(passed, 10, "[SEC] round {round}: 20 parallel attempts, limit 10");
    }
}

/// S18, RL05: the 121st scan within a minute is refused and changes nothing.
#[sqlx::test(migrations = "./migrations")]
async fn ct6_scan_is_rate_limited(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let (u, ut) = user(&pool, "u@x.co", "Uma").await;
    let event = free_event(&app, &org, 30).await;
    assert_eq!(reserve(&app, &ut, event, 1).await.0, StatusCode::CREATED);
    let ticket = ticket_ids(&pool, u, event, "active").await[0];
    let code = qr(&app, &ut, ticket).await;
    for _ in 0..120 {
        assert_eq!(scan(&app, &org, event, "garbage").await.0, StatusCode::OK);
    }
    let (s, _) = scan(&app, &org, event, &code).await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS, "[SEC] 121st scan in a minute");
    assert_eq!(
        status_of(&pool, ticket).await.0,
        "active",
        "a refused scan admits nobody"
    );
}

/// P14: transfer and gift share one per-sender bucket (the "no such email" answer is an oracle).
#[sqlx::test(migrations = "./migrations")]
async fn p14_transfer_and_gift_are_rate_limited(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let (u, ut) = user(&pool, "u@x.co", "Uma").await;
    let event = free_event(&app, &org, 30).await;
    assert_eq!(reserve(&app, &ut, event, 1).await.0, StatusCode::CREATED);
    let ticket = ticket_ids(&pool, u, event, "active").await[0];
    for i in 0..30 {
        let (s, _) = call(
            &app,
            "POST",
            &format!("/api/tickets/{ticket}/transfer"),
            Some(&ut),
            Some(json!({"email": format!("nobody{i}@x.co")})),
        )
        .await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    }
    let (s, _) = call(
        &app,
        "POST",
        &format!("/api/tickets/{ticket}/transfer"),
        Some(&ut),
        Some(json!({"email":"nobody-else@x.co"})),
    )
    .await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS, "[SEC] 31st transfer probe");
    let (s, _) = call(
        &app,
        "POST",
        &format!("/api/tickets/{ticket}/gift"),
        Some(&ut),
        Some(json!({"email":"friend@x.co"})),
    )
    .await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS, "gift shares the bucket");
    assert_eq!(status_of(&pool, ticket).await.0, "active");
}

/// P13: rows older than the longest window are pruned.
#[sqlx::test(migrations = "./migrations")]
async fn p13_rate_events_are_pruned(pool: PgPool) {
    for bucket in ["b:same", "b:other"] {
        sqlx::query("INSERT INTO rate_events (bucket, created_at) VALUES ($1, now() - interval '2 hours')")
            .bind(bucket)
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO rate_events (bucket) VALUES ('b:recent')")
        .execute(&pool)
        .await
        .unwrap();
    underground_api::ratelimit::hit(&pool, "b:same", 10, 60).await.unwrap();
    let same_old: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM rate_events WHERE bucket = 'b:same' AND created_at < now() - interval '1 hour'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(same_old, 0, "a hit prunes its own bucket");
    underground_api::ratelimit::prune(&pool).await.unwrap();
    let left: Vec<String> = sqlx::query_scalar("SELECT bucket FROM rate_events ORDER BY bucket")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(
        left,
        vec!["b:recent".to_string(), "b:same".to_string()],
        "recent rows stay"
    );
}

// ── CT7 / CT8: guest list ────────────────────────────────────────────────────────────

/// G01, G03, G04, G12: one row per person; places over active+pending_claim+used; checked_in
/// over used; returned-only owners absent; same names stay separate rows; exact key set.
#[sqlx::test(migrations = "./migrations")]
async fn ct7_guest_list_counts_people_and_places(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let (u, ut) = user(&pool, "u@x.co", "Uma").await;
    let (_, wt) = user(&pool, "w@x.co", "Wes").await;
    let (_, i1) = user(&pool, "i1@x.co", "Ivan").await;
    let (_, i2) = user(&pool, "i2@x.co", "Ivan").await;
    let (_, sq) = user(&pool, "sq@x.co", "'); DROP TABLE tickets;--").await;
    let event = free_event(&app, &org, 30).await;

    assert_eq!(reserve(&app, &ut, event, 5).await.0, StatusCode::CREATED);
    let ids = ticket_ids(&pool, u, event, "active").await;
    let (s, _) = call(
        &app,
        "POST",
        &format!("/api/tickets/{}/return", ids[0]),
        Some(&ut),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(
        &app,
        "POST",
        &format!("/api/tickets/{}/gift", ids[1]),
        Some(&ut),
        Some(json!({"email":"friend@x.co"})),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    let code = qr(&app, &ut, ids[2]).await;
    assert_eq!(scan(&app, &org, event, &code).await.1["valid"], true);

    assert_eq!(reserve(&app, &wt, event, 1).await.0, StatusCode::CREATED);
    let w_ticket: Uuid =
        sqlx::query_scalar("SELECT t.id FROM tickets t JOIN users u ON u.id = t.owner_id WHERE u.email = 'w@x.co'")
            .fetch_one(&pool)
            .await
            .unwrap();
    call(
        &app,
        "POST",
        &format!("/api/tickets/{w_ticket}/return"),
        Some(&wt),
        None,
    )
    .await;
    assert_eq!(reserve(&app, &i1, event, 2).await.0, StatusCode::CREATED);
    assert_eq!(reserve(&app, &i2, event, 2).await.0, StatusCode::CREATED);
    assert_eq!(reserve(&app, &sq, event, 1).await.0, StatusCode::CREATED);
    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM tickets")
        .fetch_one(&pool)
        .await
        .unwrap();

    let (s, list) = call(&app, "GET", &format!("/api/events/{event}/guests"), Some(&org), None).await;
    assert_eq!(s, StatusCode::OK, "{list}");
    let rows = list.as_array().unwrap();
    assert_eq!(
        rows.len(),
        4,
        "Uma, Ivan, Ivan, the SQL-ish name; Wes (returned only) absent: {list}"
    );
    for row in rows {
        assert_eq!(
            keys(row),
            set(&["name", "places", "checked_in", "booked_at"]),
            "[PII] {row}"
        );
    }
    let uma = rows.iter().find(|r| r["name"] == "Uma").unwrap();
    assert_eq!((uma["places"].as_i64(), uma["checked_in"].as_i64()), (Some(4), Some(1)));
    let ivans: Vec<&Value> = rows.iter().filter(|r| r["name"] == "Ivan").collect();
    assert_eq!(ivans.len(), 2, "grouped by person, not by name");
    assert!(ivans.iter().all(|r| r["places"] == 2 && r["checked_in"] == 0));
    assert!(rows.iter().any(|r| r["name"] == "'); DROP TABLE tickets;--"));
    assert!(!rows.iter().any(|r| r["name"] == "Wes"));
    let after: i64 = sqlx::query_scalar("SELECT count(*) FROM tickets")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(before, after);
}

/// G05–G08, D5: organiser only.
#[sqlx::test(migrations = "./migrations")]
async fn ct7_guest_list_is_organiser_only(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let (_, org2) = user(&pool, "o2@x.co", "Org2").await;
    let (_, st) = user(&pool, "s@x.co", "Stu").await;
    let (_, ut) = user(&pool, "u@x.co", "Uma").await;
    let event = free_event(&app, &org, 30).await;
    make_staff(&app, &org, event, &st).await;
    assert_eq!(reserve(&app, &ut, event, 1).await.0, StatusCode::CREATED);
    let path = format!("/api/events/{event}/guests");
    for (who, token) in [("other organiser", &org2), ("staff", &st), ("attendee", &ut)] {
        let (s, text) = call_text(&app, "GET", &path, Some(token), None).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "[SEC] {who}");
        assert!(!text.contains("Uma"), "[PII] no attendee data in the 403 body");
    }
    assert_eq!(call(&app, "GET", &path, None, None).await.0, StatusCode::UNAUTHORIZED);
    let ghost = format!("/api/events/{}/guests", Uuid::new_v4());
    assert_eq!(
        call(&app, "GET", &ghost, Some(&org), None).await.0,
        StatusCode::NOT_FOUND
    );
}

/// G09 / CT8: a name that is empty, the email local part (any case) or the whole email is shown
/// as "Гість" — on the guest list, the per-ticket list and the scan response.
#[sqlx::test(migrations = "./migrations")]
async fn ct8_placeholder_names_are_shown_as_guest(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let event = free_event(&app, &org, 30).await;
    let people = [
        ("pii.localpart.9c1@x.co", "pii.localpart.9c1"),
        ("pii.localpart.9c2@x.co", "PII.LocalPart.9C2"),
        ("pii.localpart.9c3@x.co", "pii.localpart.9c3@x.co"),
        ("pii.localpart.9c4@x.co", "   "),
        ("olena.k@x.co", "Олена"),
    ];
    let mut last = None;
    for (email, name) in people {
        let (id, t) = user(&pool, email, name).await;
        assert_eq!(reserve(&app, &t, event, 1).await.0, StatusCode::CREATED);
        last = Some((id, t));
    }
    // Email-code sign-up path itself (the real source of placeholder names).
    let pl = underground_api::users::create_passwordless_user(&pool, "pii.localpart.9c5@x.co")
        .await
        .unwrap();
    let plt = underground_api::auth::issue_token(pl.id, SECRET).unwrap();
    assert_eq!(reserve(&app, &plt, event, 1).await.0, StatusCode::CREATED);

    let (_, text) = call_text(&app, "GET", &format!("/api/events/{event}/guests"), Some(&org), None).await;
    let list = parse(&text);
    let mut names: Vec<&str> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap())
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec!["Гість", "Гість", "Гість", "Гість", "Гість", "Олена"],
        "{text}"
    );
    assert!(!text.to_lowercase().contains("pii.localpart"), "[PII] {text}");

    let (_, text) = call_text(&app, "GET", &format!("/api/events/{event}/tickets"), Some(&org), None).await;
    assert!(
        !text.to_lowercase().contains("pii.localpart"),
        "[PII] per-ticket list: {text}"
    );
    assert!(!text.contains("@"), "[PII] per-ticket list: {text}");

    let ticket = ticket_ids(&pool, pl.id, event, "active").await[0];
    let code = qr(&app, &plt, ticket).await;
    let (_, v) = scan(&app, &org, event, &code).await;
    assert_eq!(v["holder_name"], "Гість");
    let (id, t) = last.unwrap();
    let ticket = ticket_ids(&pool, id, event, "active").await[0];
    let code = qr(&app, &t, ticket).await;
    assert_eq!(
        scan(&app, &org, event, &code).await.1["holder_name"],
        "Олена",
        "a real name is kept"
    );
}

// ── Scan response, CT10, CT11 ────────────────────────────────────────────────────────

/// The holder's name, places and checked-in count at this event.
#[sqlx::test(migrations = "./migrations")]
async fn scan_reports_holder_places_and_checked_in(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let (u, ut) = user(&pool, "u@x.co", "Uma").await;
    let event = free_event(&app, &org, 30).await;
    assert_eq!(reserve(&app, &ut, event, 3).await.0, StatusCode::CREATED);
    let ids = ticket_ids(&pool, u, event, "active").await;
    let first = qr(&app, &ut, ids[0]).await;
    let (_, v) = scan(&app, &org, event, &first).await;
    assert_eq!(
        (v["valid"].clone(), v["reason"].clone(), v["holder_name"].clone()),
        (json!(true), json!("valid"), json!("Uma"))
    );
    assert_eq!((v["places"].as_i64(), v["checked_in"].as_i64()), (Some(3), Some(1)));
    let (_, v) = scan(&app, &org, event, &first).await;
    assert_eq!(v["reason"], "already_used");
    assert_eq!((v["places"].as_i64(), v["checked_in"].as_i64()), (Some(3), Some(1)));
    let second = qr(&app, &ut, ids[1]).await;
    let (_, v) = scan(&app, &org, event, &second).await;
    assert_eq!((v["places"].as_i64(), v["checked_in"].as_i64()), (Some(3), Some(2)));
    let (_, v) = scan(&app, &org, event, "garbage").await;
    assert_eq!(v["reason"], "not_found");
    assert!(v["holder_name"].is_null() && v["places"].is_null() && v["checked_in"].is_null());
}

/// S10 / CT10: once a ticket changes hands, the previous owner's QR is dead at both doors.
#[sqlx::test(migrations = "./migrations")]
async fn ct10_previous_owner_qr_is_dead(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let (u, ut) = user(&pool, "u@x.co", "Uma").await;
    let (_, vt) = user(&pool, "v@x.co", "Vik").await;
    let event = free_event(&app, &org, 30).await;
    assert_eq!(reserve(&app, &ut, event, 2).await.0, StatusCode::CREATED);
    let ids = ticket_ids(&pool, u, event, "active").await;

    // Transfer path.
    let old = qr(&app, &ut, ids[0]).await;
    let (s, _) = call(
        &app,
        "POST",
        &format!("/api/tickets/{}/transfer", ids[0]),
        Some(&ut),
        Some(json!({"email":"v@x.co"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, v) = scan(&app, &org, event, &old).await;
    assert_eq!(
        (v["valid"].clone(), v["reason"].clone()),
        (json!(false), json!("not_found")),
        "[SEC] {v}"
    );
    let (s, _) = call(
        &app,
        "POST",
        "/api/tickets/validate",
        Some(&org),
        Some(json!({"qr_token": old})),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND, "[SEC] legacy validate endpoint too");
    assert_eq!(status_of(&pool, ids[0]).await.0, "active");
    let fresh = qr(&app, &vt, ids[0]).await;
    assert_eq!(scan(&app, &org, event, &fresh).await.1["valid"], true);

    // Gift-claim path.
    let old = qr(&app, &ut, ids[1]).await;
    let (_, gift) = call(
        &app,
        "POST",
        &format!("/api/tickets/{}/gift", ids[1]),
        Some(&ut),
        Some(json!({"email":"v@x.co"})),
    )
    .await;
    let (s, _) = call(
        &app,
        "POST",
        "/api/tickets/claim",
        Some(&vt),
        Some(json!({"token": gift["claim_token"]})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, v) = scan(&app, &org, event, &old).await;
    assert_eq!(v["valid"], false, "[SEC] {v}");
    assert_eq!(status_of(&pool, ids[1]).await.0, "active");
}

/// S14, S20, CT11 / D7: cancelled or blocked → event_cancelled and nothing changes; hidden still
/// scans. Both doors.
#[sqlx::test(migrations = "./migrations")]
async fn ct11_cancelled_or_blocked_event_admits_nobody(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let (u, ut) = user(&pool, "u@x.co", "Uma").await;
    let cancelled = free_event(&app, &org, 30).await;
    let blocked = free_event(&app, &org, 30).await;
    let hidden = free_event(&app, &org, 30).await;
    for e in [cancelled, blocked, hidden] {
        assert_eq!(reserve(&app, &ut, e, 2).await.0, StatusCode::CREATED);
    }
    assert_eq!(
        call(&app, "DELETE", &format!("/api/events/{cancelled}"), Some(&org), None)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    sqlx::query("UPDATE events SET status = 'blocked' WHERE id = $1")
        .bind(blocked)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE events SET status = 'hidden' WHERE id = $1")
        .bind(hidden)
        .execute(&pool)
        .await
        .unwrap();
    for e in [cancelled, blocked] {
        let ids = ticket_ids(&pool, u, e, "active").await;
        let code = qr(&app, &ut, ids[0]).await;
        let (s, v) = scan(&app, &org, e, &code).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(
            (v["valid"].clone(), v["reason"].clone()),
            (json!(false), json!("event_cancelled")),
            "[SEC] {v}"
        );
        assert!(v["holder_name"].is_null());
        let code = qr(&app, &ut, ids[1]).await;
        let (_, v) = call(
            &app,
            "POST",
            "/api/tickets/validate",
            Some(&org),
            Some(json!({"qr_token": code})),
        )
        .await;
        assert_eq!(
            (v["valid"].clone(), v["reason"].clone()),
            (json!(false), json!("event_cancelled")),
            "[SEC] {v}"
        );
        assert_eq!(ticket_ids(&pool, u, e, "active").await.len(), 2, "nothing changes");
    }
    let ids = ticket_ids(&pool, u, hidden, "active").await;
    let code = qr(&app, &ut, ids[0]).await;
    assert_eq!(
        scan(&app, &org, hidden, &code).await.1["valid"],
        true,
        "hidden still scans (D7)"
    );
}

/// S03, P10: a ticket of another event reveals nothing about that event or its holder.
#[sqlx::test(migrations = "./migrations")]
async fn ct11_wrong_event_reveals_nothing(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let (_, org2) = user(&pool, "o2@x.co", "Org2").await;
    let (u, ut) = user(&pool, "u@x.co", "Uma").await;
    let event = free_event(&app, &org, 30).await;
    let (_, other) = call(
        &app,
        "POST",
        "/api/events",
        Some(&org2),
        Some(
            json!({"title":"Secret Draft Party","category":"Poetry","price":"0","tickets_total":5,"cost_type":"free"}),
        ),
    )
    .await;
    let other: Uuid = other["id"].as_str().unwrap().parse().unwrap();
    assert_eq!(reserve(&app, &ut, other, 1).await.0, StatusCode::CREATED);
    let code = qr(&app, &ut, ticket_ids(&pool, u, other, "active").await[0]).await;
    let (s, text) = call_text(
        &app,
        "POST",
        &format!("/api/events/{event}/scan"),
        Some(&org),
        Some(json!({"qr_token": code})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let v = parse(&text);
    assert_eq!(v["reason"], "wrong_event");
    assert_eq!(v["event_title"], "");
    assert!(
        !text.contains("Secret Draft Party") && !text.contains("Uma"),
        "[SEC] {text}"
    );
}

// ── CT12: staff management ──────────────────────────────────────────────────────────

/// S07, I03, I10, I11, I12 and accept → {event_id}.
#[sqlx::test(migrations = "./migrations")]
async fn ct12_organiser_lists_revokes_and_removes_staff(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let (s_id, st) = user(&pool, "s@x.co", "Stu").await;
    let (_, vt) = user(&pool, "v@x.co", "Vik").await;
    let (u, ut) = user(&pool, "u@x.co", "Uma").await;
    let event = free_event(&app, &org, 30).await;
    assert_eq!(reserve(&app, &ut, event, 2).await.0, StatusCode::CREATED);
    let ids = ticket_ids(&pool, u, event, "active").await;

    let tok = invite(&app, &org, event).await;
    let (s, v) = call(
        &app,
        "POST",
        "/api/staff/accept",
        Some(&st),
        Some(json!({ "token": tok })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v, json!({ "event_id": event }));

    let pending = invite(&app, &org, event).await;
    let path = format!("/api/events/{event}/staff");
    let (s, list) = call(&app, "GET", &path, Some(&org), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(list["staff"].as_array().unwrap().len(), 1);
    assert_eq!(list["staff"][0]["name"], "Stu");
    assert_eq!(list["staff"][0]["user_id"], json!(s_id));
    let invites = list["invites"].as_array().unwrap();
    assert_eq!(invites.len(), 1, "only the open invite: {list}");
    assert_eq!(invites[0]["status"], "pending");
    let invite_id = invites[0]["id"].as_str().unwrap().to_string();

    for token in [&st, &vt] {
        assert_eq!(
            call(&app, "GET", &path, Some(token), None).await.0,
            StatusCode::FORBIDDEN,
            "[SEC] I10"
        );
        assert_eq!(
            call(
                &app,
                "DELETE",
                &format!("{path}/invites/{invite_id}"),
                Some(token),
                None
            )
            .await
            .0,
            StatusCode::FORBIDDEN,
            "[SEC] I12 revoke"
        );
        assert_eq!(
            call(&app, "DELETE", &format!("{path}/{s_id}"), Some(token), None)
                .await
                .0,
            StatusCode::FORBIDDEN,
            "[SEC] I12 remove"
        );
    }

    let (s, _) = call(&app, "DELETE", &format!("{path}/invites/{invite_id}"), Some(&org), None).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (s, _) = call(
        &app,
        "POST",
        "/api/staff/accept",
        Some(&vt),
        Some(json!({ "token": pending })),
    )
    .await;
    assert_eq!(
        s,
        StatusCode::NOT_FOUND,
        "[SEC] I11 a revoked invite cannot be accepted"
    );
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM event_staff WHERE event_id = $1")
        .bind(event)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 1);
    assert_eq!(
        call(&app, "DELETE", &format!("{path}/invites/{invite_id}"), Some(&org), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );

    let code = qr(&app, &ut, ids[0]).await;
    assert_eq!(
        scan(&app, &st, event, &code).await.1["valid"],
        true,
        "staff scans before removal"
    );
    let (s, _) = call(&app, "DELETE", &format!("{path}/{s_id}"), Some(&org), None).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let code = qr(&app, &ut, ids[1]).await;
    assert_eq!(
        scan(&app, &st, event, &code).await.0,
        StatusCode::FORBIDDEN,
        "[SEC] S07 removed staff"
    );
    assert_eq!(status_of(&pool, ids[1]).await.0, "active");
    let (_, mine) = call(&app, "GET", "/api/me/staff/events", Some(&st), None).await;
    assert!(
        mine.as_array().unwrap().is_empty(),
        "the event left the removed staff's list"
    );
    assert_eq!(
        call(&app, "DELETE", &format!("{path}/{s_id}"), Some(&org), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

/// I06: the organiser cannot burn their own invite.
#[sqlx::test(migrations = "./migrations")]
async fn ct12_organiser_cannot_accept_own_invite(pool: PgPool) {
    let app = make_app(pool.clone());
    let (o, org) = user(&pool, "o@x.co", "Org").await;
    let event = free_event(&app, &org, 30).await;
    let tok = invite(&app, &org, event).await;
    let (s, _) = call(
        &app,
        "POST",
        "/api/staff/accept",
        Some(&org),
        Some(json!({ "token": tok })),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);
    let status: String = sqlx::query_scalar("SELECT status FROM staff_invites WHERE event_id = $1")
        .bind(event)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "pending", "the invite is still usable by the intended person");
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM event_staff WHERE user_id = $1")
        .bind(o)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 0);
}

/// I08: no invite for a cancelled (or blocked) event.
#[sqlx::test(migrations = "./migrations")]
async fn ct12_no_invite_for_a_cancelled_event(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let event = free_event(&app, &org, 30).await;
    call(&app, "DELETE", &format!("/api/events/{event}"), Some(&org), None).await;
    let (s, _) = call(
        &app,
        "POST",
        &format!("/api/events/{event}/staff/invite"),
        Some(&org),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);
    let blocked = free_event(&app, &org, 30).await;
    sqlx::query("UPDATE events SET status = 'blocked' WHERE id = $1")
        .bind(blocked)
        .execute(&pool)
        .await
        .unwrap();
    let (s, _) = call(
        &app,
        "POST",
        &format!("/api/events/{blocked}/staff/invite"),
        Some(&org),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM staff_invites")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 0);
}

// ── CT15: allow-lists and the PII marker sweep ──────────────────────────────────────

/// G02, S19, I10, ERR01: no organiser- or staff-facing body contains an email, the marker, or a
/// key outside its allow-list — including error bodies and a GDPR-deleted attendee.
#[sqlx::test(migrations = "./migrations")]
async fn ct15_no_email_reaches_organiser_or_staff(pool: PgPool) {
    const MARK: &str = "pii-mk-7f3a";
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, &format!("org.{MARK}@x.co"), "Org").await;
    let (_, st) = user(&pool, &format!("staff.{MARK}@x.co"), "Stu").await;
    let (u, ut) = user(&pool, &format!("uma.{MARK}@x.co"), "Uma").await;
    let (gone, gone_t) = user(&pool, &format!("gone.{MARK}@x.co"), "Gone").await;
    let (_, lp) = user(&pool, &format!("{MARK}@x.co"), MARK).await;
    let event = free_event(&app, &org, 30).await;
    let other = free_event(&app, &org, 30).await;

    let mut bodies: Vec<(String, String)> = Vec::new();
    let tok = invite(&app, &org, event).await;
    let (_, t) = call_text(
        &app,
        "POST",
        "/api/staff/accept",
        Some(&st),
        Some(json!({ "token": tok })),
    )
    .await;
    bodies.push(("accept".into(), t));
    let (_, t) = call_text(
        &app,
        "POST",
        &format!("/api/events/{event}/staff/invite"),
        Some(&org),
        None,
    )
    .await;
    bodies.push(("invite create".into(), t));

    assert_eq!(reserve(&app, &ut, event, 5).await.0, StatusCode::CREATED);
    assert_eq!(reserve(&app, &lp, event, 1).await.0, StatusCode::CREATED);
    assert_eq!(reserve(&app, &ut, other, 1).await.0, StatusCode::CREATED);
    let (s, t) = call_text(
        &app,
        "POST",
        &format!("/api/events/{event}/reserve"),
        Some(&ut),
        Some(json!({"quantity":1})),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);
    bodies.push(("cap error".into(), t));

    // A GDPR-deleted attendee with one scanned place stays on the list as "deleted user".
    assert_eq!(reserve(&app, &gone_t, event, 2).await.0, StatusCode::CREATED);
    let gone_ticket = ticket_ids(&pool, gone, event, "active").await[0];
    let code = qr(&app, &gone_t, gone_ticket).await;
    assert_eq!(scan(&app, &st, event, &code).await.1["valid"], true);
    sqlx::query("UPDATE users SET password_hash = NULL WHERE id = $1")
        .bind(gone)
        .execute(&pool)
        .await
        .unwrap();
    let (s, _) = call(
        &app,
        "DELETE",
        "/api/me",
        Some(&gone_t),
        Some(json!({"confirm":"DELETE"})),
    )
    .await;
    assert_eq!(s, StatusCode::NO_CONTENT);

    let ids = ticket_ids(&pool, u, event, "active").await;
    let scan_keys = set(&["valid", "reason", "event_title", "holder_name", "places", "checked_in"]);
    let code = qr(&app, &ut, ids[0]).await;
    let elsewhere = qr(&app, &ut, ticket_ids(&pool, u, other, "active").await[0]).await;
    let mut scan_bodies = Vec::new();
    for (label, q) in [
        ("scan valid", code.clone()),
        ("scan already_used", code),
        ("scan not_found", "garbage".to_string()),
        ("scan wrong_event", elsewhere),
    ] {
        let (_, t) = call_text(
            &app,
            "POST",
            &format!("/api/events/{event}/scan"),
            Some(&st),
            Some(json!({ "qr_token": q })),
        )
        .await;
        scan_bodies.push((label.to_string(), t));
    }
    for (label, t) in &scan_bodies {
        assert!(keys(&parse(t)).is_subset(&scan_keys), "[PII] {label}: {t}");
    }
    bodies.extend(scan_bodies);

    let (s, t) = call_text(&app, "GET", &format!("/api/events/{event}/guests"), Some(&org), None).await;
    assert_eq!(s, StatusCode::OK);
    let guests = parse(&t);
    for row in guests.as_array().unwrap() {
        assert!(
            keys(row).is_subset(&set(&["name", "places", "checked_in", "booked_at"])),
            "[PII] {row}"
        );
    }
    assert!(
        t.contains("Видалений користувач"),
        "the deleted attendee's scanned place is still listed: {t}"
    );
    bodies.push(("guest list".into(), t));
    let (_, t) = call_text(&app, "GET", &format!("/api/events/{event}/guests"), Some(&st), None).await;
    bodies.push(("guest list 403".into(), t));
    let (_, t) = call_text(&app, "GET", &format!("/api/events/{event}/tickets"), Some(&org), None).await;
    bodies.push(("ticket list".into(), t));

    let (_, t) = call_text(&app, "GET", &format!("/api/events/{event}/staff"), Some(&org), None).await;
    let staff = parse(&t);
    assert_eq!(keys(&staff), set(&["staff", "invites"]));
    for row in staff["staff"].as_array().unwrap() {
        assert_eq!(keys(row), set(&["user_id", "name", "added_at"]), "[PII] {row}");
    }
    for row in staff["invites"].as_array().unwrap() {
        assert_eq!(
            keys(row),
            set(&["id", "status", "expires_at", "created_at"]),
            "[SEC] {row}"
        );
    }
    bodies.push(("staff list".into(), t));
    let (_, t) = call_text(&app, "GET", &format!("/api/events/{event}/staff"), Some(&st), None).await;
    bodies.push(("staff list 403".into(), t));
    let (_, t) = call_text(&app, "GET", "/api/me/staff/events", Some(&st), None).await;
    bodies.push(("my staff events".into(), t));

    for (label, body) in &bodies {
        assert!(!body.contains(MARK), "[PII] {label} leaks the marker: {body}");
        assert!(!body.contains("@deleted.invalid"), "[PII] {label}: {body}");
        assert!(
            !body.contains('@'),
            "[PII] {label} carries an email-like string: {body}"
        );
        assert!(!body.contains("token_hash"), "[SEC] {label}: {body}");
    }
}

// ── Event DTO ───────────────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn event_dto_carries_tickets_total(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let (_, ut) = user(&pool, "u@x.co", "Uma").await;
    let event = free_event(&app, &org, 30).await;
    assert_eq!(reserve(&app, &ut, event, 3).await.0, StatusCode::CREATED);
    let (_, v) = call(&app, "GET", &format!("/api/events/{event}"), None, None).await;
    assert_eq!(
        (v["tickets_total"].as_i64(), v["tickets_left"].as_i64()),
        (Some(30), Some(27))
    );
    let (_, mine) = call(&app, "GET", "/api/me/events", Some(&org), None).await;
    assert_eq!(mine[0]["tickets_total"], 30);
}

// ── P2: moderation and cancellation stick ───────────────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn p2_organiser_cannot_undo_moderation_or_cancellation(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let (admin_id, admin) = user(&pool, "admin@x.co", "Admin").await;
    sqlx::query("UPDATE users SET role = 'ADMIN' WHERE id = $1")
        .bind(admin_id)
        .execute(&pool)
        .await
        .unwrap();
    let event = free_event(&app, &org, 30).await;
    let path = format!("/api/events/{event}");
    let current = move |pool: PgPool| async move {
        sqlx::query_scalar::<_, String>("SELECT status FROM events WHERE id = $1")
            .bind(event)
            .fetch_one(&pool)
            .await
            .unwrap()
    };

    for moderated in ["hidden", "blocked"] {
        let (s, _) = call(
            &app,
            "PATCH",
            &format!("/api/admin/events/{event}"),
            Some(&admin),
            Some(json!({ "status": moderated })),
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let (s, _) = call(&app, "PUT", &path, Some(&org), Some(json!({"status":"published"}))).await;
        assert_eq!(
            s,
            StatusCode::CONFLICT,
            "[SEC] organiser re-publishes an admin-{moderated} event"
        );
        assert_eq!(current(pool.clone()).await, moderated);
        let (s, _) = call(&app, "PUT", &path, Some(&org), Some(json!({"title":"Renamed"}))).await;
        assert_eq!(s, StatusCode::OK, "other edits are not blocked");
    }

    // The admin restores it; the organiser's own hide/re-publish works again.
    call(
        &app,
        "PATCH",
        &format!("/api/admin/events/{event}"),
        Some(&admin),
        Some(json!({"status":"published"})),
    )
    .await;
    assert_eq!(
        call(&app, "PUT", &path, Some(&org), Some(json!({"status":"hidden"})))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&app, "PUT", &path, Some(&org), Some(json!({"status":"published"})))
            .await
            .0,
        StatusCode::OK
    );

    assert_eq!(
        call(&app, "DELETE", &path, Some(&org), None).await.0,
        StatusCode::NO_CONTENT
    );
    let (s, _) = call(&app, "PUT", &path, Some(&org), Some(json!({"status":"published"}))).await;
    assert_eq!(s, StatusCode::CONFLICT, "[SEC] no un-cancelling");
    assert_eq!(current(pool.clone()).await, "cancelled");
}

// ── P9: friends are mutual, published events only ──────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn p9_attendance_is_shown_to_mutual_friends_on_published_events_only(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let (viewer_id, viewer) = user(&pool, "v@x.co", "Vik").await;
    let (carol_id, carol) = user(&pool, "c@x.co", "Carol").await;
    let event = free_event(&app, &org, 30).await;
    assert_eq!(reserve(&app, &carol, event, 1).await.0, StatusCode::CREATED);
    call(&app, "POST", &format!("/api/events/{event}/save"), Some(&carol), None).await;
    call(
        &app,
        "POST",
        &format!("/api/users/{carol_id}/follow"),
        Some(&viewer),
        None,
    )
    .await;
    let social = format!("/api/events/{event}/social");

    let (_, s) = call(&app, "GET", &social, Some(&viewer), None).await;
    assert!(
        s["friends_going"].as_array().unwrap().is_empty(),
        "[SEC] one-sided follow: {s}"
    );
    assert!(
        s["friends_interested"].as_array().unwrap().is_empty(),
        "[SEC] one-sided follow: {s}"
    );

    call(
        &app,
        "POST",
        &format!("/api/users/{viewer_id}/follow"),
        Some(&carol),
        None,
    )
    .await;
    let (_, s) = call(&app, "GET", &social, Some(&viewer), None).await;
    assert_eq!(s["friends_going"][0]["name"], "Carol", "mutual follow: {s}");
    assert_eq!(s["friends_interested"][0]["name"], "Carol");

    sqlx::query("UPDATE events SET status = 'hidden' WHERE id = $1")
        .bind(event)
        .execute(&pool)
        .await
        .unwrap();
    // Fix batch 1, item 10: a non-published event has no social data at all (was: 200 with
    // empty friend lists).
    let (status, _) = call(&app, "GET", &social, Some(&viewer), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "[SEC] hidden event");
}

// ── P12, P15, P16 ───────────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn p12_register_caps_the_name(pool: PgPool) {
    let app = make_app(pool);
    let (s, _) = call(
        &app,
        "POST",
        "/api/auth/register",
        None,
        Some(json!({"email":"long@x.co","password":"password1","name":"a".repeat(81)})),
    )
    .await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    let (s, _) = call(
        &app,
        "POST",
        "/api/auth/register",
        None,
        Some(json!({"email":"ok@x.co","password":"password1","name":"a".repeat(80)})),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
}

#[sqlx::test(migrations = "./migrations")]
async fn p15_passwordless_account_can_delete_itself(pool: PgPool) {
    let app = make_app(pool.clone());
    let pl = underground_api::users::create_passwordless_user(&pool, "nopw@x.co")
        .await
        .unwrap();
    let t = underground_api::auth::issue_token(pl.id, SECRET).unwrap();
    for body in [json!({}), json!({"confirm":"delete"}), json!({"password":"x"})] {
        let (s, _) = call(&app, "DELETE", "/api/me", Some(&t), Some(body.clone())).await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    }
    let (s, _) = call(&app, "DELETE", "/api/me", Some(&t), Some(json!({"confirm":"DELETE"}))).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let blocked: bool = sqlx::query_scalar("SELECT blocked FROM users WHERE id = $1")
        .bind(pl.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(blocked);

    // A password account cannot skip the password with the confirm word.
    let (s, v) = call(
        &app,
        "POST",
        "/api/auth/register",
        None,
        Some(json!({"email":"pw@x.co","password":"password1","name":"Pat"})),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    let pt = v["token"].as_str().unwrap().to_string();
    let (s, _) = call(&app, "DELETE", "/api/me", Some(&pt), Some(json!({"confirm":"DELETE"}))).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "[SEC] password still required");
    assert_eq!(call(&app, "GET", "/api/me", Some(&pt), None).await.0, StatusCode::OK);
}

#[sqlx::test(migrations = "./migrations")]
async fn p16_deleting_an_account_returns_its_free_places(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org) = user(&pool, "o@x.co", "Org").await;
    let (u, ut) = user(&pool, "u@x.co", "Uma").await;
    sqlx::query("UPDATE users SET password_hash = NULL WHERE id = $1")
        .bind(u)
        .execute(&pool)
        .await
        .unwrap();
    let event = free_event(&app, &org, 10).await;
    assert_eq!(reserve(&app, &ut, event, 3).await.0, StatusCode::CREATED);
    let ids = ticket_ids(&pool, u, event, "active").await;
    let code = qr(&app, &ut, ids[0]).await;
    assert_eq!(scan(&app, &org, event, &code).await.1["valid"], true);
    assert_eq!(tickets_left(&pool, event).await, 7);
    let (s, _) = call(&app, "DELETE", "/api/me", Some(&ut), Some(json!({"confirm":"DELETE"}))).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    assert_eq!(tickets_left(&pool, event).await, 9, "two unused places back on sale");
    assert_eq!(status_of(&pool, ids[0]).await.0, "used", "the scanned one stays used");
}

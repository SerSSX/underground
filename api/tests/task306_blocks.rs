//! TASK-306 — person-to-person blocking. Integration tests written from the abuse-case table
//! `docs/backlog/TASK-306-abuse-cases.md` (the spec), not from the implementation. Each test names
//! the rows it covers. A = blocker, B = blocked, C = third user, X = admin.

use std::sync::atomic::{AtomicU32, Ordering};

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use underground_api::routes::router;
use underground_api::state::AppState;
use uuid::Uuid;

// ---------------------------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------------------------

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

async fn send_raw(app: &Router, req: Request<Body>) -> (StatusCode, Vec<u8>) {
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    (status, bytes.to_vec())
}

async fn send(app: &Router, req: Request<Body>) -> (StatusCode, Value) {
    let (status, bytes) = send_raw(app, req).await;
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()))
    };
    (status, body)
}

/// Each signup comes from its own forwarded address, so the per-IP signup cap never trips.
static NEXT_IP: AtomicU32 = AtomicU32::new(1);

async fn register(app: &Router, email: &str, name: &str) -> (String, Uuid) {
    let n = NEXT_IP.fetch_add(1, Ordering::SeqCst);
    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/register")
        .header("content-type", "application/json")
        .header("x-forwarded-for", format!("10.{}.{}.{}", (n >> 16) & 255, (n >> 8) & 255, n & 255))
        .body(Body::from(
            json!({"email": email, "password": "password1", "name": name}).to_string(),
        ))
        .unwrap();
    let (status, v) = send(app, req).await;
    assert!(status.is_success(), "register {email}: {status} {v}");
    (
        v["token"].as_str().unwrap().to_string(),
        v["user"]["id"].as_str().unwrap().parse().unwrap(),
    )
}

async fn block(app: &Router, tok: &str, id: Uuid) -> StatusCode {
    send(app, build("POST", &format!("/api/users/{id}/block"), Some(tok), None)).await.0
}

async fn unblock(app: &Router, tok: &str, id: Uuid) -> StatusCode {
    send(app, build("DELETE", &format!("/api/users/{id}/block"), Some(tok), None)).await.0
}

async fn follow(app: &Router, tok: &str, id: Uuid) -> StatusCode {
    send(app, build("POST", &format!("/api/users/{id}/follow"), Some(tok), None)).await.0
}

async fn dm(app: &Router, tok: &str, to: Uuid, text: &str) -> (StatusCode, Value) {
    send(
        app,
        build("POST", "/api/messages", Some(tok), Some(json!({"recipient_id": to, "text": text}))),
    )
    .await
}

async fn profile(app: &Router, tok: Option<&str>, id: Uuid) -> (StatusCode, Value) {
    send(app, build("GET", &format!("/api/users/{id}"), tok, None)).await
}

async fn get_json(app: &Router, tok: Option<&str>, path: &str) -> Value {
    let (status, v) = send(app, build("GET", path, tok, None)).await;
    assert_eq!(status, StatusCode::OK, "GET {path}: {v}");
    v
}

async fn notif_list(app: &Router, tok: &str) -> Vec<Value> {
    get_json(app, Some(tok), "/api/me/notifications").await.as_array().unwrap().clone()
}

async fn unread(app: &Router, tok: &str) -> (i64, i64) {
    let v = get_json(app, Some(tok), "/api/me/unread").await;
    (v["notifications"].as_i64().unwrap(), v["messages"].as_i64().unwrap())
}

async fn create_post(app: &Router, tok: &str, text: &str) -> Uuid {
    let (status, v) = send(app, build("POST", "/api/posts", Some(tok), Some(json!({"text": text})))).await;
    assert_eq!(status, StatusCode::CREATED, "{v}");
    v["id"].as_str().unwrap().parse().unwrap()
}

async fn like(app: &Router, tok: &str, post: Uuid) -> (StatusCode, Value) {
    send(app, build("POST", &format!("/api/posts/{post}/like"), Some(tok), None)).await
}

async fn comment(app: &Router, tok: &str, post: Uuid, text: &str) -> (StatusCode, Value) {
    send(
        app,
        build("POST", &format!("/api/posts/{post}/comments"), Some(tok), Some(json!({"text": text}))),
    )
    .await
}

async fn comments_of(app: &Router, tok: Option<&str>, post: Uuid) -> Vec<Value> {
    get_json(app, tok, &format!("/api/posts/{post}/comments")).await.as_array().unwrap().clone()
}

async fn free_event(app: &Router, tok: &str, title: &str, category: &str, city: Option<&str>) -> Uuid {
    let (status, v) = send(
        app,
        build(
            "POST",
            "/api/events",
            Some(tok),
            Some(json!({"title": title, "category": category, "city": city, "price": "0",
                        "tickets_total": 40, "cost_type": "free"})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{v}");
    v["id"].as_str().unwrap().parse().unwrap()
}

async fn reserve(app: &Router, tok: &str, event: Uuid, qty: i64) -> (StatusCode, Value) {
    send(
        app,
        build("POST", &format!("/api/events/{event}/reserve"), Some(tok), Some(json!({"quantity": qty}))),
    )
    .await
}

async fn put_prefs(app: &Router, tok: &str, categories: Value, city: Value) {
    let (status, v) = send(
        app,
        build("PUT", "/api/me/prefs", Some(tok), Some(json!({"categories": categories, "city": city}))),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
}

/// Ticket ids `owner` holds on `event`, oldest first.
async fn tickets_of(pool: &PgPool, owner: Uuid, event: Uuid) -> Vec<Uuid> {
    sqlx::query_scalar("SELECT id FROM tickets WHERE owner_id = $1 AND event_id = $2 ORDER BY created_at, id")
        .bind(owner)
        .bind(event)
        .fetch_all(pool)
        .await
        .unwrap()
}

async fn n(pool: &PgPool, sql: &str, a: Uuid, b: Uuid) -> i64 {
    sqlx::query_scalar(sql).bind(a).bind(b).fetch_one(pool).await.unwrap()
}

async fn block_row(pool: &PgPool, a: Uuid, b: Uuid) -> i64 {
    n(pool, "SELECT count(*) FROM user_blocks WHERE blocker_id = $1 AND blocked_id = $2", a, b).await
}

async fn blocks_of(pool: &PgPool, a: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM user_blocks WHERE blocker_id = $1")
        .bind(a)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn all_blocks(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM user_blocks").fetch_one(pool).await.unwrap()
}

async fn follows_between(pool: &PgPool, a: Uuid, b: Uuid) -> i64 {
    n(
        pool,
        "SELECT count(*) FROM follows WHERE (follower_id = $1 AND followee_id = $2) \
            OR (follower_id = $2 AND followee_id = $1)",
        a,
        b,
    )
    .await
}

async fn msgs(pool: &PgPool, from: Uuid, to: Uuid) -> i64 {
    n(pool, "SELECT count(*) FROM messages WHERE sender_id = $1 AND recipient_id = $2", from, to).await
}

async fn alerts_from(pool: &PgPool, to: Uuid, actor: Uuid) -> i64 {
    n(pool, "SELECT count(*) FROM notifications WHERE user_id = $1 AND actor_id = $2", to, actor).await
}

/// `count` plain users inserted directly (no password, never sign in).
async fn seed_users(pool: &PgPool, count: i32) -> Vec<Uuid> {
    let tag = Uuid::new_v4().simple().to_string();
    sqlx::query_scalar(
        "INSERT INTO users (email, name) \
         SELECT 's' || g || '-' || $1 || '@seed.test', 'Seed ' || g FROM generate_series(1, $2) g RETURNING id",
    )
    .bind(tag)
    .bind(count)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn seed_blocks(pool: &PgPool, blocker: Uuid, ids: &[Uuid]) {
    sqlx::query("INSERT INTO user_blocks (blocker_id, blocked_id) SELECT $1, unnest($2::uuid[])")
        .bind(blocker)
        .bind(ids.to_vec())
        .execute(pool)
        .await
        .unwrap();
}

async fn admin_ban(pool: &PgPool, id: Uuid) {
    sqlx::query("UPDATE users SET blocked = true WHERE id = $1").bind(id).execute(pool).await.unwrap();
}

async fn clear_rate(pool: &PgPool) {
    sqlx::query("DELETE FROM rate_events").execute(pool).await.unwrap();
}

fn ids_in(list: &[Value], key: &str) -> Vec<String> {
    list.iter().map(|v| v[key].as_str().unwrap_or_default().to_string()).collect()
}

// ---------------------------------------------------------------------------------------------
// 1. Block endpoints (S1)
// ---------------------------------------------------------------------------------------------

/// AB1, AB2, AB3, AB4, AB114 (B has not blocked A).
#[sqlx::test(migrations = "./migrations")]
async fn ab1_ab4_block_is_idempotent_and_unblock_restores_flag_not_follows(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (tb, b) = register(&app, "b@x.co", "Bob").await;
    assert_eq!(follow(&app, &ta, b).await, StatusCode::NO_CONTENT);
    assert_eq!(follow(&app, &tb, a).await, StatusCode::NO_CONTENT);
    assert_eq!(follows_between(&pool, a, b).await, 2);

    // AB1
    let (status, body) = send_raw(&app, build("POST", &format!("/api/users/{b}/block"), Some(&ta), None)).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "AB1");
    assert!(body.is_empty(), "AB1/AB114: empty body");
    assert_eq!(block_row(&pool, a, b).await, 1, "AB1: one row");
    assert_eq!(follows_between(&pool, a, b).await, 0, "AB1: both follows gone");
    let (_, p) = profile(&app, Some(&ta), b).await;
    assert_eq!(p["is_blocked"], json!(true), "AB1: is_blocked");
    let t1: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT created_at FROM user_blocks WHERE blocker_id = $1 AND blocked_id = $2")
            .bind(a)
            .bind(b)
            .fetch_one(&pool)
            .await
            .unwrap();

    // AB2
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT, "AB2");
    assert_eq!(block_row(&pool, a, b).await, 1, "AB2: still one row");
    let t2: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT created_at FROM user_blocks WHERE blocker_id = $1 AND blocked_id = $2")
            .bind(a)
            .bind(b)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(t1, t2, "AB2/D3: created_at unchanged");

    // AB3
    let (status, body) = send_raw(&app, build("DELETE", &format!("/api/users/{b}/block"), Some(&ta), None)).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "AB3");
    assert!(body.is_empty(), "AB114: empty body");
    assert_eq!(block_row(&pool, a, b).await, 0, "AB3: row gone");
    let (_, p) = profile(&app, Some(&ta), b).await;
    assert_eq!(p["is_blocked"], json!(false), "AB3");
    assert_eq!(follows_between(&pool, a, b).await, 0, "AB3: no follow recreated");

    // AB4
    assert_eq!(unblock(&app, &ta, b).await, StatusCode::NO_CONTENT, "AB4");
    assert_eq!(all_blocks(&pool).await, 0);
}

/// AB5, AB6, AB8, AB10.
#[sqlx::test(migrations = "./migrations")]
async fn ab5_ab6_ab8_ab10_self_unknown_and_banned_targets(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (_, banned) = register(&app, "bb@x.co", "Banned").await;
    admin_ban(&pool, banned).await;

    // AB5
    assert_eq!(block(&app, &ta, a).await, StatusCode::UNPROCESSABLE_ENTITY, "AB5 POST self");
    assert_eq!(unblock(&app, &ta, a).await, StatusCode::UNPROCESSABLE_ENTITY, "AB5 DELETE self");
    // AB6
    let random = Uuid::new_v4();
    assert_eq!(block(&app, &ta, random).await, StatusCode::NOT_FOUND, "AB6 random");
    assert_eq!(block(&app, &ta, Uuid::nil()).await, StatusCode::NOT_FOUND, "AB6 nil");
    // AB8
    assert_eq!(block(&app, &ta, banned).await, StatusCode::NOT_FOUND, "AB8 admin-banned");
    // AB10
    assert_eq!(unblock(&app, &ta, Uuid::new_v4()).await, StatusCode::NOT_FOUND, "AB10");
    assert_eq!(all_blocks(&pool).await, 0, "no row from any of these");
}

/// AB9: a row stays removable after the target is admin-banned.
#[sqlx::test(migrations = "./migrations")]
async fn ab9_unblock_after_target_admin_banned(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (_, b) = register(&app, "b@x.co", "Bob").await;
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    admin_ban(&pool, b).await;
    assert_eq!(unblock(&app, &ta, b).await, StatusCode::NO_CONTENT, "AB9");
    assert_eq!(block_row(&pool, a, b).await, 0, "AB9: removed");
}

/// AB14.
#[sqlx::test(migrations = "./migrations")]
async fn ab14_anonymous_is_401(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, b) = register(&app, "b@x.co", "Bob").await;
    let (s, _) = send(&app, build("POST", &format!("/api/users/{b}/block"), None, None)).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _) = send(&app, build("DELETE", &format!("/api/users/{b}/block"), None, None)).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _) = send(&app, build("GET", "/api/me/blocks", None, None)).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    assert_eq!(all_blocks(&pool).await, 0);
}

/// AB17: the body is ignored (mass assignment).
#[sqlx::test(migrations = "./migrations")]
async fn ab17_body_is_ignored(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (_, b) = register(&app, "b@x.co", "Bob").await;
    let (_, c) = register(&app, "c@x.co", "Carol").await;
    let (_, d) = register(&app, "d@x.co", "Dave").await;
    let (s, _) = send(
        &app,
        build(
            "POST",
            &format!("/api/users/{b}/block"),
            Some(&ta),
            Some(json!({"blocker_id": c, "blocked_id": d, "created_at": "2000-01-01T00:00:00Z"})),
        ),
    )
    .await;
    assert_eq!(s, StatusCode::NO_CONTENT, "AB17");
    assert_eq!(all_blocks(&pool).await, 1, "AB17: only one row");
    assert_eq!(block_row(&pool, a, b).await, 1, "AB17: it is (A,B)");
    let fresh: bool = sqlx::query_scalar(
        "SELECT created_at > now() - interval '5 minutes' FROM user_blocks WHERE blocker_id = $1 AND blocked_id = $2",
    )
    .bind(a)
    .bind(b)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(fresh, "AB17: created_at is now, not 2000");
}

/// AB19, AB20: IDOR on unblock and on the list.
#[sqlx::test(migrations = "./migrations")]
async fn ab19_ab20_a_caller_touches_only_own_rows(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (_, b) = register(&app, "b@x.co", "Bob").await;
    let (tc, _) = register(&app, "c@x.co", "Carol").await;
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    // AB19
    assert_eq!(unblock(&app, &tc, b).await, StatusCode::NO_CONTENT, "AB19: C's own no-op");
    assert_eq!(block_row(&pool, a, b).await, 1, "AB19: (A,B) still exists");
    // AB20
    for q in ["", "?user_id=", "?blocker_id="] {
        let path = if q.is_empty() { "/api/me/blocks".to_string() } else { format!("/api/me/blocks{q}{a}") };
        let v = get_json(&app, Some(&tc), &path).await;
        assert_eq!(v, json!([]), "AB20 {path}");
    }
}

/// AB22, D4: the list shape, admin-banned targets included, no email.
#[sqlx::test(migrations = "./migrations")]
async fn ab22_list_shape(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, _a) = register(&app, "a@x.co", "Alice").await;
    let (_, b) = register(&app, "bob.secret@x.co", "Bob").await;
    let (_, b2) = register(&app, "b2.secret@x.co", "гість").await;
    sqlx::query("UPDATE users SET avatar_url = '/uploads/bob.webp' WHERE id = $1")
        .bind(b)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    assert_eq!(block(&app, &ta, b2).await, StatusCode::NO_CONTENT);
    admin_ban(&pool, b2).await;

    let (s, raw) = send_raw(&app, build("GET", "/api/me/blocks", Some(&ta), None)).await;
    assert_eq!(s, StatusCode::OK);
    let text = String::from_utf8(raw).unwrap();
    assert!(!text.contains("secret@x.co"), "AB22: no email in the list");
    let list: Vec<Value> = serde_json::from_str(&text).unwrap();
    assert_eq!(list.len(), 2, "AB22/D4: admin-banned target still listed");
    for e in &list {
        let obj = e.as_object().unwrap();
        assert!(obj.contains_key("id") && obj.contains_key("name") && obj.contains_key("avatar_url"));
        assert!(obj.contains_key("blocked_at"), "AB22: when");
        assert!(!obj.contains_key("email"), "AB22: no email key");
    }
    let eb = list.iter().find(|e| e["id"] == json!(b)).expect("B listed");
    assert_eq!(eb["name"], "Bob");
    assert_eq!(eb["avatar_url"], "/uploads/bob.webp");
    let eb2 = list.iter().find(|e| e["id"] == json!(b2)).expect("B'' listed");
    assert_eq!(eb2["name"], "Гість", "AB22: display-name rule");
}

/// AB23, AB24, AB25, AB26 (D1).
#[sqlx::test(migrations = "./migrations")]
async fn ab23_ab26_cap_of_1000(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (_, b) = register(&app, "b@x.co", "Bob").await;
    let seeded = seed_users(&pool, 998).await;
    seed_blocks(&pool, a, &seeded).await;
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    assert_eq!(blocks_of(&pool, a).await, 999);

    // AB23
    let x1 = seed_users(&pool, 1).await[0];
    assert_eq!(block(&app, &ta, x1).await, StatusCode::NO_CONTENT, "AB23: 1000th");
    assert_eq!(blocks_of(&pool, a).await, 1000);

    // AB24
    let y = seed_users(&pool, 1).await[0];
    sqlx::query("INSERT INTO follows (follower_id, followee_id) VALUES ($1, $2), ($2, $1)")
        .bind(a)
        .bind(y)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(block(&app, &ta, y).await, StatusCode::CONFLICT, "AB24: 1001st → 409");
    assert_eq!(blocks_of(&pool, a).await, 1000, "AB24");
    assert_eq!(block_row(&pool, a, y).await, 0, "AB24: no row for Y");
    assert_eq!(follows_between(&pool, a, y).await, 2, "AB24: follows untouched");

    // AB25
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT, "AB25: repeat at the cap");
    assert_eq!(blocks_of(&pool, a).await, 1000);

    // AB26
    assert_eq!(unblock(&app, &ta, b).await, StatusCode::NO_CONTENT, "AB26 delete");
    assert_eq!(block(&app, &ta, y).await, StatusCode::NO_CONTENT, "AB26: cap freed");
    assert_eq!(blocks_of(&pool, a).await, 1000);
}

/// AB27: two concurrent blocks at 999 → exactly one wins.
#[sqlx::test(migrations = "./migrations")]
async fn ab27_cap_under_concurrency(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let seeded = seed_users(&pool, 999).await;
    seed_blocks(&pool, a, &seeded).await;
    for round in 0..10 {
        let xy = seed_users(&pool, 2).await;
        let (r1, r2) = tokio::join!(block(&app, &ta, xy[0]), block(&app, &ta, xy[1]));
        let mut got = [r1.as_u16(), r2.as_u16()];
        got.sort();
        assert_eq!(got, [204, 409], "AB27 round {round}");
        assert_eq!(blocks_of(&pool, a).await, 1000, "AB27 round {round}");
        sqlx::query("DELETE FROM user_blocks WHERE blocker_id = $1 AND blocked_id = ANY($2)")
            .bind(a)
            .bind(xy.clone())
            .execute(&pool)
            .await
            .unwrap();
    }
}

/// AB29: two concurrent blocks of the same person → both 204, one row.
#[sqlx::test(migrations = "./migrations")]
async fn ab29_same_block_twice_concurrently(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (_, b) = register(&app, "b@x.co", "Bob").await;
    for round in 0..10 {
        let (r1, r2) = tokio::join!(block(&app, &ta, b), block(&app, &ta, b));
        assert_eq!((r1, r2), (StatusCode::NO_CONTENT, StatusCode::NO_CONTENT), "AB29 round {round}");
        assert_eq!(block_row(&pool, a, b).await, 1, "AB29 round {round}");
        sqlx::query("DELETE FROM user_blocks").execute(&pool).await.unwrap();
    }
}

/// AB31 (D5): block and unblock share one bucket of 30 a minute.
#[sqlx::test(migrations = "./migrations")]
async fn ab31_toggle_rate_limit(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (_, b) = register(&app, "b@x.co", "Bob").await;
    for i in 1..=30 {
        let s = if i % 2 == 1 { block(&app, &ta, b).await } else { unblock(&app, &ta, b).await };
        assert_eq!(s, StatusCode::NO_CONTENT, "AB31 call {i}");
    }
    assert_eq!(block_row(&pool, a, b).await, 0, "after call 30 (an unblock)");
    assert_eq!(block(&app, &ta, b).await, StatusCode::TOO_MANY_REQUESTS, "AB31 call 31");
    assert_eq!(block_row(&pool, a, b).await, 0, "AB31: no state change on 429");
}

/// AB34: a personal block is not the admin ban.
#[sqlx::test(migrations = "./migrations")]
async fn ab34_blocked_person_keeps_using_the_app(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, _a) = register(&app, "a@x.co", "Alice").await;
    let (tb, b) = register(&app, "b@x.co", "Bob").await;
    let (tc, c) = register(&app, "c@x.co", "Carol").await;
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    let (s, _) = send(&app, build("GET", "/api/me", Some(&tb), None)).await;
    assert_eq!(s, StatusCode::OK, "AB34 /api/me");
    assert_eq!(dm(&app, &tb, c, "hi").await.0, StatusCode::CREATED, "AB34: B messages C");
    assert_eq!(profile(&app, Some(&tc), b).await.0, StatusCode::OK, "AB34: C sees B");
    let banned: bool = sqlx::query_scalar("SELECT blocked FROM users WHERE id = $1")
        .bind(b)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!banned, "AB34: users.blocked untouched");
}

// ---------------------------------------------------------------------------------------------
// 2. Messages (S2)
// ---------------------------------------------------------------------------------------------

/// AB38, AB39, AB52, AB53, AB80–AB83, AB113: every refusal is the same neutral 403.
#[sqlx::test(migrations = "./migrations")]
async fn ab38_ab39_ab52_ab53_ab80_ab83_ab113_refusals_both_ways(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alicename").await;
    let (tb, b) = register(&app, "b@x.co", "Bobname").await;
    let pa = create_post(&app, &ta, "A's post").await;
    let pb = create_post(&app, &tb, "B's post").await;
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);

    let mut bodies: Vec<(&str, Value)> = Vec::new();
    let (s, v) = dm(&app, &tb, a, "hi").await;
    assert_eq!(s, StatusCode::FORBIDDEN, "AB38");
    bodies.push(("AB38", v));
    let (s, v) = dm(&app, &ta, b, "hi").await;
    assert_eq!(s, StatusCode::FORBIDDEN, "AB39");
    bodies.push(("AB39", v));
    assert_eq!(msgs(&pool, b, a).await + msgs(&pool, a, b).await, 0, "AB38/AB39: no messages row");

    let (s, v) = send(&app, build("POST", &format!("/api/users/{a}/follow"), Some(&tb), None)).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "AB52");
    bodies.push(("AB52", v));
    let upper = a.to_string().to_uppercase();
    let (s, _) = send(&app, build("POST", &format!("/api/users/{upper}/follow"), Some(&tb), None)).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "AB52 upper-case id");
    let (s, v) = send(&app, build("POST", &format!("/api/users/{b}/follow"), Some(&ta), None)).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "AB53");
    bodies.push(("AB53", v));
    assert_eq!(follows_between(&pool, a, b).await, 0, "AB52/AB53: no follows row");

    let (s, v) = like(&app, &tb, pa).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "AB80");
    bodies.push(("AB80", v));
    let (s, v) = like(&app, &ta, pb).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "AB81");
    bodies.push(("AB81", v));
    let likes: i64 = sqlx::query_scalar("SELECT count(*) FROM post_likes").fetch_one(&pool).await.unwrap();
    assert_eq!(likes, 0, "AB80/AB81: no post_likes row");

    let (s, v) = comment(&app, &tb, pa, "x").await;
    assert_eq!(s, StatusCode::FORBIDDEN, "AB82");
    bodies.push(("AB82", v));
    let (s, v) = comment(&app, &ta, pb, "x").await;
    assert_eq!(s, StatusCode::FORBIDDEN, "AB83");
    bodies.push(("AB83", v));
    let comments: i64 = sqlx::query_scalar("SELECT count(*) FROM post_comments").fetch_one(&pool).await.unwrap();
    assert_eq!(comments, 0, "AB82/AB83: no post_comments row");

    assert_eq!(alerts_from(&pool, a, b).await, 0, "AB52/AB80/AB82: no alert for A from B");
    assert_eq!(alerts_from(&pool, b, a).await, 0, "no alert for B from A either");

    // AB113 / D6: one body for all, {"error": <same string>}, naming nobody.
    let first = bodies[0].1.clone();
    assert!(first["error"].is_string(), "AB113: {{\"error\": ...}} body, got {first}");
    assert_eq!(first.as_object().unwrap().len(), 1, "AB113: only the error key");
    for (row, body) in &bodies {
        assert_eq!(body, &first, "AB113: {row} body differs");
        let text = body.to_string().to_lowercase();
        assert!(!text.contains(&a.to_string()) && !text.contains(&b.to_string()), "AB113: no id in {row}");
        assert!(!text.contains("alicename") && !text.contains("bobname"), "AB113: no name in {row}");
        assert!(!text.contains("block") && !text.contains("заблок"), "AB113: no 'blocked' phrase in {row}");
    }
}

/// AB41, AB42: history kept on both sides.
#[sqlx::test(migrations = "./migrations")]
async fn ab41_ab42_history_kept(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (tb, b) = register(&app, "b@x.co", "Bob").await;
    assert_eq!(dm(&app, &ta, b, "1").await.0, StatusCode::CREATED);
    assert_eq!(dm(&app, &tb, a, "2").await.0, StatusCode::CREATED);
    assert_eq!(dm(&app, &ta, b, "3").await.0, StatusCode::CREATED);
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    let t = get_json(&app, Some(&ta), &format!("/api/messages/{b}")).await;
    assert_eq!(t.as_array().unwrap().len(), 3, "AB41");
    let t = get_json(&app, Some(&tb), &format!("/api/messages/{a}")).await;
    assert_eq!(t.as_array().unwrap().len(), 3, "AB42");
}

/// AB43, AB44, AB45, AB46, AB47.
#[sqlx::test(migrations = "./migrations")]
async fn ab43_ab47_conversations_and_unread_hide_only_for_blocker(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (tb, b) = register(&app, "b@x.co", "Bob").await;
    let (tc, c) = register(&app, "c@x.co", "Carol").await;
    assert_eq!(dm(&app, &tb, a, "b1").await.0, StatusCode::CREATED);
    assert_eq!(dm(&app, &tb, a, "b2").await.0, StatusCode::CREATED);
    assert_eq!(dm(&app, &tc, a, "c1").await.0, StatusCode::CREATED);
    assert_eq!(dm(&app, &ta, b, "a1").await.0, StatusCode::CREATED);
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);

    let conv = get_json(&app, Some(&ta), "/api/me/conversations").await;
    let peers = ids_in(conv.as_array().unwrap(), "peer_id");
    assert!(!peers.contains(&b.to_string()), "AB43: B hidden");
    assert!(peers.contains(&c.to_string()), "AB43: C present");
    let conv = get_json(&app, Some(&tb), "/api/me/conversations").await;
    assert!(ids_in(conv.as_array().unwrap(), "peer_id").contains(&a.to_string()), "AB44");
    assert_eq!(unread(&app, &ta).await.1, 1, "AB45");
    assert_eq!(unread(&app, &tb).await.1, 1, "AB46");

    assert_eq!(unblock(&app, &ta, b).await, StatusCode::NO_CONTENT);
    let conv = get_json(&app, Some(&ta), "/api/me/conversations").await;
    assert!(ids_in(conv.as_array().unwrap(), "peer_id").contains(&b.to_string()), "AB47: B back");
    assert_eq!(unread(&app, &ta).await.1, 3, "AB47");
}

/// AB48: mutual block; each row is its own.
#[sqlx::test(migrations = "./migrations")]
async fn ab48_mutual_block(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (tb, b) = register(&app, "b@x.co", "Bob").await;
    assert_eq!(dm(&app, &ta, b, "hello").await.0, StatusCode::CREATED);
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    assert_eq!(block(&app, &tb, a).await, StatusCode::NO_CONTENT);
    assert_eq!(unblock(&app, &ta, b).await, StatusCode::NO_CONTENT);
    assert_eq!(block_row(&pool, b, a).await, 1, "AB48: B's row stands");
    assert_eq!(dm(&app, &ta, b, "x").await.0, StatusCode::FORBIDDEN, "AB48 A→B");
    assert_eq!(dm(&app, &tb, a, "x").await.0, StatusCode::FORBIDDEN, "AB48 B→A");
    let conv = get_json(&app, Some(&ta), "/api/me/conversations").await;
    assert!(ids_in(conv.as_array().unwrap(), "peer_id").contains(&b.to_string()), "AB48: A sees B");
    let conv = get_json(&app, Some(&tb), "/api/me/conversations").await;
    assert!(!ids_in(conv.as_array().unwrap(), "peer_id").contains(&a.to_string()), "AB48: B hides A");
}

/// AB49 (D11): sends racing a block never commit after it.
#[sqlx::test(migrations = "./migrations")]
async fn ab49_sends_racing_the_block(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (tb, b) = register(&app, "b@x.co", "Bob").await;
    for round in 0..3 {
        clear_rate(&pool).await;
        let mut handles = Vec::new();
        for i in 0..20 {
            let (app, tb) = (app.clone(), tb.clone());
            handles.push(tokio::spawn(async move { dm(&app, &tb, a, &format!("m{i}")).await.0 }));
        }
        let (app2, ta2, pool2) = (app.clone(), ta.clone(), pool.clone());
        let blocker = tokio::spawn(async move {
            let s = block(&app2, &ta2, b).await;
            let right_after = msgs(&pool2, b, a).await;
            (s, right_after)
        });
        let mut statuses = Vec::new();
        for h in handles {
            statuses.push(h.await.unwrap());
        }
        let (s, right_after) = blocker.await.unwrap();
        assert_eq!(s, StatusCode::NO_CONTENT, "round {round}");
        for st in &statuses {
            assert!(
                *st == StatusCode::CREATED || *st == StatusCode::FORBIDDEN,
                "AB49 round {round}: unexpected {st}"
            );
        }
        let after_all = msgs(&pool, b, a).await;
        assert_eq!(right_after, after_all, "AB49 round {round}: a send committed after the block");
        assert_eq!(dm(&app, &tb, a, "late").await.0, StatusCode::FORBIDDEN, "AB49 round {round}: after");
        sqlx::query("DELETE FROM user_blocks").execute(&pool).await.unwrap();
    }
}

// ---------------------------------------------------------------------------------------------
// 3. Follows (S3)
// ---------------------------------------------------------------------------------------------

/// AB51, AB54, AB55.
#[sqlx::test(migrations = "./migrations")]
async fn ab51_ab54_ab55_follows_removed_lists_match_counts(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (tb, b) = register(&app, "b@x.co", "Bob").await;
    let (tc, c) = register(&app, "c@x.co", "Carol").await;
    assert_eq!(follow(&app, &ta, b).await, StatusCode::NO_CONTENT);
    assert_eq!(follow(&app, &tb, a).await, StatusCode::NO_CONTENT);
    assert_eq!(follow(&app, &tc, a).await, StatusCode::NO_CONTENT);
    assert_eq!(follow(&app, &tc, b).await, StatusCode::NO_CONTENT);
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    assert_eq!(follows_between(&pool, a, b).await, 0, "AB51");

    for (me, other) in [(a, b), (b, a)] {
        let followers = get_json(&app, None, &format!("/api/users/{me}/followers")).await;
        let following = get_json(&app, None, &format!("/api/users/{me}/following")).await;
        let (followers, following) = (followers.as_array().unwrap(), following.as_array().unwrap());
        assert!(!ids_in(followers, "id").contains(&other.to_string()), "AB51 followers of {me}");
        assert!(!ids_in(following, "id").contains(&other.to_string()), "AB51 following of {me}");
        let p = profile(&app, None, me).await.1;
        assert_eq!(p["followers_count"].as_i64().unwrap(), followers.len() as i64, "AB51 count ↔ list");
        assert_eq!(p["following_count"].as_i64().unwrap(), following.len() as i64, "AB51 count ↔ list");
    }
    let _ = c;

    // AB54
    let (s, _) = send(&app, build("DELETE", &format!("/api/users/{a}/follow"), Some(&tb), None)).await;
    assert_eq!(s, StatusCode::NO_CONTENT, "AB54");
    // AB55
    assert_eq!(unblock(&app, &ta, b).await, StatusCode::NO_CONTENT);
    assert_eq!(follows_between(&pool, a, b).await, 0, "AB55");
}

/// AB56: a follow re-created between two blocks.
#[sqlx::test(migrations = "./migrations")]
async fn ab56_follow_between_blocks(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (tb, b) = register(&app, "b@x.co", "Bob").await;
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    assert_eq!(unblock(&app, &ta, b).await, StatusCode::NO_CONTENT);
    assert_eq!(follow(&app, &tb, a).await, StatusCode::NO_CONTENT);
    let list = notif_list(&app, &ta).await;
    assert!(
        list.iter().any(|n| n["kind"] == "new_follower" && n["actor_id"] == json!(b)),
        "precondition: A got new_follower"
    );
    assert_eq!(unread(&app, &ta).await.0, 1);
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    assert_eq!(follows_between(&pool, a, b).await, 0, "AB56: follow gone again");
    let list = notif_list(&app, &ta).await;
    assert!(!list.iter().any(|n| n["actor_id"] == json!(b)), "AB56: alert hidden");
    assert_eq!(unread(&app, &ta).await.0, 0, "AB56: not counted");
}

/// AB58: friends lists drop each other; counts unchanged.
#[sqlx::test(migrations = "./migrations")]
async fn ab58_event_friends_lists(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (tb, b) = register(&app, "b@x.co", "Bob").await;
    let (tc, _c) = register(&app, "c@x.co", "Carol").await;
    let e = free_event(&app, &tc, "Gig", "Poetry", None).await;
    assert_eq!(follow(&app, &ta, b).await, StatusCode::NO_CONTENT);
    assert_eq!(follow(&app, &tb, a).await, StatusCode::NO_CONTENT);
    for t in [&ta, &tb] {
        assert_eq!(reserve(&app, t, e, 1).await.0, StatusCode::CREATED);
        let (s, _) = send(&app, build("POST", &format!("/api/events/{e}/save"), Some(t), None)).await;
        assert!(s.is_success(), "save: {s}");
    }
    let before = get_json(&app, Some(&ta), &format!("/api/events/{e}/social")).await;
    assert!(
        ids_in(before["friends_going"].as_array().unwrap(), "id").contains(&b.to_string()),
        "precondition: B is A's friend going"
    );
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    for (tok, other) in [(&ta, b), (&tb, a)] {
        let v = get_json(&app, Some(tok), &format!("/api/events/{e}/social")).await;
        assert!(!ids_in(v["friends_going"].as_array().unwrap(), "id").contains(&other.to_string()), "AB58 going");
        assert!(
            !ids_in(v["friends_interested"].as_array().unwrap(), "id").contains(&other.to_string()),
            "AB58 interested"
        );
        assert_eq!(v["going_count"], before["going_count"], "AB58 going_count");
        assert_eq!(v["interested_count"], before["interested_count"], "AB58 interested_count");
    }
}

/// AB59, AB75 (D7): no organizer_event / event_match alert for A from B's event.
#[sqlx::test(migrations = "./migrations")]
async fn ab59_ab75_no_event_alerts_from_blocked_organiser(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (tb, b) = register(&app, "b@x.co", "Bob").await;
    let (tc, c) = register(&app, "c@x.co", "Carol").await;
    let (ta2, a2) = register(&app, "a2@x.co", "Ann").await;
    for t in [&ta, &tc, &ta2] {
        put_prefs(&app, t, json!(["Rock"]), json!("Kyiv")).await;
    }
    // AB59: A followed B before blocking. AB75: A2 never followed B.
    assert_eq!(follow(&app, &ta, b).await, StatusCode::NO_CONTENT);
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    assert_eq!(block(&app, &ta2, b).await, StatusCode::NO_CONTENT);
    let e = free_event(&app, &tb, "Rock night", "Rock", Some("Kyiv")).await;
    for (who, row) in [(a, "AB59"), (a2, "AB75")] {
        let k: i64 = sqlx::query_scalar("SELECT count(*) FROM notifications WHERE user_id = $1 AND event_id = $2")
            .bind(who)
            .bind(e)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(k, 0, "{row}: no event alert for the blocker");
    }
    let k: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM notifications WHERE user_id = $1 AND event_id = $2 AND kind = 'event_match'",
    )
    .bind(c)
    .bind(e)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(k, 1, "AB59/AB75: C with the same prefs gets event_match");
}

// ---------------------------------------------------------------------------------------------
// 4. Alerts and push (S4)
// ---------------------------------------------------------------------------------------------

/// AB60, AB73.
#[sqlx::test(migrations = "./migrations")]
async fn ab60_ab73_held_alerts_hidden_then_back(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, _a) = register(&app, "a@x.co", "Alice").await;
    let (tb, b) = register(&app, "b@x.co", "Bob").await;
    let (tc, c) = register(&app, "c@x.co", "Carol").await;
    let pa = create_post(&app, &ta, "hi").await;
    assert_eq!(follow(&app, &tb, _a).await, StatusCode::NO_CONTENT);
    assert_eq!(like(&app, &tb, pa).await.0, StatusCode::NO_CONTENT);
    assert_eq!(comment(&app, &tb, pa, "nice").await.0, StatusCode::CREATED);
    assert_eq!(like(&app, &tc, pa).await.0, StatusCode::NO_CONTENT);
    assert_eq!(notif_list(&app, &ta).await.len(), 4, "precondition");
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);

    let list = notif_list(&app, &ta).await;
    assert_eq!(list.len(), 1, "AB60: only C's alert");
    assert_eq!(list[0]["actor_id"], json!(c));
    assert_eq!(unread(&app, &ta).await.0, 1, "AB60: count = C's unread only");

    assert_eq!(unblock(&app, &ta, b).await, StatusCode::NO_CONTENT);
    let list = notif_list(&app, &ta).await;
    assert_eq!(list.iter().filter(|n| n["actor_id"] == json!(b)).count(), 3, "AB73: B's 3 back");
    assert_eq!(unread(&app, &ta).await.0, 4, "AB73: counted again");
}

/// AB61: the open-offer branch hides the blocked person's offer too.
#[sqlx::test(migrations = "./migrations")]
async fn ab61_open_offer_branch_hidden(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (tb, b) = register(&app, "b@x.co", "Bob").await;
    let (tc, c) = register(&app, "c@x.co", "Carol").await;
    let e = free_event(&app, &tc, "Gig", "Poetry", None).await;
    assert_eq!(reserve(&app, &tb, e, 1).await.0, StatusCode::CREATED);
    let tid = tickets_of(&pool, b, e).await[0];
    let (s, _) = send(
        &app,
        build("POST", &format!("/api/tickets/{tid}/gift"), Some(&tb), Some(json!({"email": "a@x.co"}))),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    sqlx::query(
        "INSERT INTO notifications (user_id, kind, title, actor_id, created_at, pushed_at) \
         SELECT $1, 'post_like', 'C liked', $2, now() + g * interval '1 second', now() FROM generate_series(1, 101) g",
    )
    .bind(a)
    .bind(c)
    .execute(&pool)
    .await
    .unwrap();
    let list = notif_list(&app, &ta).await;
    assert!(
        list.iter().any(|n| n["actor_id"] == json!(b) && n["kind"] == "place_offer"),
        "precondition: the open offer is listed beyond the newest 100"
    );
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    let list = notif_list(&app, &ta).await;
    assert!(!list.iter().any(|n| n["actor_id"] == json!(b)), "AB61: B's offer alert absent");
    assert!(!list.iter().any(|n| n["kind"] == "place_offer"), "AB61");
}

/// AB62: hidden alerts do not use up the limit of 100.
#[sqlx::test(migrations = "./migrations")]
async fn ab62_hidden_alerts_do_not_consume_limit(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (_, b) = register(&app, "b@x.co", "Bob").await;
    let (_, c) = register(&app, "c@x.co", "Carol").await;
    sqlx::query(
        "INSERT INTO notifications (user_id, kind, title, actor_id, created_at) \
         SELECT $1, 'post_like', 'C', $2, now() - interval '1 day' + g * interval '1 second' \
         FROM generate_series(1, 50) g",
    )
    .bind(a)
    .bind(c)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO notifications (user_id, kind, title, actor_id, created_at) \
         SELECT $1, 'post_like', 'B', $2, now() + g * interval '1 second' FROM generate_series(1, 100) g",
    )
    .bind(a)
    .bind(b)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    let list = notif_list(&app, &ta).await;
    assert_eq!(list.len(), 50, "AB62: C's 50");
    assert!(list.iter().all(|n| n["actor_id"] == json!(c)), "AB62");
}

/// AB63, D9: no booking alert from the blocked booker; others' bookings carry the actor.
#[sqlx::test(migrations = "./migrations")]
async fn ab63_booking_alert_suppressed(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (tb, b) = register(&app, "b@x.co", "Bob").await;
    let (tc, c) = register(&app, "c@x.co", "Carol").await;
    let ea = free_event(&app, &ta, "A's gig", "Poetry", None).await;
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    let (s, v) = reserve(&app, &tb, ea, 2).await;
    assert_eq!(s, StatusCode::CREATED, "AB63/S7: B still books, {v}");
    assert_eq!(tickets_of(&pool, b, ea).await.len(), 2, "AB63: 2 places");
    let k: i64 = sqlx::query_scalar("SELECT count(*) FROM notifications WHERE user_id = $1 AND kind = 'booking'")
        .bind(a)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(k, 0, "AB63: no booking alert for A");
    assert_eq!(reserve(&app, &tc, ea, 1).await.0, StatusCode::CREATED);
    let actor: Option<Uuid> =
        sqlx::query_scalar("SELECT actor_id FROM notifications WHERE user_id = $1 AND kind = 'booking'")
            .bind(a)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(actor, Some(c), "D9: booking alert stores the booker");
}

/// AB65, AB66: an offer to the blocker's email looks exactly like one to an unknown email.
#[sqlx::test(migrations = "./migrations")]
async fn ab65_ab66_offers_to_blocker_look_like_unknown_email(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (tb, b) = register(&app, "b@x.co", "Bob").await;
    let (tc, _) = register(&app, "c@x.co", "Carol").await;
    let e = free_event(&app, &tc, "Gig", "Poetry", None).await;
    assert_eq!(reserve(&app, &tb, e, 4).await.0, StatusCode::CREATED);
    let t = tickets_of(&pool, b, e).await;
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);

    // AB65
    let (s1, raw1) = send_raw(
        &app,
        build("POST", &format!("/api/tickets/{}/transfer", t[0]), Some(&tb), Some(json!({"email": "a@x.co"}))),
    )
    .await;
    let (s2, raw2) = send_raw(
        &app,
        build("POST", &format!("/api/tickets/{}/transfer", t[1]), Some(&tb), Some(json!({"email": "nobody@x.co"}))),
    )
    .await;
    assert_eq!(s1, StatusCode::ACCEPTED, "AB65");
    assert_eq!(s2, StatusCode::ACCEPTED);
    assert_eq!(raw1, raw2, "AB65: byte-identical to an unknown email");
    assert_eq!(serde_json::from_slice::<Value>(&raw1).unwrap(), json!({"status": "sent"}));
    let pending: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ticket_gifts WHERE ticket_id = $1 AND status = 'pending' AND lower(to_email) = 'a@x.co'",
    )
    .bind(t[0])
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pending, 1, "AB65: pending offer row");

    // AB66
    let (g1, v1) = send(
        &app,
        build("POST", &format!("/api/tickets/{}/gift", t[2]), Some(&tb), Some(json!({"email": "a@x.co"}))),
    )
    .await;
    let (g2, v2) = send(
        &app,
        build("POST", &format!("/api/tickets/{}/gift", t[3]), Some(&tb), Some(json!({"email": "other@x.co"}))),
    )
    .await;
    assert_eq!(g1, StatusCode::CREATED, "AB66");
    assert_eq!(g2, StatusCode::CREATED);
    let keys = |v: &Value| v.as_object().unwrap().keys().cloned().collect::<Vec<_>>();
    assert_eq!(keys(&v1), keys(&v2), "AB66: same fields");

    assert_eq!(alerts_from(&pool, a, b).await, 0, "AB65/AB66: no alert for A");
    assert!(notif_list(&app, &ta).await.is_empty(), "AB65/AB66: A's list empty");
}

/// AB57/AB97 window: like and follow commit BEFORE `notify_actor` runs, so a block can land in between.
/// The alert written in that window must not exist for A (S4). Modelled deterministically by calling the
/// alert writer after the block has committed; C's alert is the control.
#[sqlx::test(migrations = "./migrations")]
async fn ab57_ab97_social_alert_after_block_is_not_written(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (_, b) = register(&app, "b@x.co", "Bob").await;
    let (_, c) = register(&app, "c@x.co", "Carol").await;
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    for (kind, actor) in [("post_like", b), ("new_follower", b), ("post_comment", b), ("post_like", c)] {
        underground_api::notifications::notify_actor(&pool, a, actor, kind, ("x", "x"), None, None)
            .await
            .unwrap();
    }
    assert_eq!(alerts_from(&pool, a, b).await, 0, "AB57/AB97: no alert row from B after the block");
    assert_eq!(alerts_from(&pool, a, c).await, 1, "control: C's alert written");
}

/// AB71: an alert queued for push before the block is never pushed, not even after an unblock.
#[sqlx::test(migrations = "./migrations")]
async fn ab71_queued_push_dropped(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (tb, b) = register(&app, "b@x.co", "Bob").await;
    let (tc, c) = register(&app, "c@x.co", "Carol").await;
    sqlx::query("INSERT INTO device_tokens (token, user_id) VALUES ('dev-a', $1)")
        .bind(a)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(follow(&app, &tb, a).await, StatusCode::NO_CONTENT);
    assert_eq!(follow(&app, &tc, a).await, StatusCode::NO_CONTENT);
    let from_b: Uuid = sqlx::query_scalar("SELECT id FROM notifications WHERE user_id = $1 AND actor_id = $2")
        .bind(a)
        .bind(b)
        .fetch_one(&pool)
        .await
        .unwrap();
    let from_c: Uuid = sqlx::query_scalar("SELECT id FROM notifications WHERE user_id = $1 AND actor_id = $2")
        .bind(a)
        .bind(c)
        .fetch_one(&pool)
        .await
        .unwrap();
    let ids = |v: Vec<underground_api::push::PendingPush>| v.into_iter().map(|p| p.notification_id).collect::<Vec<_>>();
    let before = ids(underground_api::push::pending(&pool, 1000).await.unwrap());
    assert!(before.contains(&from_b) && before.contains(&from_c), "precondition: both queued");

    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    let now = ids(underground_api::push::pending(&pool, 1000).await.unwrap());
    assert!(!now.contains(&from_b), "AB71: B's queued alert not selected");
    assert!(now.contains(&from_c), "AB71: C's still selected");
    assert_eq!(unblock(&app, &ta, b).await, StatusCode::NO_CONTENT);
    let later = ids(underground_api::push::pending(&pool, 1000).await.unwrap());
    assert!(!later.contains(&from_b), "AB71: not pushed after the unblock either");
}

/// AB72: the push selection filters at send time.
#[sqlx::test(migrations = "./migrations")]
async fn ab72_push_filter_at_send_time(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (_, b) = register(&app, "b@x.co", "Bob").await;
    let (_, c) = register(&app, "c@x.co", "Carol").await;
    sqlx::query("INSERT INTO device_tokens (token, user_id) VALUES ('dev-a', $1)")
        .bind(a)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    let insert = |actor: Uuid| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, Uuid>(
                "INSERT INTO notifications (user_id, kind, title, actor_id) VALUES ($1, 'post_like', 't', $2) RETURNING id",
            )
            .bind(a)
            .bind(actor)
            .fetch_one(&pool)
            .await
            .unwrap()
        }
    };
    let nb = insert(b).await;
    let nc = insert(c).await;
    let got: Vec<Uuid> = underground_api::push::pending(&pool, 1000)
        .await
        .unwrap()
        .into_iter()
        .map(|p| p.notification_id)
        .collect();
    assert!(!got.contains(&nb), "AB72: alert from a blocked actor not selected");
    assert!(got.contains(&nc), "AB72: control alert selected");
}

/// AB77: S4 protects the blocker only; A's own acts reach B.
#[sqlx::test(migrations = "./migrations")]
async fn ab77_blockers_own_acts_still_alert_b(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (tb, b) = register(&app, "b@x.co", "Bob").await;
    let (tc, _) = register(&app, "c@x.co", "Carol").await;
    let ec = free_event(&app, &tc, "C's gig", "Poetry", None).await;
    let eb = free_event(&app, &tb, "B's gig", "Poetry", None).await;
    assert_eq!(reserve(&app, &ta, ec, 1).await.0, StatusCode::CREATED);
    let t_a = tickets_of(&pool, a, ec).await[0];
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    let (s, _) = send(
        &app,
        build("POST", &format!("/api/tickets/{t_a}/transfer"), Some(&ta), Some(json!({"email": "b@x.co"}))),
    )
    .await;
    assert_eq!(s, StatusCode::ACCEPTED);
    assert_eq!(reserve(&app, &ta, eb, 1).await.0, StatusCode::CREATED, "AB77: A books E_B");
    let list = notif_list(&app, &tb).await;
    assert!(list.iter().any(|n| n["kind"] == "place_offer"), "AB77: B gets the place offer");
    assert!(list.iter().any(|n| n["kind"] == "booking"), "AB77: B gets the booking alert");
}

// ---------------------------------------------------------------------------------------------
// 5. Posts, likes and comments (S5)
// ---------------------------------------------------------------------------------------------

/// AB85, AB86, AB87, AB88, AB89, AB90.
#[sqlx::test(migrations = "./migrations")]
async fn ab85_ab90_comment_lists_hide_per_viewer(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (tb, b) = register(&app, "b@x.co", "Bob").await;
    let (tc, c) = register(&app, "c@x.co", "Carol").await;
    let pa = create_post(&app, &ta, "A's").await;
    let pc = create_post(&app, &tc, "C's").await;
    assert_eq!(like(&app, &tb, pa).await.0, StatusCode::NO_CONTENT);
    for p in [pa, pc] {
        assert_eq!(comment(&app, &tb, p, "from B").await.0, StatusCode::CREATED);
        assert_eq!(comment(&app, &tc, p, "from C").await.0, StatusCode::CREATED);
        assert_eq!(comment(&app, &ta, p, "from A").await.0, StatusCode::CREATED);
    }
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    let has = |list: &[Value], who: Uuid| list.iter().any(|c| c["author_id"] == json!(who));

    // AB86: the block deletes nothing.
    for tok in [Some(tc.as_str()), None] {
        assert!(has(&comments_of(&app, tok, pa).await, b), "AB86: B's comment listed for C/anon");
    }
    let posts = get_json(&app, Some(&tc), &format!("/api/users/{a}/posts")).await;
    let pv = posts.as_array().unwrap().iter().find(|p| p["id"] == json!(pa)).unwrap().clone();
    assert_eq!(pv["likes_count"], json!(1), "AB86: B's like counted");
    // AB87
    let l = comments_of(&app, Some(&ta), pa).await;
    assert!(!has(&l, b) && has(&l, c), "AB87");
    // AB88
    let l = comments_of(&app, Some(&ta), pc).await;
    assert!(!has(&l, b) && has(&l, c), "AB88");
    // AB89
    assert!(has(&comments_of(&app, None, pc).await, b), "AB89");
    // AB90
    assert!(has(&comments_of(&app, Some(&tb), pa).await, a), "AB90 P_A");
    assert!(has(&comments_of(&app, Some(&tb), pc).await, a), "AB90 P_C");
    // AB85
    let (s, _) = send(&app, build("DELETE", &format!("/api/posts/{pa}/like"), Some(&tb), None)).await;
    assert_eq!(s, StatusCode::NO_CONTENT, "AB85");
    let k = n(&pool, "SELECT count(*) FROM post_likes WHERE post_id = $1 AND user_id = $2", pa, b).await;
    assert_eq!(k, 0, "AB85: like removed");
}

/// AB91: hidden comments do not use up LIMIT 200.
#[sqlx::test(migrations = "./migrations")]
async fn ab91_hidden_comments_do_not_consume_limit(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, _a) = register(&app, "a@x.co", "Alice").await;
    let (_, b) = register(&app, "b@x.co", "Bob").await;
    let (tc, c) = register(&app, "c@x.co", "Carol").await;
    let pc = create_post(&app, &tc, "C's").await;
    sqlx::query(
        "INSERT INTO post_comments (post_id, author_id, text, created_at) \
         SELECT $1, $2, 'b' || g, now() - interval '1 day' + g * interval '1 second' FROM generate_series(1, 200) g",
    )
    .bind(pc)
    .bind(b)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO post_comments (post_id, author_id, text, created_at) \
         SELECT $1, $2, 'c' || g, now() + g * interval '1 second' FROM generate_series(1, 10) g",
    )
    .bind(pc)
    .bind(c)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    let l = comments_of(&app, Some(&ta), pc).await;
    assert_eq!(l.len(), 10, "AB91: C's 10");
    assert!(l.iter().all(|x| x["author_id"] == json!(c)));
}

/// AB92 (D10): comments_count follows the viewer's list.
#[sqlx::test(migrations = "./migrations")]
async fn ab92_comments_count_per_viewer(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, _a) = register(&app, "a@x.co", "Alice").await;
    let (tb, b) = register(&app, "b@x.co", "Bob").await;
    let (tc, c) = register(&app, "c@x.co", "Carol").await;
    let pc = create_post(&app, &tc, "C's").await;
    for _ in 0..3 {
        assert_eq!(comment(&app, &tb, pc, "b").await.0, StatusCode::CREATED);
    }
    for _ in 0..2 {
        assert_eq!(comment(&app, &tc, pc, "c").await.0, StatusCode::CREATED);
    }
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    let count_for = |tok: Option<String>| {
        let app = app.clone();
        async move {
            let v = get_json(&app, tok.as_deref(), &format!("/api/users/{c}/posts")).await;
            v.as_array().unwrap().iter().find(|p| p["id"] == json!(pc)).unwrap()["comments_count"].clone()
        }
    };
    assert_eq!(count_for(Some(ta.clone())).await, json!(2), "AB92: A sees 2");
    assert_eq!(count_for(Some(tc.clone())).await, json!(5), "AB92: C sees 5");
    assert_eq!(count_for(None).await, json!(5), "AB92: anonymous sees 5");
    assert_eq!(comments_of(&app, Some(&ta), pc).await.len(), 2, "AB92: list ↔ count");
}

/// AB93, AB96.
#[sqlx::test(migrations = "./migrations")]
async fn ab93_ab96_feed_hides_blocked_author(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (tb, b) = register(&app, "b@x.co", "Bob").await;
    let (tc, c) = register(&app, "c@x.co", "Carol").await;
    put_prefs(&app, &ta, json!(["Jazz"]), json!(null)).await;
    let _eb = free_event(&app, &tb, "B jazz", "Jazz", None).await;
    let ea = free_event(&app, &ta, "A jazz", "Jazz", None).await;
    let _ec = free_event(&app, &tc, "C jazz", "Jazz", None).await;
    sqlx::query("INSERT INTO posts (author_id, kind, text, event_id, rating) VALUES ($1, 'review', 'ok', $2, 4)")
        .bind(b)
        .bind(ea)
        .execute(&pool)
        .await
        .unwrap();
    let feed = get_json(&app, Some(&ta), "/api/feed").await;
    let authors = ids_in(feed.as_array().unwrap(), "author_id");
    assert_eq!(authors.iter().filter(|x| **x == b.to_string()).count(), 2, "precondition: both B posts in feed");
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    let feed = get_json(&app, Some(&ta), "/api/feed").await;
    let authors = ids_in(feed.as_array().unwrap(), "author_id");
    assert!(!authors.contains(&b.to_string()), "AB93: B's posts hidden");
    assert!(authors.contains(&c.to_string()), "AB93: C's present");
    // AB96
    let posts = get_json(&app, Some(&ta), &format!("/api/users/{b}/posts")).await;
    assert!(!posts.as_array().unwrap().is_empty(), "AB96: B's wall still readable by A");
    let _ = a;
}

/// AB94: hidden posts do not use up LIMIT 50.
#[sqlx::test(migrations = "./migrations")]
async fn ab94_hidden_posts_do_not_consume_feed_limit(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, _a) = register(&app, "a@x.co", "Alice").await;
    let (tb, b) = register(&app, "b@x.co", "Bob").await;
    let (tc, c) = register(&app, "c@x.co", "Carol").await;
    put_prefs(&app, &ta, json!(["Jazz"]), json!(null)).await;
    let eb = free_event(&app, &tb, "B jazz", "Jazz", None).await;
    let ec = free_event(&app, &tc, "C jazz", "Jazz", None).await;
    // Push the auto posts out of the way, then 10 older by C and 60 newest by B.
    sqlx::query("DELETE FROM posts").execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO posts (author_id, kind, event_id, created_at) \
         SELECT $1, 'event_share', $2, now() - interval '1 day' + g * interval '1 second' FROM generate_series(1, 10) g",
    )
    .bind(c)
    .bind(ec)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO posts (author_id, kind, event_id, created_at) \
         SELECT $1, 'event_share', $2, now() + g * interval '1 second' FROM generate_series(1, 60) g",
    )
    .bind(b)
    .bind(eb)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    let feed = get_json(&app, Some(&ta), "/api/feed").await;
    let feed = feed.as_array().unwrap();
    assert_eq!(feed.len(), 10, "AB94: C's 10");
    assert!(feed.iter().all(|p| p["author_id"] == json!(c)));
}

/// Spec conflict 3 (orchestrator ruling): a post's author may delete any comment under their post.
#[sqlx::test(migrations = "./migrations")]
async fn comment_delete_by_comment_author_or_post_author_only(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, _a) = register(&app, "a@x.co", "Alice").await;
    let (tb, _b) = register(&app, "b@x.co", "Bob").await;
    let (tc, _c) = register(&app, "c@x.co", "Carol").await;
    let pa = create_post(&app, &ta, "A's").await;
    let other = create_post(&app, &tc, "C's").await;
    let cid = |v: Value| -> Uuid { v["id"].as_str().unwrap().parse().unwrap() };
    let b1 = cid(comment(&app, &tb, pa, "b1").await.1);
    let b2 = cid(comment(&app, &tb, pa, "b2").await.1);
    let c1 = cid(comment(&app, &tc, pa, "c1").await.1);
    let exists = |id: Uuid| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM post_comments WHERE id = $1)")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    let del = |tok: String, post: Uuid, id: Uuid| {
        let app = app.clone();
        async move {
            send(&app, build("DELETE", &format!("/api/posts/{post}/comments/{id}"), Some(&tok), None)).await.0
        }
    };
    // C (neither the comment's nor the post's author) cannot delete B's comment.
    assert_eq!(del(tc.clone(), pa, b1).await, StatusCode::NOT_FOUND, "third party → 404");
    assert!(exists(b1).await);
    // Wrong post id → 404, even for the post's author and the comment's author.
    assert_eq!(del(ta.clone(), other, c1).await, StatusCode::NOT_FOUND, "wrong post → 404");
    assert_eq!(del(tb.clone(), other, b2).await, StatusCode::NOT_FOUND, "wrong post → 404 (author)");
    assert!(exists(c1).await && exists(b2).await);
    // Unknown comment id → 404.
    assert_eq!(del(ta.clone(), pa, Uuid::new_v4()).await, StatusCode::NOT_FOUND);
    // The post's author deletes someone else's comment; the comment's author deletes their own.
    assert_eq!(del(ta.clone(), pa, b1).await, StatusCode::NO_CONTENT, "post author → 204");
    assert!(!exists(b1).await);
    assert_eq!(del(tb.clone(), pa, b2).await, StatusCode::NO_CONTENT, "comment author → 204");
    assert!(!exists(b2).await);
    assert_eq!(del(ta.clone(), pa, c1).await, StatusCode::NO_CONTENT);
    assert!(!exists(c1).await);
    // Anonymous → 401.
    let (s, _) = send(&app, build("DELETE", &format!("/api/posts/{pa}/comments/{c1}"), None, None)).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------------------------------------
// 6. Place offers (S6)
// ---------------------------------------------------------------------------------------------

/// AB99: the block cancels nothing; B keeps control of B's ticket.
#[sqlx::test(migrations = "./migrations")]
async fn ab99_pending_offer_survives_and_sender_can_revoke(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, _a) = register(&app, "a@x.co", "Alice").await;
    let (tb, b) = register(&app, "b@x.co", "Bob").await;
    let (tc, _) = register(&app, "c@x.co", "Carol").await;
    let e = free_event(&app, &tc, "Gig", "Poetry", None).await;
    assert_eq!(reserve(&app, &tb, e, 1).await.0, StatusCode::CREATED);
    let tid = tickets_of(&pool, b, e).await[0];
    let (s, _) = send(
        &app,
        build("POST", &format!("/api/tickets/{tid}/transfer"), Some(&tb), Some(json!({"email": "a@x.co"}))),
    )
    .await;
    assert_eq!(s, StatusCode::ACCEPTED);
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    let gift: String = sqlx::query_scalar("SELECT status FROM ticket_gifts WHERE ticket_id = $1")
        .bind(tid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(gift, "pending", "AB99: offer stays pending");
    let st: String = sqlx::query_scalar("SELECT status FROM tickets WHERE id = $1").bind(tid).fetch_one(&pool).await.unwrap();
    assert_eq!(st, "pending_claim", "AB99");
    let (s, _) = send(&app, build("DELETE", &format!("/api/tickets/{tid}/gift"), Some(&tb), None)).await;
    assert_eq!(s, StatusCode::OK, "AB99: B revokes");
    let st: String = sqlx::query_scalar("SELECT status FROM tickets WHERE id = $1").bind(tid).fetch_one(&pool).await.unwrap();
    assert_eq!(st, "active", "AB99");
}

/// AB100: A may accept B's offer through the link.
#[sqlx::test(migrations = "./migrations")]
async fn ab100_blocker_may_claim_by_link(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (tb, b) = register(&app, "b@x.co", "Bob").await;
    let (tc, _) = register(&app, "c@x.co", "Carol").await;
    let e = free_event(&app, &tc, "Gig", "Poetry", None).await;
    assert_eq!(reserve(&app, &tb, e, 1).await.0, StatusCode::CREATED);
    let tid = tickets_of(&pool, b, e).await[0];
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    let (s, v) = send(
        &app,
        build("POST", &format!("/api/tickets/{tid}/gift"), Some(&tb), Some(json!({"email": "a@x.co"}))),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    let token = v["claim_token"].as_str().unwrap().to_string();
    let (s, v) = send(&app, build("POST", "/api/tickets/claim", Some(&ta), Some(json!({"token": token})))).await;
    assert_eq!(s, StatusCode::OK, "AB100: {v}");
    let owner: Uuid = sqlx::query_scalar("SELECT owner_id FROM tickets WHERE id = $1").bind(tid).fetch_one(&pool).await.unwrap();
    assert_eq!(owner, a, "AB100: A owns it");
}

// ---------------------------------------------------------------------------------------------
// 7. Kept on purpose (S7)
// ---------------------------------------------------------------------------------------------

/// AB103: the guest list still shows B.
#[sqlx::test(migrations = "./migrations")]
async fn ab103_guest_list_keeps_blocked_guest(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, _a) = register(&app, "a@x.co", "Alice").await;
    let (tb, b) = register(&app, "b@x.co", "Bob").await;
    let ea = free_event(&app, &ta, "A's", "Poetry", None).await;
    assert_eq!(reserve(&app, &tb, ea, 2).await.0, StatusCode::CREATED);
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    let g = get_json(&app, Some(&ta), &format!("/api/events/{ea}/guests")).await;
    let row = g.as_array().unwrap().iter().find(|x| x["user_id"] == json!(b)).expect("AB103: B listed").clone();
    assert_eq!(row["name"], "Bob", "AB103");
    assert_eq!(row["places"], json!(2), "AB103");
}

/// AB105: B's review of A's event is accepted, listed and counted.
#[sqlx::test(migrations = "./migrations")]
async fn ab105_reviews_neither_refused_nor_hidden(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (tb, b) = register(&app, "b@x.co", "Bob").await;
    let ea = free_event(&app, &ta, "A's", "Poetry", None).await;
    assert_eq!(reserve(&app, &tb, ea, 1).await.0, StatusCode::CREATED);
    sqlx::query("UPDATE events SET starts_at = now() - interval '2 hours' WHERE id = $1")
        .bind(ea)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    let (s, v) = send(&app, build("POST", "/api/reviews", Some(&tb), Some(json!({"event_id": ea, "rating": 2})))).await;
    assert_eq!(s, StatusCode::CREATED, "AB105: {v}");
    for tok in [Some(ta.as_str()), None] {
        let r = get_json(&app, tok, &format!("/api/events/{ea}/reviews")).await;
        assert!(
            r["reviews"].as_array().unwrap().iter().any(|x| x["author_id"] == json!(b)),
            "AB105: listed"
        );
        assert_eq!(r["count"], json!(1));
    }
    let p = profile(&app, Some(&ta), a).await.1;
    assert_eq!(p["rating_count"], json!(1), "AB105: counted in A's rating");
}

/// AB107: profiles, events and walls stay public both ways.
#[sqlx::test(migrations = "./migrations")]
async fn ab107_profiles_stay_public(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (tb, b) = register(&app, "b@x.co", "Bob").await;
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    for path in [format!("/api/users/{a}"), format!("/api/users/{a}/events"), format!("/api/users/{a}/posts")] {
        assert_eq!(send(&app, build("GET", &path, Some(&tb), None)).await.0, StatusCode::OK, "AB107 B {path}");
    }
    for path in [format!("/api/users/{b}"), format!("/api/users/{b}/events")] {
        assert_eq!(send(&app, build("GET", &path, Some(&ta), None)).await.0, StatusCode::OK, "AB107 A {path}");
    }
}

/// AB108: reporting stays open both ways.
#[sqlx::test(migrations = "./migrations")]
async fn ab108_reports_open_both_ways(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (tb, b) = register(&app, "b@x.co", "Bob").await;
    let (tx, _) = register(&app, "x@x.co", "Admin").await;
    sqlx::query("UPDATE users SET role = 'ADMIN' WHERE email = 'x@x.co'").execute(&pool).await.unwrap();
    let pa = create_post(&app, &ta, "A's").await;
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    let report = |tok: String, ty: &'static str, id: Uuid| {
        let app = app.clone();
        async move {
            send(
                &app,
                build("POST", "/api/reports", Some(&tok), Some(json!({"target_type": ty, "target_id": id, "reason": "spam"}))),
            )
            .await
            .0
        }
    };
    assert_eq!(report(tb.clone(), "user", a).await, StatusCode::CREATED, "AB108 B→user A");
    assert_eq!(report(tb.clone(), "post", pa).await, StatusCode::CREATED, "AB108 B→P_A");
    assert_eq!(report(ta.clone(), "user", b).await, StatusCode::CREATED, "AB108 A→user B");
    let q = get_json(&app, Some(&tx), "/api/admin/reports").await;
    let targets = ids_in(q.as_array().unwrap(), "target_id");
    for id in [a, pa, b] {
        assert!(targets.contains(&id.to_string()), "AB108: {id} in the queue");
    }
}

// ---------------------------------------------------------------------------------------------
// 8. Information leaks
// ---------------------------------------------------------------------------------------------

/// AB110, AB111, AB112, AB114 (B has blocked A).
#[sqlx::test(migrations = "./migrations")]
async fn ab110_ab112_ab114_no_field_tells_b(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (tb, b) = register(&app, "b@x.co", "Bob").await;
    let (tc, c) = register(&app, "c@x.co", "Carol").await;
    let (_, d) = register(&app, "d@x.co", "Dave").await;
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    assert_eq!(block(&app, &ta, d).await, StatusCode::NO_CONTENT);

    // AB110
    let (s1, by_b) = profile(&app, Some(&tb), a).await;
    let (s2, by_c) = profile(&app, Some(&tc), a).await;
    assert_eq!((s1, s2), (StatusCode::OK, StatusCode::OK));
    assert_eq!(by_b, by_c, "AB110: B's view of A equals C's");
    assert_eq!(by_b["is_blocked"], json!(false), "AB110");
    // AB111
    for tok in [None, Some("not.a.valid.token")] {
        let (s, p) = profile(&app, tok, b).await;
        assert_eq!(s, StatusCode::OK, "AB111");
        assert_eq!(p["is_blocked"], json!(false), "AB111");
    }
    // AB112
    let list = get_json(&app, Some(&ta), "/api/me/blocks").await;
    let listed = ids_in(list.as_array().unwrap(), "id");
    assert_eq!(listed.len(), 2);
    for id in &listed {
        let (_, p) = profile(&app, Some(&ta), id.parse().unwrap()).await;
        assert_eq!(p["is_blocked"], json!(true), "AB112: listed → true");
    }
    assert_eq!(profile(&app, Some(&ta), c).await.1["is_blocked"], json!(false), "AB112: C false");
    assert_eq!(profile(&app, Some(&tb), a).await.1["is_blocked"], json!(false), "AB112: B→A false");

    // AB114: B blocks A while A blocks B; B's own calls still answer 204 with an empty body.
    for method in ["POST", "DELETE", "POST"] {
        let (s, body) = send_raw(&app, build(method, &format!("/api/users/{b}/block"), Some(&ta), None)).await;
        assert_eq!(s, StatusCode::NO_CONTENT, "AB114 A {method} (B has not blocked A)");
        assert!(body.is_empty());
    }
    assert_eq!(block(&app, &tb, a).await, StatusCode::NO_CONTENT);
    for method in ["POST", "DELETE"] {
        let (s, body) = send_raw(&app, build(method, &format!("/api/users/{b}/block"), Some(&ta), None)).await;
        assert_eq!(s, StatusCode::NO_CONTENT, "AB114 A {method} (B has blocked A)");
        assert!(body.is_empty(), "AB114: empty body");
    }
}

/// AB115 (and AB7): 404 on block exactly when the profile is 404.
#[sqlx::test(migrations = "./migrations")]
async fn ab115_no_existence_oracle(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, _a) = register(&app, "a@x.co", "Alice").await;
    let (_, active) = register(&app, "act@x.co", "Act").await;
    let (_, banned) = register(&app, "ban@x.co", "Ban").await;
    let (te, erased) = register(&app, "era@x.co", "Era").await;
    admin_ban(&pool, banned).await;
    let (s, v) = send(&app, build("DELETE", "/api/me", Some(&te), Some(json!({"password": "password1"})))).await;
    assert_eq!(s, StatusCode::NO_CONTENT, "erase: {v}");
    for (id, label) in [(active, "active"), (banned, "banned"), (erased, "erased (AB7)"), (Uuid::new_v4(), "random")] {
        let profile_404 = profile(&app, Some(&ta), id).await.0 == StatusCode::NOT_FOUND;
        let block_status = block(&app, &ta, id).await;
        assert_eq!(block_status == StatusCode::NOT_FOUND, profile_404, "AB115 {label}: block {block_status}");
        if label != "active" {
            assert_eq!(block_status, StatusCode::NOT_FOUND, "AB115 {label}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 9. Account erasure and export (S8)
// ---------------------------------------------------------------------------------------------

/// AB118, AB123, AB124.
#[sqlx::test(migrations = "./migrations")]
async fn ab118_ab123_ab124_export_lists_blocks_made(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (tb, b) = register(&app, "b@x.co", "Bob").await;
    let (_, c) = register(&app, "c@x.co", "Carol").await;
    let (td, d) = register(&app, "d@x.co", "Dave").await;
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    assert_eq!(block(&app, &ta, c).await, StatusCode::NO_CONTENT);
    assert_eq!(block(&app, &td, a).await, StatusCode::NO_CONTENT);

    // AB123
    let ex = get_json(&app, Some(&ta), "/api/me/export").await;
    let blocks = ex["blocks"].as_array().expect("AB123: top-level blocks array");
    assert_eq!(blocks.len(), 2, "AB123: exactly two");
    for e in blocks {
        let o = e.as_object().unwrap();
        assert!(o.contains_key("user_id") && o.contains_key("blocked_at"), "AB123: id and time");
        assert!(!o.contains_key("email"), "AB123: no email");
    }
    let ids = ids_in(blocks, "user_id");
    assert!(ids.contains(&b.to_string()) && ids.contains(&c.to_string()));
    assert!(!ex["blocks"].to_string().contains(&d.to_string()), "AB123: nothing about D's block");

    // AB118
    let ex_b = get_json(&app, Some(&tb), "/api/me/export").await;
    assert_eq!(ex_b["blocks"], json!([]), "AB118: B's export has no block");
    assert!(!ex_b.to_string().contains(&a.to_string()), "AB118: nothing about A anywhere");

    // AB124
    assert_eq!(unblock(&app, &ta, c).await, StatusCode::NO_CONTENT);
    let ex = get_json(&app, Some(&ta), "/api/me/export").await;
    assert_eq!(ids_in(ex["blocks"].as_array().unwrap(), "user_id"), vec![b.to_string()], "AB124");
}

/// AB121: erasure removes blocks both ways.
#[sqlx::test(migrations = "./migrations")]
async fn ab121_erasure_removes_blocks_both_ways(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (_, b) = register(&app, "b@x.co", "Bob").await;
    let (_, c) = register(&app, "c@x.co", "Carol").await;
    let (td, d) = register(&app, "d@x.co", "Dave").await;
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    assert_eq!(block(&app, &ta, c).await, StatusCode::NO_CONTENT);
    assert_eq!(block(&app, &td, a).await, StatusCode::NO_CONTENT);
    let (s, v) = send(&app, build("DELETE", "/api/me", Some(&ta), Some(json!({"password": "password1"})))).await;
    assert_eq!(s, StatusCode::NO_CONTENT, "{v}");
    let left = n(&pool, "SELECT count(*) FROM user_blocks WHERE blocker_id = $1 OR blocked_id = $2", a, a).await;
    assert_eq!(left, 0, "AB121: no row with A");
    assert_eq!(get_json(&app, Some(&td), "/api/me/blocks").await, json!([]), "AB121: D's list drops A");
    assert_eq!(blocks_of(&pool, d).await, 0, "AB121: D's cap count dropped");
}

/// AB122: the other side's erasure frees the cap.
#[sqlx::test(migrations = "./migrations")]
async fn ab122_erasure_of_blocked_frees_cap(pool: PgPool) {
    let app = make_app(pool.clone());
    let (ta, a) = register(&app, "a@x.co", "Alice").await;
    let (tb, b) = register(&app, "b@x.co", "Bob").await;
    let seeded = seed_users(&pool, 999).await;
    seed_blocks(&pool, a, &seeded).await;
    assert_eq!(block(&app, &ta, b).await, StatusCode::NO_CONTENT);
    assert_eq!(blocks_of(&pool, a).await, 1000);
    let (s, _) = send(&app, build("DELETE", "/api/me", Some(&tb), Some(json!({"password": "password1"})))).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let list = get_json(&app, Some(&ta), "/api/me/blocks").await;
    let list = list.as_array().unwrap();
    assert_eq!(list.len(), 999, "AB122: list drops B");
    assert!(!ids_in(list, "id").contains(&b.to_string()));
    let y = seed_users(&pool, 1).await[0];
    assert_eq!(block(&app, &ta, y).await, StatusCode::NO_CONTENT, "AB122: cap freed");
}

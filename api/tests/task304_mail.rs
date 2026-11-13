//! TASK-304: password reset and login codes delivered through Resend — against a stub Resend on
//! 127.0.0.1 (RESEND_URL), never the real one. Mail is ON in this test binary.
//!
//! The stub is a plain thread with a blocking listener, shared by every test of this binary (each
//! `#[sqlx::test]` has its own Tokio runtime, which would take a Tokio server down with it). Tests
//! tell their mails apart by recipient address.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::http::{HeaderMap, Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use underground_api::routes::router;
use underground_api::state::AppState;

const KEY: &str = "re_test_not_a_real_key";
const FROM: &str = "Underground <no-reply@ssvstudio.dev>";

#[derive(Clone, Debug)]
struct Captured {
    authorization: String,
    idempotency_key: String,
    body: Value,
}

fn captured() -> &'static Mutex<Vec<Captured>> {
    static C: OnceLock<Mutex<Vec<Captured>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(Vec::new()))
}

/// Start the stub once; returns its base URL. Recipients starting with "fail" get HTTP 500.
fn stub() -> &'static str {
    static URL: OnceLock<String> = OnceLock::new();
    URL.get_or_init(|| {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let path_ok = line.starts_with("POST /emails ");
                let (mut len, mut auth, mut idem) = (0usize, String::new(), String::new());
                loop {
                    let mut h = String::new();
                    reader.read_line(&mut h).unwrap();
                    let h = h.trim_end().to_string();
                    if h.is_empty() {
                        break;
                    }
                    let (name, value) = h.split_once(':').unwrap();
                    match name.to_ascii_lowercase().as_str() {
                        "content-length" => len = value.trim().parse().unwrap(),
                        "authorization" => auth = value.trim().to_string(),
                        "idempotency-key" => idem = value.trim().to_string(),
                        _ => {}
                    }
                }
                let mut buf = vec![0; len];
                reader.read_exact(&mut buf).unwrap();
                let body: Value = serde_json::from_slice(&buf).unwrap_or(Value::Null);
                let fail = body["to"][0].as_str().is_some_and(|t| t.starts_with("fail"));
                captured().lock().unwrap().push(Captured { authorization: auth, idempotency_key: idem, body });
                let (status, out) = if !path_ok {
                    ("404 Not Found", "{}")
                } else if fail {
                    ("500 Internal Server Error", "{\"message\":\"boom\"}")
                } else {
                    ("200 OK", "{\"id\":\"stub-id\"}")
                };
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}",
                    out.len()
                );
            }
        });
        url
    })
}

fn make_app(pool: PgPool) -> Router {
    std::env::set_var("RESEND_URL", stub());
    std::env::set_var("RESEND_API_KEY", KEY);
    std::env::set_var("EMAIL_FROM", FROM);
    std::env::set_var("WEB_APP_URL", "https://web.test");
    router(AppState {
        db: pool,
        jwt_secret: "test-secret".into(),
    })
}

fn build(method: &str, path: &str, ip: &str, body: Option<Value>) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip)
        .body(body.map(|v| Body::from(v.to_string())).unwrap_or_else(Body::empty))
        .unwrap()
}

/// Status, headers without `date`, raw body bytes.
async fn raw(app: &Router, req: Request<Body>) -> (StatusCode, HeaderMap, Vec<u8>) {
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let mut headers = resp.headers().clone();
    headers.remove("date");
    (
        status,
        headers,
        to_bytes(resp.into_body(), usize::MAX).await.unwrap().to_vec(),
    )
}

async fn send(app: &Router, req: Request<Body>) -> (StatusCode, Value) {
    let (st, _, b) = raw(app, req).await;
    (
        st,
        if b.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&b).unwrap()
        },
    )
}

async fn register(app: &Router, email: &str, locale: &str) {
    let (st, _) = send(
        app,
        build(
            "POST",
            "/api/auth/register",
            "198.51.100.1",
            Some(json!({"email": email, "password": "password1", "name": "U", "locale": locale})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
}

/// Wait up to 10 s for `n` mails to `to`.
async fn mails_to(to: &str, n: usize) -> Vec<Captured> {
    for _ in 0..200 {
        let got: Vec<_> = captured()
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.body["to"][0] == to)
            .cloned()
            .collect();
        if got.len() >= n {
            return got;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    captured()
        .lock()
        .unwrap()
        .iter()
        .filter(|c| c.body["to"][0] == to)
        .cloned()
        .collect()
}

fn token_from(mail: &Captured) -> String {
    let text = mail.body["text"].as_str().unwrap();
    let at = text
        .find("https://web.test/reset-password?token=")
        .expect("link in the text part");
    text[at + "https://web.test/reset-password?token=".len()..]
        .chars()
        .take_while(|c| c.is_ascii_hexdigit())
        .collect()
}

async fn forgot(app: &Router, email: &str, ip: &str) -> (StatusCode, HeaderMap, Vec<u8>) {
    raw(
        app,
        build("POST", "/api/auth/forgot-password", ip, Some(json!({"email": email}))),
    )
    .await
}

async fn reset(app: &Router, token: &str, password: &str) -> StatusCode {
    send(
        app,
        build(
            "POST",
            "/api/auth/reset-password",
            "198.51.100.9",
            Some(json!({"token": token, "password": password})),
        ),
    )
    .await
    .0
}

#[sqlx::test(migrations = "./migrations")]
async fn features_report_mail_on(pool: PgPool) {
    let app = make_app(pool);
    let (st, v) = send(&app, build("GET", "/api/features", "198.51.100.1", None)).await;
    assert_eq!((st, v), (StatusCode::OK, json!({"email": true})));
}

#[sqlx::test(migrations = "./migrations")]
async fn reset_link_arrives_works_once_and_signs_out(pool: PgPool) {
    let app = make_app(pool.clone());
    register(&app, "flow@x.co", "uk").await;
    let (st, _, body) = forgot(&app, "Flow@X.co ", "198.51.100.2").await;
    assert_eq!((st, body.len()), (StatusCode::NO_CONTENT, 0));

    let mails = mails_to("flow@x.co", 1).await;
    assert_eq!(mails.len(), 1, "exactly one mail");
    let m = &mails[0];
    assert_eq!(m.authorization, format!("Bearer {KEY}"));
    assert!(!m.idempotency_key.is_empty(), "every mail carries an idempotency key");
    assert_eq!(m.body["from"], FROM);
    assert!(
        m.body["subject"].as_str().unwrap().contains("Скидання"),
        "uk account → Ukrainian mail"
    );
    assert!(
        m.body["html"].as_str().unwrap().contains("reset-password?token="),
        "HTML part too"
    );
    let token = token_from(m);
    assert_eq!(token.len(), 64);

    // Stored only as a hash, valid for 30 minutes.
    let (hash_ok, mins): (bool, f64) = sqlx::query_as(
        "SELECT token_hash = $1, (EXTRACT(EPOCH FROM expires_at - now()) / 60)::float8 FROM password_resets WHERE used_at IS NULL",
    )
    .bind(underground_api::passwords::token_hash(&token))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(hash_ok, "[SEC] the raw token is not what is stored");
    assert!(mins > 29.0 && mins <= 30.0, "TTL {mins}");

    assert_eq!(reset(&app, &token, "newpass12").await, StatusCode::NO_CONTENT);
    assert_eq!(
        reset(&app, &token, "another12").await,
        StatusCode::CONFLICT,
        "[SEC] a reset token works once"
    );
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/auth/login",
            "198.51.100.2",
            Some(json!({"email": "flow@x.co", "password": "newpass12"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
}

#[sqlx::test(migrations = "./migrations")]
async fn an_expired_or_replaced_link_is_refused(pool: PgPool) {
    let app = make_app(pool.clone());
    register(&app, "old@x.co", "en").await;
    forgot(&app, "old@x.co", "198.51.100.3").await;
    let first = token_from(&mails_to("old@x.co", 1).await[0]);
    forgot(&app, "old@x.co", "198.51.100.3").await;
    let second = token_from(&mails_to("old@x.co", 2).await[1]);
    assert_ne!(first, second);
    assert!(
        mails_to("old@x.co", 2).await[1].body["subject"]
            .as_str()
            .unwrap()
            .starts_with("Reset"),
        "en mail"
    );
    assert_eq!(
        reset(&app, &first, "newpass12").await,
        StatusCode::CONFLICT,
        "[SEC] asking again retires the older link"
    );
    sqlx::query("UPDATE password_resets SET expires_at = now() - interval '1 minute' WHERE used_at IS NULL")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        reset(&app, &second, "newpass12").await,
        StatusCode::CONFLICT,
        "[SEC] an expired link is refused"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn forgot_answers_the_same_for_known_unknown_and_blocked(pool: PgPool) {
    let app = make_app(pool.clone());
    register(&app, "known@x.co", "en").await;
    register(&app, "blocked@x.co", "en").await;
    sqlx::query("UPDATE users SET blocked = true WHERE email = 'blocked@x.co'")
        .execute(&pool)
        .await
        .unwrap();

    let known = forgot(&app, "known@x.co", "198.51.100.4").await;
    let unknown = forgot(&app, "unknown@x.co", "198.51.100.4").await;
    let blocked = forgot(&app, "blocked@x.co", "198.51.100.4").await;
    assert_eq!(known.0, StatusCode::NO_CONTENT);
    assert_eq!(
        known, unknown,
        "[SEC] no account oracle: status, headers and body identical"
    );
    assert_eq!(known, blocked, "[SEC] a blocked account looks like an unknown one");

    assert_eq!(mails_to("known@x.co", 1).await.len(), 1);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        mails_to("unknown@x.co", 1).await.is_empty(),
        "no mail to an address without an account"
    );
    assert!(
        mails_to("blocked@x.co", 1).await.is_empty(),
        "no mail to a blocked account"
    );

    // The per-address limit counts every address alike: the 6th ask is 429 for both.
    for _ in 0..4 {
        assert_eq!(
            forgot(&app, "known@x.co", "198.51.100.4").await.0,
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            forgot(&app, "unknown@x.co", "198.51.100.4").await.0,
            StatusCode::NO_CONTENT
        );
    }
    let known6 = forgot(&app, "known@x.co", "198.51.100.5").await;
    let unknown6 = forgot(&app, "unknown@x.co", "198.51.100.5").await;
    assert_eq!(known6.0, StatusCode::TOO_MANY_REQUESTS, "[SEC] per-email limit");
    assert_eq!(known6, unknown6, "[SEC] the limit is no oracle either");
}

#[sqlx::test(migrations = "./migrations")]
async fn forgot_is_rate_limited_per_ip(pool: PgPool) {
    let app = make_app(pool);
    for i in 0..20 {
        let (st, _, _) = forgot(&app, &format!("nobody{i}@x.co"), "203.0.113.50").await;
        assert_eq!(st, StatusCode::NO_CONTENT, "request {i}");
    }
    let (st, _, _) = forgot(&app, "nobody99@x.co", "203.0.113.50").await;
    assert_eq!(
        st,
        StatusCode::TOO_MANY_REQUESTS,
        "[SEC] 21st request from one IP in an hour"
    );
    let (st, _, _) = forgot(&app, "nobody99@x.co", "203.0.113.51").await;
    assert_eq!(st, StatusCode::NO_CONTENT, "another IP is not affected");
}

#[sqlx::test(migrations = "./migrations")]
async fn login_code_arrives_and_signs_in(pool: PgPool) {
    let app = make_app(pool);
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/auth/email/request-code",
            "198.51.100.6",
            Some(json!({"email": "code@x.co", "locale": "uk"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    let m = &mails_to("code@x.co", 1).await[0];
    assert!(
        m.body["subject"].as_str().unwrap().starts_with("Код входу"),
        "no account yet → the request's language"
    );
    let text = m.body["text"].as_str().unwrap();
    let code: String = text.chars().filter(char::is_ascii_digit).take(6).collect();
    assert_eq!(code.len(), 6, "{text}");
    let (st, v) = send(
        &app,
        build(
            "POST",
            "/api/auth/email/verify-code",
            "198.51.100.6",
            Some(json!({"email": "code@x.co", "code": code})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert!(v["token"].is_string());
}

#[sqlx::test(migrations = "./migrations")]
async fn a_failing_resend_is_retried_twice_with_one_idempotency_key(pool: PgPool) {
    let app = make_app(pool);
    register(&app, "fail@x.co", "en").await;
    let (st, _, _) = forgot(&app, "fail@x.co", "198.51.100.7").await;
    assert_eq!(st, StatusCode::NO_CONTENT, "the answer does not wait on Resend");
    let attempts = mails_to("fail@x.co", 3).await;
    assert_eq!(attempts.len(), 3, "first try + 2 retries");
    assert!(
        attempts
            .iter()
            .all(|a| a.idempotency_key == attempts[0].idempotency_key),
        "one key → at most one mail"
    );
    tokio::time::sleep(Duration::from_secs(5)).await;
    assert_eq!(mails_to("fail@x.co", 4).await.len(), 3, "bounded: no 4th attempt");
}

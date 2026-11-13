//! TASK-304: change password while signed in, `has_password`, and `GET /api/features` with mail
//! OFF (this test binary never sets RESEND_API_KEY / EMAIL_FROM).

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use chrono::Utc;
use jsonwebtoken::{encode, EncodingKey, Header};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use underground_api::auth::Claims;
use underground_api::routes::router;
use underground_api::state::AppState;

const SECRET: &str = "test-secret";

fn make_app(pool: PgPool) -> Router {
    std::env::remove_var("RESEND_API_KEY");
    std::env::remove_var("EMAIL_FROM");
    router(AppState {
        db: pool,
        jwt_secret: SECRET.into(),
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

async fn register(app: &Router, email: &str, password: &str) -> String {
    let (st, v) = send(
        app,
        build(
            "POST",
            "/api/auth/register",
            None,
            Some(json!({"email": email, "password": password, "name": "U"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    v["token"].as_str().unwrap().to_string()
}

async fn login(app: &Router, email: &str, password: &str) -> StatusCode {
    send(
        app,
        build(
            "POST",
            "/api/auth/login",
            None,
            Some(json!({"email": email, "password": password})),
        ),
    )
    .await
    .0
}

fn token_with_iat(user_id: &str, iat: i64) -> String {
    let claims = Claims {
        sub: user_id.to_string(),
        exp: (Utc::now().timestamp() + 3600) as usize,
        iat,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(SECRET.as_bytes()),
    )
    .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn change_password_checks_the_current_one_and_signs_out_other_sessions(pool: PgPool) {
    let app = make_app(pool);
    let phone = register(&app, "cp@x.co", "password1").await;
    // A second session of the same account (another device).
    let (_, v) = send(
        &app,
        build(
            "POST",
            "/api/auth/login",
            None,
            Some(json!({"email": "cp@x.co", "password": "password1"})),
        ),
    )
    .await;
    let laptop = v["token"].as_str().unwrap().to_string();

    let (_, me) = send(&app, build("GET", "/api/me", Some(&phone), None)).await;
    assert_eq!(me["has_password"], json!(true));
    assert!(me.get("password_hash").is_none(), "[SEC] never the hash");

    // Wrong current password → 422 (not 401: the client must not take it for "signed out").
    let (st, v) = send(
        &app,
        build(
            "POST",
            "/api/me/password",
            Some(&phone),
            Some(json!({"current": "nope-nope", "new": "brandnew1"})),
        ),
    )
    .await;
    assert_eq!(
        st,
        StatusCode::UNPROCESSABLE_ENTITY,
        "[SEC] wrong current password refused"
    );
    assert_eq!(v["error"], "неправильний поточний пароль");
    // Missing current password → 422.
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/me/password",
            Some(&phone),
            Some(json!({"new": "brandnew1"})),
        ),
    )
    .await;
    assert_eq!(
        st,
        StatusCode::UNPROCESSABLE_ENTITY,
        "[SEC] a password account must prove the current one"
    );
    // Too short / too long.
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/me/password",
            Some(&phone),
            Some(json!({"current": "password1", "new": "short"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/me/password",
            Some(&phone),
            Some(json!({"current": "password1", "new": "x".repeat(257)})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        login(&app, "cp@x.co", "password1").await,
        StatusCode::OK,
        "nothing changed so far"
    );
    // No token → 401.
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/me/password",
            None,
            Some(json!({"current": "password1", "new": "brandnew1"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);

    // Success: a fresh token for this device.
    let (st, v) = send(
        &app,
        build(
            "POST",
            "/api/me/password",
            Some(&phone),
            Some(json!({"current": "password1", "new": "brandnew1"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let fresh = v["token"].as_str().unwrap().to_string();

    // Every token from before the change is dead, at once (same second included).
    let (st, _) = send(&app, build("GET", "/api/me", Some(&phone), None)).await;
    assert_eq!(
        st,
        StatusCode::UNAUTHORIZED,
        "[SEC] the old token of this device is dead"
    );
    let (st, _) = send(&app, build("GET", "/api/me", Some(&laptop), None)).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED, "[SEC] the other session is signed out");
    let (st, _) = send(&app, build("GET", "/api/me", Some(&fresh), None)).await;
    assert_eq!(st, StatusCode::OK, "the token from the answer works");

    assert_eq!(
        login(&app, "cp@x.co", "password1").await,
        StatusCode::UNAUTHORIZED,
        "old password gone"
    );
    assert_eq!(
        login(&app, "cp@x.co", "brandnew1").await,
        StatusCode::OK,
        "new password works"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn change_password_retires_open_reset_links(pool: PgPool) {
    let app = make_app(pool.clone());
    let token = register(&app, "rl@x.co", "password1").await;
    sqlx::query(
        "INSERT INTO password_resets (user_id, token_hash, expires_at) \
         SELECT id, $1, now() + interval '20 minutes' FROM users WHERE email = 'rl@x.co'",
    )
    .bind(underground_api::passwords::token_hash("open-link"))
    .execute(&pool)
    .await
    .unwrap();
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/me/password",
            Some(&token),
            Some(json!({"current": "password1", "new": "brandnew1"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/auth/reset-password",
            None,
            Some(json!({"token": "open-link", "password": "attacker1"})),
        ),
    )
    .await;
    assert_eq!(
        st,
        StatusCode::CONFLICT,
        "[SEC] a reset link from before the change cannot undo it"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn passwordless_account_sets_a_first_password_only_after_a_recent_sign_in(pool: PgPool) {
    let app = make_app(pool.clone());
    let id: uuid::Uuid = sqlx::query_scalar("INSERT INTO users (email, name) VALUES ('nopw@x.co', 'N') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();
    let id = id.to_string();

    let old = token_with_iat(&id, Utc::now().timestamp() - 11 * 60);
    let (_, me) = send(&app, build("GET", "/api/me", Some(&old), None)).await;
    assert_eq!(me["has_password"], json!(false));
    let (st, v) = send(
        &app,
        build(
            "POST",
            "/api/me/password",
            Some(&old),
            Some(json!({"new": "firstpass1"})),
        ),
    )
    .await;
    assert_eq!(
        st,
        StatusCode::UNAUTHORIZED,
        "[SEC] an 11-minute-old sign-in cannot set a password"
    );
    assert_eq!(v["error"], "увійдіть ще раз, щоб задати пароль");

    let recent = token_with_iat(&id, Utc::now().timestamp() - 60);
    let (st, v) = send(
        &app,
        build(
            "POST",
            "/api/me/password",
            Some(&recent),
            Some(json!({"new": "firstpass1"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let fresh = v["token"].as_str().unwrap();
    let (_, me) = send(&app, build("GET", "/api/me", Some(fresh), None)).await;
    assert_eq!(me["has_password"], json!(true));
    assert_eq!(login(&app, "nopw@x.co", "firstpass1").await, StatusCode::OK);
    // From now on it is a password account: the current password is required.
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/me/password",
            Some(fresh),
            Some(json!({"new": "another12"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "./migrations")]
async fn change_password_is_rate_limited_per_account(pool: PgPool) {
    let app = make_app(pool);
    let token = register(&app, "rate@x.co", "password1").await;
    for i in 0..10 {
        let (st, _) = send(
            &app,
            build(
                "POST",
                "/api/me/password",
                Some(&token),
                Some(json!({"current": format!("guess-{i}"), "new": "brandnew1"})),
            ),
        )
        .await;
        assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "attempt {i}");
    }
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/me/password",
            Some(&token),
            Some(json!({"current": "password1", "new": "brandnew1"})),
        ),
    )
    .await;
    assert_eq!(
        st,
        StatusCode::TOO_MANY_REQUESTS,
        "[SEC] 11th try in an hour refused, even with the right password"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn features_report_mail_off_without_a_key(pool: PgPool) {
    let app = make_app(pool);
    let resp = app
        .clone()
        .oneshot(build("GET", "/api/features", None, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers().get("cache-control").unwrap(), "no-store");
    let v: Value = serde_json::from_slice(&to_bytes(resp.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(v, json!({"email": false}));
    // With mail off, forgot-password still answers 204 (the web hides it; the API never tells).
    let (st, body) = send(
        &app,
        build(
            "POST",
            "/api/auth/forgot-password",
            None,
            Some(json!({"email": "nobody@x.co"})),
        ),
    )
    .await;
    assert_eq!((st, body), (StatusCode::NO_CONTENT, Value::Null));
}

/// A token minted in the very second of the change (before it) must die too: JWT `iat` has
/// one-second resolution, so the watermark goes one second past the change.
#[sqlx::test(migrations = "./migrations")]
async fn a_token_from_the_same_second_as_the_change_is_rejected(pool: PgPool) {
    let app = make_app(pool.clone());
    let mut token = register(&app, "same@x.co", "password1").await;
    let id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email = 'same@x.co'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let mut current = "password1".to_string();
    let mut checked = false;
    for round in 0..4 {
        // Start right after a second boundary so the change finishes within that second.
        let now = Utc::now();
        let wait = 1_000 - i64::from(now.timestamp_subsec_millis()) + 20;
        tokio::time::sleep(std::time::Duration::from_millis(wait as u64)).await;
        let before = Utc::now().timestamp();
        let next = format!("newpass-{round}");
        let (st, v) = send(
            &app,
            build(
                "POST",
                "/api/me/password",
                Some(&token),
                Some(json!({"current": current, "new": next})),
            ),
        )
        .await;
        assert_eq!(st, StatusCode::OK, "{v}");
        let after = Utc::now().timestamp();
        token = v["token"].as_str().unwrap().to_string();
        current = next;
        if before == after {
            let same_second = token_with_iat(&id.to_string(), before);
            let (st, _) = send(&app, build("GET", "/api/me", Some(&same_second), None)).await;
            assert_eq!(
                st,
                StatusCode::UNAUTHORIZED,
                "[SEC] a token minted in the second of the change is dead"
            );
            checked = true;
            break;
        }
    }
    assert!(checked, "the change never fitted in one second; the check did not run");
    let (st, _) = send(&app, build("GET", "/api/me", Some(&token), None)).await;
    assert_eq!(st, StatusCode::OK);
}

/// Review item 1: a reset signs out a token minted in the very second of the reset, too.
#[sqlx::test(migrations = "./migrations")]
async fn a_reset_kills_a_token_from_the_same_second(pool: PgPool) {
    let app = make_app(pool.clone());
    register(&app, "rs@x.co", "password1").await;
    let id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email = 'rs@x.co'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let mut checked = false;
    for round in 0..4 {
        let raw = format!("same-second-{round}");
        sqlx::query("INSERT INTO password_resets (user_id, token_hash, expires_at) VALUES ($1, $2, now() + interval '20 minutes')")
            .bind(id)
            .bind(underground_api::passwords::token_hash(&raw))
            .execute(&pool)
            .await
            .unwrap();
        let wait = 1_000 - i64::from(Utc::now().timestamp_subsec_millis()) + 20;
        tokio::time::sleep(std::time::Duration::from_millis(wait as u64)).await;
        let before = Utc::now().timestamp();
        let (st, _) = send(
            &app,
            build(
                "POST",
                "/api/auth/reset-password",
                None,
                Some(json!({"token": raw, "password": format!("resetpw-{round}")})),
            ),
        )
        .await;
        assert_eq!(st, StatusCode::NO_CONTENT);
        let after = Utc::now().timestamp();
        if before == after {
            // iat = the floored second of the reset (what `floor(watermark)` was before the fix).
            let same_second = token_with_iat(&id.to_string(), before);
            let (st, _) = send(&app, build("GET", "/api/me", Some(&same_second), None)).await;
            assert_eq!(
                st,
                StatusCode::UNAUTHORIZED,
                "[SEC] a token minted in the second of the reset is dead"
            );
            let next_second = token_with_iat(&id.to_string(), before + 1);
            let (st, _) = send(&app, build("GET", "/api/me", Some(&next_second), None)).await;
            assert_eq!(st, StatusCode::OK, "a sign-in after the reset works");
            checked = true;
            break;
        }
    }
    assert!(checked, "the reset never fitted in one second; the check did not run");
}

/// Review item 6: the bounds count characters, not bytes.
#[sqlx::test(migrations = "./migrations")]
async fn password_bounds_count_characters(pool: PgPool) {
    let app = make_app(pool);
    let token = register(&app, "chars@x.co", "password1").await;
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/me/password",
            Some(&token),
            Some(json!({"current": "password1", "new": "ж".repeat(4)})),
        ),
    )
    .await;
    assert_eq!(
        st,
        StatusCode::UNPROCESSABLE_ENTITY,
        "4 characters (8 bytes) is too short"
    );
    let (st, v) = send(
        &app,
        build(
            "POST",
            "/api/me/password",
            Some(&token),
            Some(json!({"current": "password1", "new": "ж".repeat(200)})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "200 characters (400 bytes) is accepted: {v}");
    assert_eq!(login(&app, "chars@x.co", &"ж".repeat(200)).await, StatusCode::OK);
}

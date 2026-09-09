//! Integration tests for organizer KYC (P1 #78).

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

async fn register(app: &Router, email: &str) -> (String, String) {
    let (_, v) = send(
        app,
        build(
            "POST",
            "/api/auth/register",
            None,
            Some(json!({"email":email,"password":"password1","name":"U"})),
        ),
    )
    .await;
    (
        v["token"].as_str().unwrap().to_string(),
        v["user"]["id"].as_str().unwrap().to_string(),
    )
}

/// Configure a provider that MOVES REAL MONEY, which is what makes the payout wall apply at all:
/// with the test/noop providers there is no destination to be missing, so no wall.
async fn money_moving_provider(pool: &PgPool) {
    sqlx::query(
        "UPDATE payment_settings SET provider = 'stripe', mode = 'test', \
            stripe_secret_key = 'sk_test_x', stripe_webhook_secret = 'whsec_x' WHERE id = TRUE",
    )
    .execute(pool)
    .await
    .unwrap();
}

/// Helper: make this user an ADMIN.
async fn make_admin(pool: &PgPool, email: &str) {
    sqlx::query("UPDATE users SET role = 'ADMIN' WHERE email = $1")
        .bind(email)
        .execute(pool)
        .await
        .unwrap();
}

/// A FREE event needs no payment setup at all: the wall appears only where money would move.
/// This is the first thing a new organiser does, and a payment screen here would be a wall in
/// front of the product for no reason.
#[sqlx::test(migrations = "./migrations")]
async fn a_free_event_publishes_without_any_payment_setup(pool: PgPool) {
    let app = make_app(pool.clone());
    // With a real money-moving provider configured, so this proves the wall does not apply to a
    // free event rather than passing because there is no wall at all.
    money_moving_provider(&pool).await;
    let (org, _) = register(&app, "org@x.co").await;
    let (st, v) = send(
        &app,
        build(
            "POST",
            "/api/events",
            Some(&org),
            Some(
                json!({"title":"Free gig","category":"Rock","cost_type":"free","price":"0",
                        "tickets_total":10,"status":"published"}),
            ),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "{v}");
    assert_eq!(v["status"], "published");
}

/// [SEC] A PAID event cannot be published until the provider says the organiser's account can
/// actually take a card. Otherwise we would sell tickets whose payment has no destination.
#[sqlx::test(migrations = "./migrations")]
async fn a_paid_event_cannot_be_published_until_payouts_are_connected(pool: PgPool) {
    let app = make_app(pool.clone());
    money_moving_provider(&pool).await;
    let (org, org_id) = register(&app, "org@x.co").await;
    let paid = json!({"title":"Paid gig","category":"Rock","price":"250","currency":"UAH",
                      "tickets_total":10,"status":"published"});

    let (st, v) = send(&app, build("POST", "/api/events", Some(&org), Some(paid.clone()))).await;
    assert_eq!(st, StatusCode::CONFLICT, "{v}");
    assert!(v["error"].as_str().unwrap().contains("виплати"), "{v}");

    // A DRAFT is fine — nothing is on sale yet.
    let mut draft = paid.clone();
    draft["status"] = json!("draft");
    let (st, dv) = send(&app, build("POST", "/api/events", Some(&org), Some(draft))).await;
    assert_eq!(st, StatusCode::CREATED, "{dv}");
    let event_id = dv["id"].as_str().unwrap().to_string();

    // …and publishing that draft later hits the SAME gate: a second path to the same state.
    let (st, _) = send(
        &app,
        build(
            "PUT",
            &format!("/api/events/{event_id}"),
            Some(&org),
            Some(json!({"status":"published"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "[SEC] publishing a draft is gated too");

    // Once the provider says charges are enabled, both paths open.
    sqlx::query("UPDATE users SET psp_account_id = 'acct_x', psp_charges_enabled = true WHERE id = $1::uuid")
        .bind(&org_id)
        .execute(&pool)
        .await
        .unwrap();
    let (st, _) = send(
        &app,
        build(
            "PUT",
            &format!("/api/events/{event_id}"),
            Some(&org),
            Some(json!({"status":"published"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let (st, _) = send(&app, build("POST", "/api/events", Some(&org), Some(paid))).await;
    assert_eq!(st, StatusCode::CREATED);
}

/// [SEC] A blocked organiser cannot sell, whatever the provider thinks of their account: the
/// provider knows whether the account is valid, not whether we have suspended them.
#[sqlx::test(migrations = "./migrations")]
async fn a_blocked_organiser_cannot_publish_a_paid_event(pool: PgPool) {
    let app = make_app(pool.clone());
    money_moving_provider(&pool).await;
    let (org, org_id) = register(&app, "org@x.co").await;
    sqlx::query(
        "UPDATE users SET psp_account_id = 'acct_x', psp_charges_enabled = true, blocked = true WHERE id = $1::uuid",
    )
    .bind(&org_id)
    .execute(&pool)
    .await
    .unwrap();
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/events",
            Some(&org),
            Some(json!({"title":"Paid","category":"Rock","price":"250","currency":"UAH",
                        "tickets_total":5,"status":"published"})),
        ),
    )
    .await;
    // Blocked is refused earlier and harder than the payment gate: the token stops authenticating
    // at all. The point stands — the provider's capability flags never override our suspension.
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    let published: i64 = sqlx::query_scalar("SELECT count(*) FROM events WHERE status = 'published'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(published, 0, "nothing went on sale");
}

/// Connecting twice must not create two accounts at the provider: the second call resumes the
/// first. A forgotten second account is one that can receive real money nobody reconciles.
#[sqlx::test(migrations = "./migrations")]
async fn connecting_twice_reuses_the_same_account(pool: PgPool) {
    let app = make_app(pool.clone());
    let (org, org_id) = register(&app, "org@x.co").await;
    sqlx::query("UPDATE payment_settings SET provider = 'test', mode = 'test' WHERE id = TRUE")
        .execute(&pool)
        .await
        .unwrap();

    let (st, first) = send(
        &app,
        build(
            "POST",
            "/api/me/payouts/connect",
            Some(&org),
            Some(json!({"country":"pl"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{first}");
    let url = first["onboarding_url"].as_str().unwrap();
    assert!(url.contains("onboarding="), "{url}");
    // One query string, not two: the return URL already carries `?done=1`, and a link with a
    // second `?` is not one an organiser can follow.
    assert_eq!(url.matches('?').count(), 1, "malformed onboarding URL: {url}");

    let stored: Option<String> = sqlx::query_scalar("SELECT psp_account_id FROM users WHERE id = $1::uuid")
        .bind(&org_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(stored.is_some());

    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/me/payouts/connect",
            Some(&org),
            Some(json!({"country":"de"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let after: Option<String> = sqlx::query_scalar("SELECT psp_account_id FROM users WHERE id = $1::uuid")
        .bind(&org_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        after, stored,
        "the second call resumes the first account, it does not make another"
    );
}

/// [SEC] The account id comes ONLY from the provider. A client that names one in the request
/// body is naming somebody else's account — and the money that would follow it.
#[sqlx::test(migrations = "./migrations")]
async fn sec_the_account_id_can_never_come_from_the_request_body(pool: PgPool) {
    let app = make_app(pool.clone());
    let (org, org_id) = register(&app, "org@x.co").await;
    sqlx::query("UPDATE payment_settings SET provider = 'test', mode = 'test' WHERE id = TRUE")
        .execute(&pool)
        .await
        .unwrap();
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/me/payouts/connect",
            Some(&org),
            Some(
                json!({"country":"pl","psp_account_id":"acct_victim","account":"acct_victim",
                        "connected_account":"acct_victim"}),
            ),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let stored: String = sqlx::query_scalar("SELECT psp_account_id FROM users WHERE id = $1::uuid")
        .bind(&org_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_ne!(
        stored, "acct_victim",
        "[SEC] the body must not be able to name an account"
    );
    assert!(stored.starts_with("acct_test_"), "the provider chose it: {stored}");
}

/// A two-letter country is required; anything else is a validation error rather than a request
/// the provider will reject in its own words.
#[sqlx::test(migrations = "./migrations")]
async fn connect_validates_the_country(pool: PgPool) {
    let app = make_app(pool.clone());
    let (org, _) = register(&app, "org@x.co").await;
    for bad in [json!({"country":""}), json!({"country":"POL"}), json!({"country":"1!"})] {
        let (st, _) = send(&app, build("POST", "/api/me/payouts/connect", Some(&org), Some(bad))).await;
        assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    }
}

/// The status endpoint answers for the CALLER and nobody else, and reports the three states the
/// organiser cabinet renders.
#[sqlx::test(migrations = "./migrations")]
async fn payout_status_is_own_only_and_reports_the_three_states(pool: PgPool) {
    let app = make_app(pool.clone());
    let (org, org_id) = register(&app, "org@x.co").await;
    let (other, _) = register(&app, "other@x.co").await;

    let (st, v) = send(&app, build("GET", "/api/me/payouts/status", Some(&org), None)).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v["connected"], false);
    assert_eq!(v["charges_enabled"], false);
    assert_eq!(v["payouts_enabled"], false);

    sqlx::query(
        "UPDATE users SET psp_account_id = 'acct_org', psp_country = 'PL', psp_charges_enabled = true, \
            psp_requirements_due = '[\"individual.verification.document\"]'::jsonb WHERE id = $1::uuid",
    )
    .bind(&org_id)
    .execute(&pool)
    .await
    .unwrap();
    let (_, v) = send(&app, build("GET", "/api/me/payouts/status", Some(&org), None)).await;
    assert_eq!(v["connected"], true);
    assert_eq!(v["charges_enabled"], true);
    assert_eq!(v["payouts_enabled"], false, "selling is allowed before money can leave");
    assert_eq!(v["country"], "PL");
    assert_eq!(v["requirements_due"][0], "individual.verification.document");

    // Another organiser sees their OWN empty state, never this one's.
    let (_, ov) = send(&app, build("GET", "/api/me/payouts/status", Some(&other), None)).await;
    assert_eq!(ov["connected"], false);
    assert!(ov["country"].is_null());

    // Unauthenticated has no state to see at all.
    let (st, _) = send(&app, build("GET", "/api/me/payouts/status", None, None)).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
}

/// [SEC] An admin may block an organiser, but may not hand them the ability to take money: the
/// capability flags mirror the provider, and no admin endpoint writes them.
#[sqlx::test(migrations = "./migrations")]
async fn sec_an_admin_cannot_switch_on_the_provider_capabilities(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org_id) = register(&app, "org@x.co").await;
    let (admin, _) = register(&app, "admin@x.co").await;
    make_admin(&pool, "admin@x.co").await;

    let (st, _) = send(
        &app,
        build(
            "PATCH",
            &format!("/api/admin/users/{org_id}"),
            Some(&admin),
            Some(
                json!({"verified":true,"psp_charges_enabled":true,"psp_payouts_enabled":true,
                        "psp_account_id":"acct_admin_granted"}),
            ),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let row: (bool, bool, Option<String>) = sqlx::query_as(
        "SELECT psp_charges_enabled, psp_payouts_enabled, psp_account_id FROM users WHERE id = $1::uuid",
    )
    .bind(&org_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        row,
        (false, false, None),
        "[SEC] only the provider grants the ability to take money"
    );
}

/// An admin still records OUR own verdict on an organiser, which is a different thing from the
/// provider's capability flags.
#[sqlx::test(migrations = "./migrations")]
async fn admin_records_the_platforms_own_kyc_verdict(pool: PgPool) {
    let app = make_app(pool.clone());
    let (_, org_id) = register(&app, "org@x.co").await;
    let (admin, _) = register(&app, "admin@x.co").await;
    make_admin(&pool, "admin@x.co").await;
    let (st, _) = send(
        &app,
        build(
            "POST",
            &format!("/api/admin/users/{org_id}/kyc"),
            Some(&admin),
            Some(json!({"status":"verified"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    let s: String = sqlx::query_scalar("SELECT kyc_status FROM users WHERE id = $1::uuid")
        .bind(&org_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(s, "verified");

    let (st, _) = send(
        &app,
        build(
            "POST",
            &format!("/api/admin/users/{org_id}/kyc"),
            Some(&admin),
            Some(json!({"status":"bogus"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
}

/// The compare-and-set that makes a racing second connect adopt the first account rather than
/// overwrite it. Reached by planting an account between the read and the write, which is what a
/// concurrent request does.
#[sqlx::test(migrations = "./migrations")]
async fn a_racing_connect_adopts_the_account_that_won(pool: PgPool) {
    let app = make_app(pool.clone());
    let (org, org_id) = register(&app, "org@x.co").await;
    sqlx::query("UPDATE payment_settings SET provider = 'test', mode = 'test' WHERE id = TRUE")
        .execute(&pool)
        .await
        .unwrap();

    // A concurrent call got there first and stored ITS account. Our request has already read
    // `psp_account_id = NULL`, so it will create one at the provider and try to store it.
    sqlx::query("UPDATE users SET psp_account_id = 'acct_winner' WHERE id = $1::uuid")
        .bind(&org_id)
        .execute(&pool)
        .await
        .unwrap();

    let (st, v) = send(
        &app,
        build(
            "POST",
            "/api/me/payouts/connect",
            Some(&org),
            Some(json!({"country":"pl"})),
        ),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let stored: String = sqlx::query_scalar("SELECT psp_account_id FROM users WHERE id = $1::uuid")
        .bind(&org_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        stored, "acct_winner",
        "the first writer wins; the loser adopts it instead of pointing us at an orphan account"
    );
    assert!(
        v["onboarding_url"].as_str().unwrap().contains("acct_winner"),
        "and the organiser is sent to onboard the account we actually kept: {v}"
    );
}

/// [SEC] A blocked organiser cannot start payout onboarding at all — being blocked is our
/// decision and the provider knows nothing about it.
#[sqlx::test(migrations = "./migrations")]
async fn sec_a_blocked_organiser_cannot_connect_payouts(pool: PgPool) {
    let app = make_app(pool.clone());
    let (org, org_id) = register(&app, "org@x.co").await;
    sqlx::query("UPDATE payment_settings SET provider = 'test', mode = 'test' WHERE id = TRUE")
        .execute(&pool)
        .await
        .unwrap();
    // Blocked in the database only, so the token still authenticates and the handler's own guard
    // is what has to refuse — not the auth layer.
    sqlx::query("UPDATE users SET blocked = true WHERE id = $1::uuid")
        .bind(&org_id)
        .execute(&pool)
        .await
        .unwrap();
    let (st, _) = send(
        &app,
        build(
            "POST",
            "/api/me/payouts/connect",
            Some(&org),
            Some(json!({"country":"pl"})),
        ),
    )
    .await;
    assert!(
        st == StatusCode::FORBIDDEN || st == StatusCode::UNAUTHORIZED,
        "a blocked organiser must not onboard, got {st}"
    );
    let stored: Option<String> = sqlx::query_scalar("SELECT psp_account_id FROM users WHERE id = $1::uuid")
        .bind(&org_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(stored.is_none());
}

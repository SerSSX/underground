//! Event reminders (TASK-017 M4): the sweep reminds going + interested users of
//! events starting within the window, exactly once, and skips events outside it.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use chrono::{Duration, Utc};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use underground_api::reminders;
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

async fn register(app: &Router, email: &str) -> String {
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
    v["token"].as_str().unwrap().to_string()
}

async fn create_event_at(app: &Router, token: &str, title: &str, starts_at: &str) -> String {
    let (status, v) = send(
        app,
        build(
            "POST",
            "/api/events",
            Some(token),
            Some(json!({"title":title,"category":"Rock","city":"Kyiv","price":"0","tickets_total":10,"cost_type":"free","starts_at":starts_at})),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    v["id"].as_str().unwrap().to_string()
}

async fn notifs(app: &Router, token: &str) -> Vec<Value> {
    let (_, v) = send(app, build("GET", "/api/me/notifications", Some(token), None)).await;
    v.as_array().unwrap().clone()
}

#[sqlx::test(migrations = "./migrations")]
async fn sweep_reminds_going_and_interested_once(pool: PgPool) {
    let app = make_app(pool.clone());
    let maker = register(&app, "m@x.co").await;
    let holder = register(&app, "h@x.co").await;
    let saver = register(&app, "s@x.co").await;

    let soon = (Utc::now() + Duration::hours(2)).to_rfc3339();
    let far = (Utc::now() + Duration::hours(48)).to_rfc3339();
    let soon_ev = create_event_at(&app, &maker, "Soon", &soon).await;
    let far_ev = create_event_at(&app, &maker, "Far", &far).await;

    // holder is "going" (free ticket) to both; saver is "interested" in the soon event.
    send(
        &app,
        build("POST", &format!("/api/events/{soon_ev}/reserve"), Some(&holder), None),
    )
    .await;
    send(
        &app,
        build("POST", &format!("/api/events/{far_ev}/reserve"), Some(&holder), None),
    )
    .await;
    send(
        &app,
        build("POST", &format!("/api/events/{soon_ev}/save"), Some(&saver), None),
    )
    .await;
    // The saver prefers English → their reminder is localized (TASK-017 M5).
    send(
        &app,
        build(
            "PUT",
            "/api/me/prefs",
            Some(&saver),
            Some(json!({"categories": [], "locale": "en"})),
        ),
    )
    .await;

    // Only the soon event (within 24h) reminds its 2 users.
    let n = reminders::sweep(&pool, 24).await.unwrap();
    assert_eq!(n, 2, "holder + saver of the soon event");
    // Idempotent: a second sweep emits nothing (dedup ledger).
    assert_eq!(reminders::sweep(&pool, 24).await.unwrap(), 0);

    for tok in [&holder, &saver] {
        let list = notifs(&app, tok).await;
        assert!(
            list.iter()
                .any(|x| x["kind"] == "event_reminder" && x["event_id"] == soon_ev),
            "reminded about the soon event"
        );
    }
    // The reminder body is localized per recipient locale.
    let saver_rem = notifs(&app, &saver).await;
    let sr = saver_rem.iter().find(|x| x["kind"] == "event_reminder").unwrap();
    assert_eq!(sr["body"], "Event starts soon", "saver locale=en → English body");
    let holder_rem = notifs(&app, &holder).await;
    let hr = holder_rem.iter().find(|x| x["kind"] == "event_reminder").unwrap();
    assert_eq!(
        hr["body"], "Подія скоро розпочнеться",
        "holder default uk → Ukrainian body"
    );
    // The far event (>24h) never reminds anyone.
    let hl = notifs(&app, &holder).await;
    assert!(!hl
        .iter()
        .any(|x| x["kind"] == "event_reminder" && x["event_id"] == far_ev));
}

#[sqlx::test(migrations = "./migrations")]
async fn review_prompt_sweep_prompts_attendees_once(pool: PgPool) {
    let app = make_app(pool.clone());
    let maker = register(&app, "m@x.co").await;
    let attendee = register(&app, "a@x.co").await;
    let stranger = register(&app, "n@x.co").await;

    // Create a valid (future) event, reserve a ticket, then move it into the past (ended 2h ago).
    let ev = create_event_at(
        &app,
        &maker,
        "Past Show",
        &(Utc::now() + Duration::hours(2)).to_rfc3339(),
    )
    .await;
    send(
        &app,
        build("POST", &format!("/api/events/{ev}/reserve"), Some(&attendee), None),
    )
    .await;
    send(
        &app,
        build(
            "PUT",
            "/api/me/prefs",
            Some(&attendee),
            Some(json!({"categories": [], "locale": "en"})),
        ),
    )
    .await;
    sqlx::query("UPDATE events SET starts_at = now() - interval '5 hours', ends_at = now() - interval '2 hours' WHERE id = $1::uuid")
        .bind(&ev)
        .execute(&pool)
        .await
        .unwrap();

    let n = reminders::review_prompt_sweep(&pool, 14).await.unwrap();
    assert_eq!(n, 1, "the attendee is prompted for a review");
    assert_eq!(
        reminders::review_prompt_sweep(&pool, 14).await.unwrap(),
        0,
        "idempotent — no second prompt"
    );

    let list = notifs(&app, &attendee).await;
    let p = list
        .iter()
        .find(|x| x["kind"] == "review_prompt")
        .expect("attendee got a review prompt");
    assert_eq!(p["event_id"], ev);
    assert_eq!(p["body"], "How was it? Leave a review", "en locale body");
    assert!(
        notifs(&app, &stranger)
            .await
            .iter()
            .all(|x| x["kind"] != "review_prompt"),
        "a non-attendee is never prompted"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn review_prompt_sweep_skips_reviewed_and_stale(pool: PgPool) {
    let app = make_app(pool.clone());
    let maker = register(&app, "m@x.co").await;
    let reviewer = register(&app, "r@x.co").await;

    // A recently-ended event the reviewer attended AND already reviewed → no prompt.
    let recent = create_event_at(&app, &maker, "Recent", &(Utc::now() + Duration::hours(2)).to_rfc3339()).await;
    send(
        &app,
        build("POST", &format!("/api/events/{recent}/reserve"), Some(&reviewer), None),
    )
    .await;
    sqlx::query("UPDATE events SET starts_at = now() - interval '4 hours', ends_at = now() - interval '1 hour' WHERE id = $1::uuid")
        .bind(&recent)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO reviews (event_id, author_id, rating) SELECT $1::uuid, u.id, 5 FROM users u WHERE u.email = 'r@x.co'")
        .bind(&recent)
        .execute(&pool)
        .await
        .unwrap();

    // A stale ended event (30 days ago) the reviewer attended → outside recency window → no prompt.
    let stale = create_event_at(&app, &maker, "Stale", &(Utc::now() + Duration::hours(2)).to_rfc3339()).await;
    send(
        &app,
        build("POST", &format!("/api/events/{stale}/reserve"), Some(&reviewer), None),
    )
    .await;
    sqlx::query("UPDATE events SET starts_at = now() - interval '30 days', ends_at = now() - interval '30 days' WHERE id = $1::uuid")
        .bind(&stale)
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(
        reminders::review_prompt_sweep(&pool, 14).await.unwrap(),
        0,
        "already-reviewed and stale events prompt nobody"
    );
    assert!(notifs(&app, &reviewer)
        .await
        .iter()
        .all(|x| x["kind"] != "review_prompt"));
}

#[sqlx::test(migrations = "./migrations")]
async fn selling_fast_sweep_nudges_low_stock_savers(pool: PgPool) {
    let app = make_app(pool.clone());
    let maker = register(&app, "m@x.co").await;
    let saver = register(&app, "s@x.co").await;
    let holder = register(&app, "h@x.co").await;

    let future = (Utc::now() + Duration::hours(48)).to_rfc3339();
    let low = create_event_at(&app, &maker, "Almost Gone", &future).await; // tickets_total = 10
    let full = create_event_at(&app, &maker, "Plenty", &future).await;

    // saver saves both; holder saves the low-stock one AND holds a ticket to it.
    for ev in [&low, &full] {
        send(
            &app,
            build("POST", &format!("/api/events/{ev}/save"), Some(&saver), None),
        )
        .await;
    }
    send(
        &app,
        build("POST", &format!("/api/events/{low}/save"), Some(&holder), None),
    )
    .await;
    send(
        &app,
        build("POST", &format!("/api/events/{low}/reserve"), Some(&holder), None),
    )
    .await;

    // Drive `low` to 1/10 = 10% (≤ 15%); `full` stays well-stocked.
    sqlx::query("UPDATE events SET tickets_left = 1 WHERE id = $1::uuid")
        .bind(&low)
        .execute(&pool)
        .await
        .unwrap();

    let n = reminders::selling_fast_sweep(&pool, 0.15, 5).await.unwrap();
    assert_eq!(
        n, 1,
        "only the saver (not the ticket-holder) of the low-stock event is nudged"
    );
    assert_eq!(
        reminders::selling_fast_sweep(&pool, 0.15, 5).await.unwrap(),
        0,
        "idempotent"
    );

    let list = notifs(&app, &saver).await;
    assert!(
        list.iter().any(|x| x["kind"] == "selling_fast" && x["event_id"] == low),
        "saver is nudged about the low-stock event"
    );
    assert!(
        list.iter()
            .all(|x| !(x["kind"] == "selling_fast" && x["event_id"] == full)),
        "a well-stocked event is never flagged as selling fast"
    );
    assert!(
        notifs(&app, &holder).await.iter().all(|x| x["kind"] != "selling_fast"),
        "existing ticket-holders are not nudged to buy"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn selling_fast_sweep_respects_daily_cap(pool: PgPool) {
    // TASK-210: a heavy saver whose many saved events all dip low in one tick is not spammed —
    // the per-user rolling cap bounds nudges within a statement AND across successive ticks.
    let app = make_app(pool.clone());
    let maker = register(&app, "m@x.co").await;
    let saver = register(&app, "s@x.co").await;

    let future = (Utc::now() + Duration::hours(48)).to_rfc3339();
    let mut evs = Vec::new();
    for i in 0..3 {
        let ev = create_event_at(&app, &maker, &format!("Low {i}"), &future).await;
        send(
            &app,
            build("POST", &format!("/api/events/{ev}/save"), Some(&saver), None),
        )
        .await;
        sqlx::query("UPDATE events SET tickets_left = 1 WHERE id = $1::uuid")
            .bind(&ev)
            .execute(&pool)
            .await
            .unwrap();
        evs.push(ev);
    }

    // Cap = 2: three eligible events, but at most two nudges land in one pass...
    let first = reminders::selling_fast_sweep(&pool, 0.15, 2).await.unwrap();
    assert_eq!(first, 2, "one statement never exceeds the per-user daily cap");
    // ...and a second same-window pass adds nothing, though the 3rd event is still eligible.
    let second = reminders::selling_fast_sweep(&pool, 0.15, 2).await.unwrap();
    assert_eq!(
        second, 0,
        "the rolling-window budget is exhausted across ticks, not just per statement"
    );

    let got = notifs(&app, &saver)
        .await
        .iter()
        .filter(|x| x["kind"] == "selling_fast")
        .count();
    assert_eq!(got, 2, "the saver holds exactly the capped number of nudges");
}

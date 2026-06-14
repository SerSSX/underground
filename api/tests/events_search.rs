//! Integration tests for `GET /api/events` filters (q/city/category/free/sort).

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::Value;
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

/// Seed a fixed catalog the filter assertions are written against.
async fn seed_events(pool: &PgPool) {
    sqlx::query(
        "INSERT INTO events (title, category, city, starts_at, price, tickets_total, tickets_left, cost_type, status) VALUES \
         ('Louna Live',    'Rock',   'Kyiv',  '2026-09-03T19:00:00Z', 450, 100, 100, 'paid', 'published'), \
         ('Jazz Evening',  'Jazz',   'Kyiv',  '2026-09-01T19:00:00Z', 300,  50,  50, 'paid', 'published'), \
         ('Poetry Night',  'Poetry', 'Lviv',  '2026-09-02T19:00:00Z',   0,  30,  30, 'free', 'published'), \
         ('Rock Marathon', 'Rock',   'Lviv',  '2026-09-04T19:00:00Z', 150, 200, 200, 'paid', 'published'), \
         ('Hidden Rave',   'Rock',   'Kyiv',  '2026-09-05T19:00:00Z', 500, 100, 100, 'paid', 'hidden')",
    )
    .execute(pool)
    .await
    .unwrap();
}

async fn list(app: &Router, query: &str) -> Vec<Value> {
    let uri = if query.is_empty() {
        "/api/events".to_string()
    } else {
        format!("/api/events?{query}")
    };
    let resp = app
        .clone()
        .oneshot(Request::get(&uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "GET {uri}");
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    v.as_array().unwrap().clone()
}

fn titles(events: &[Value]) -> Vec<&str> {
    events.iter().map(|e| e["title"].as_str().unwrap()).collect()
}

#[sqlx::test(migrations = "./migrations")]
async fn no_filters_returns_published_soonest_first(pool: PgPool) {
    seed_events(&pool).await;
    let app = make_app(pool);
    let events = list(&app, "").await;
    assert_eq!(
        titles(&events),
        vec!["Jazz Evening", "Poetry Night", "Louna Live", "Rock Marathon"],
        "published only, ordered by starts_at ASC; hidden excluded"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn q_searches_title_case_insensitively(pool: PgPool) {
    seed_events(&pool).await;
    let app = make_app(pool);
    assert_eq!(titles(&list(&app, "q=rock").await), vec!["Rock Marathon"]);
    assert_eq!(titles(&list(&app, "q=LOUNA").await), vec!["Louna Live"]);
    assert!(list(&app, "q=nothing-matches").await.is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn q_with_like_metacharacters_is_literal(pool: PgPool) {
    seed_events(&pool).await;
    let app = make_app(pool);
    // [SEC] `%` must not act as a match-all wildcard injection.
    assert!(list(&app, "q=%25%25").await.is_empty(), "q=%% finds nothing");
}

#[sqlx::test(migrations = "./migrations")]
async fn city_filter(pool: PgPool) {
    seed_events(&pool).await;
    let app = make_app(pool);
    assert_eq!(
        titles(&list(&app, "city=Kyiv").await),
        vec!["Jazz Evening", "Louna Live"]
    );
    assert_eq!(
        titles(&list(&app, "city=Lviv").await),
        vec!["Poetry Night", "Rock Marathon"]
    );
    assert!(list(&app, "city=Odesa").await.is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn category_filter(pool: PgPool) {
    seed_events(&pool).await;
    let app = make_app(pool);
    assert_eq!(
        titles(&list(&app, "category=Rock").await),
        vec!["Louna Live", "Rock Marathon"],
        "hidden Rock event excluded"
    );
    assert_eq!(titles(&list(&app, "category=Poetry").await), vec!["Poetry Night"]);
}

#[sqlx::test(migrations = "./migrations")]
async fn free_filter_both_ways(pool: PgPool) {
    seed_events(&pool).await;
    let app = make_app(pool);
    assert_eq!(titles(&list(&app, "free=true").await), vec!["Poetry Night"]);
    assert_eq!(
        titles(&list(&app, "free=false").await),
        vec!["Jazz Evening", "Louna Live", "Rock Marathon"]
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn sort_by_price_both_directions(pool: PgPool) {
    seed_events(&pool).await;
    let app = make_app(pool);
    assert_eq!(
        titles(&list(&app, "sort=price").await),
        vec!["Poetry Night", "Rock Marathon", "Jazz Evening", "Louna Live"]
    );
    assert_eq!(
        titles(&list(&app, "sort=price_desc").await),
        vec!["Louna Live", "Jazz Evening", "Rock Marathon", "Poetry Night"]
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn unknown_sort_falls_back_to_date(pool: PgPool) {
    seed_events(&pool).await;
    let app = make_app(pool);
    assert_eq!(
        titles(&list(&app, "sort=evil;DROP%20TABLE").await),
        vec!["Jazz Evening", "Poetry Night", "Louna Live", "Rock Marathon"],
        "unknown sort key ignored (allow-list), default date order"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn filters_combine(pool: PgPool) {
    seed_events(&pool).await;
    let app = make_app(pool);
    assert_eq!(
        titles(&list(&app, "city=Lviv&category=Rock&free=false").await),
        vec!["Rock Marathon"]
    );
    assert!(list(&app, "city=Kyiv&free=true").await.is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn blank_params_are_ignored(pool: PgPool) {
    seed_events(&pool).await;
    let app = make_app(pool);
    assert_eq!(
        list(&app, "q=&city=%20&category=").await.len(),
        4,
        "empty/whitespace params do not filter"
    );
}

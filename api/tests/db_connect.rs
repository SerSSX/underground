//! Integration test for the pool/migration bootstrap (`db::connect`).
//! Requires a reachable Postgres (compose `db`); uses `DATABASE_URL` or the dev default.

use underground_api::db;

fn database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://underground:underground_dev_pw@localhost:5544/underground_dev".to_string())
}

#[tokio::test]
async fn connect_runs_migrations_and_queries() {
    let pool = db::connect(&database_url()).await.expect("connect + migrate");
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM events")
        .fetch_one(&pool)
        .await
        .expect("events table queryable after migrations");
    assert!(count >= 0);
    pool.close().await;
}

#[tokio::test]
async fn connect_fails_on_unreachable_server() {
    // Negative path: unreachable host:port → connect returns Err (covers error branch).
    let res = db::connect("postgres://nobody:nobody@127.0.0.1:1/none").await;
    assert!(res.is_err());
}

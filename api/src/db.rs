//! Postgres connection pool.

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

/// Opens a connection pool and runs pending migrations.
pub async fn connect(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new().max_connections(10).connect(database_url).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

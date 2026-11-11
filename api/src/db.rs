//! Postgres connection pool.

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

/// Opens a connection pool and runs pending migrations.
pub async fn connect(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new().max_connections(10).connect(database_url).await?;
    // ignore_missing: a binary from an older release must still start against a schema a newer release
    // already migrated (schema changes are expand-contract, so the older code keeps working). Without it,
    // rolling back after any release that added a migration fails with VersionMissing and the stand stays down.
    sqlx::migrate!("./migrations")
        .set_ignore_missing(true)
        .run(&pool)
        .await?;
    Ok(pool)
}

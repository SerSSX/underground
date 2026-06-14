//! Integration tests for the event read repository.
//! `#[sqlx::test]` provisions an ephemeral database and runs migrations (rules §6.3).

use sqlx::PgPool;
use underground_api::events::fetch_published;

#[sqlx::test(migrations = "./migrations")]
async fn empty_when_no_events(pool: PgPool) -> sqlx::Result<()> {
    let events = fetch_published(&pool).await?;
    assert!(events.is_empty());
    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn returns_only_published_sorted_by_start(pool: PgPool) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO events (title, status, starts_at) VALUES ($1, 'published', now() + interval '2 day')")
        .bind("Later")
        .execute(&pool)
        .await?;
    sqlx::query("INSERT INTO events (title, status, starts_at) VALUES ($1, 'published', now() + interval '1 day')")
        .bind("Sooner")
        .execute(&pool)
        .await?;
    sqlx::query("INSERT INTO events (title, status) VALUES ($1, 'draft')")
        .bind("Hidden")
        .execute(&pool)
        .await?;

    let events = fetch_published(&pool).await?;

    assert_eq!(events.len(), 2, "draft event must be excluded");
    assert_eq!(events[0].title, "Sooner", "soonest first");
    assert_eq!(events[1].title, "Later");
    Ok(())
}

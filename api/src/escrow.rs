//! Escrow clearing lifecycle (TASK-017 M6). A background sweep advances held funds to
//! `clearing` once the event is over, then to `cleared` once the clearing window has
//! elapsed — at which point an admin can settle the payout. Frozen (disputed) orders are
//! left untouched until an admin resolves them.

use sqlx::PgPool;

/// Run one clearing pass. Returns `(newly_clearing, newly_cleared)`.
/// Event end is `ends_at`, falling back to `starts_at` when `ends_at` is unset.
pub async fn sweep(db: &PgPool, clearing_days: i32) -> Result<(u64, u64), sqlx::Error> {
    let to_clearing = sqlx::query(
        "UPDATE orders o SET escrow_status = 'clearing' \
         FROM events e \
         WHERE e.id = o.event_id AND o.escrow_status = 'held' \
           AND COALESCE(e.ends_at, e.starts_at) IS NOT NULL \
           AND COALESCE(e.ends_at, e.starts_at) < now()",
    )
    .execute(db)
    .await?
    .rows_affected();

    let to_cleared = sqlx::query(
        "UPDATE orders o SET escrow_status = 'cleared' \
         FROM events e \
         WHERE e.id = o.event_id AND o.escrow_status = 'clearing' \
           AND COALESCE(e.ends_at, e.starts_at) < now() - make_interval(days => $1)",
    )
    .bind(clearing_days)
    .execute(db)
    .await?
    .rows_affected();

    Ok((to_clearing, to_cleared))
}

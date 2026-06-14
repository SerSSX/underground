//! Event reminders (TASK-017 M4). A background sweep (driven by the binary's
//! worker loop, see `main.rs`) fans out an `event_reminder` notification to every
//! ticket holder ("going") and saver ("interested") of an event that starts soon.
//! `event_reminders` is a dedup ledger so each (event, user) is reminded once.

use sqlx::PgPool;

/// Run one reminder pass: for every published event starting within the next
/// `window_hours`, insert an `event_reminder` notification for each going/interested
/// user not already reminded. Returns the number of reminders emitted.
pub async fn sweep(db: &PgPool, window_hours: i32) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "WITH due AS ( \
            SELECT id, title FROM events \
            WHERE status = 'published' AND starts_at IS NOT NULL \
              AND starts_at >= now() AND starts_at < now() + make_interval(hours => $1) \
         ), \
         targets AS ( \
            SELECT d.id AS event_id, t.owner_id AS user_id \
            FROM due d JOIN tickets t ON t.event_id = d.id AND t.status = 'active' \
            UNION \
            SELECT d.id AS event_id, s.user_id \
            FROM due d JOIN event_saves s ON s.event_id = d.id \
         ), \
         fresh AS ( \
            INSERT INTO event_reminders (event_id, user_id) \
            SELECT event_id, user_id FROM targets \
            ON CONFLICT DO NOTHING \
            RETURNING event_id, user_id \
         ) \
         INSERT INTO notifications (user_id, kind, title, body, event_id) \
         SELECT f.user_id, 'event_reminder', d.title, \
            CASE WHEN COALESCE(up.locale, 'uk') = 'en' THEN 'Event starts soon' \
                 ELSE 'Подія скоро розпочнеться' END, \
            f.event_id \
         FROM fresh f JOIN due d ON d.id = f.event_id \
         LEFT JOIN user_prefs up ON up.user_id = f.user_id",
    )
    .bind(window_hours)
    .execute(db)
    .await?;
    Ok(result.rows_affected())
}

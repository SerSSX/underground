//! Event reminders (TASK-017 M4). A background sweep (driven by the binary's
//! worker loop, see `main.rs`) fans out an `event_reminder` notification to every
//! ticket holder ("going") and saver ("interested") of an event that starts soon.
//! `event_reminders` is a dedup ledger so each (event, user) is reminded once.
//! Titles copied into notifications are cut to 120 characters (TASK-305 F1).

use sqlx::PgPool;

/// Run one reminder pass: for every published event starting within the next
/// `window_hours`, insert an `event_reminder` notification for each going/interested
/// user not already reminded. Returns the number of reminders emitted.
pub async fn sweep(db: &PgPool, window_hours: i32) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "WITH due AS ( \
            SELECT id, left(title, 120) AS title FROM events \
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
            CASE WHEN COALESCE(up.locale, 'uk') <> 'uk' THEN 'Event starts soon' \
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

/// Run one review-prompt pass (TASK-199-D lifecycle trigger): for every published event that
/// ENDED within the last `recency_days`, insert a `review_prompt` notification for each ticket
/// holder who has neither been prompted before nor already reviewed it. Idempotent via
/// NOT EXISTS dedup (no ledger). Localized per recipient. Returns the number of prompts emitted.
pub async fn review_prompt_sweep(db: &PgPool, recency_days: i32) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "WITH ended AS ( \
            SELECT id, left(title, 120) AS title FROM events \
            WHERE status = 'published' \
              AND COALESCE(ends_at, starts_at + interval '3 hours') < now() \
              AND COALESCE(ends_at, starts_at) > now() - make_interval(days => $1) \
         ), \
         targets AS ( \
            SELECT DISTINCT e.id AS event_id, e.title, t.owner_id AS user_id \
            FROM ended e JOIN tickets t ON t.event_id = e.id AND t.status IN ('active', 'used') \
         ) \
         INSERT INTO notifications (user_id, kind, title, body, event_id) \
         SELECT tg.user_id, 'review_prompt', tg.title, \
            CASE WHEN COALESCE(up.locale, 'uk') <> 'uk' THEN 'How was it? Leave a review' \
                 ELSE 'Як враження? Залиште відгук' END, \
            tg.event_id \
         FROM targets tg \
         LEFT JOIN user_prefs up ON up.user_id = tg.user_id \
         WHERE NOT EXISTS (SELECT 1 FROM notifications n \
                           WHERE n.user_id = tg.user_id AND n.event_id = tg.event_id AND n.kind = 'review_prompt') \
           AND NOT EXISTS (SELECT 1 FROM reviews r WHERE r.event_id = tg.event_id AND r.author_id = tg.user_id)",
    )
    .bind(recency_days)
    .execute(db)
    .await?;
    Ok(result.rows_affected())
}

/// Run one "selling fast" pass (TASK-209 conversion trigger): for every published, on-sale event
/// whose remaining stock has dropped to `threshold_frac` of its total (but is not yet sold out),
/// nudge each user who SAVED it and doesn't already hold a ticket. Once per (user, event) via
/// NOT EXISTS dedup; localized.
///
/// TASK-210 anti-spam rate cap: no user receives more than `daily_cap` `selling_fast` nudges per
/// rolling 24h — freshly-eligible candidates are ranked per user and only the first
/// `daily_cap − (already sent in window)` are inserted. This bounds a single statement (a saver of
/// many low-stock events) *and* successive ticks (the window count sees prior committed rows).
/// Returns the number of nudges emitted.
pub async fn selling_fast_sweep(db: &PgPool, threshold_frac: f64, daily_cap: i64) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "WITH low AS ( \
            SELECT id, left(title, 120) AS title FROM events \
            WHERE status = 'published' AND sales_closed = false \
              AND (starts_at IS NULL OR starts_at >= now()) \
              AND tickets_total > 0 AND tickets_left > 0 \
              AND tickets_left::numeric <= tickets_total::numeric * $1 \
         ), \
         fresh AS ( \
            SELECT DISTINCT l.id AS event_id, l.title, s.user_id \
            FROM low l JOIN event_saves s ON s.event_id = l.id \
            WHERE NOT EXISTS (SELECT 1 FROM notifications n \
                              WHERE n.user_id = s.user_id AND n.event_id = l.id AND n.kind = 'selling_fast') \
              AND NOT EXISTS (SELECT 1 FROM tickets t \
                              WHERE t.event_id = l.id AND t.owner_id = s.user_id AND t.status IN ('active', 'used')) \
         ), \
         budget AS ( \
            SELECT f.event_id, f.title, f.user_id, \
               $2::bigint - COALESCE((SELECT count(*) FROM notifications n \
                    WHERE n.user_id = f.user_id AND n.kind = 'selling_fast' \
                      AND n.created_at > now() - interval '24 hours'), 0) AS remaining, \
               row_number() OVER (PARTITION BY f.user_id ORDER BY f.event_id) AS rn \
            FROM fresh f \
         ) \
         INSERT INTO notifications (user_id, kind, title, body, event_id) \
         SELECT b.user_id, 'selling_fast', b.title, \
            CASE WHEN COALESCE(up.locale, 'uk') <> 'uk' THEN 'Selling fast — few tickets left' \
                 ELSE 'Квитки закінчуються — встигни купити' END, \
            b.event_id \
         FROM budget b \
         LEFT JOIN user_prefs up ON up.user_id = b.user_id \
         WHERE b.rn <= b.remaining",
    )
    .bind(threshold_frac)
    .bind(daily_cap)
    .execute(db)
    .await?;
    Ok(result.rows_affected())
}

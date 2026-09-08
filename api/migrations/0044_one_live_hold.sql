-- REVIEW-4 #2: one live hold per buyer per event, enforced by the DATABASE, not by the request.
-- Two checkouts of one buyer racing each other both saw zero pending rows to supersede and both
-- inserted a hold; nothing in the request path can see the other transaction. The partial unique
-- index makes the second INSERT fail (checkout maps it to 409) and the loser's seat decrement
-- rolls back with it. Idempotent: duplicates are closed first (newest survives; held seats go
-- back), so the index always builds, and IF NOT EXISTS covers a re-run.
WITH dup AS (
    SELECT id, event_id, hold_expires_at IS NOT NULL AS held,
           row_number() OVER (PARTITION BY event_id, buyer_id ORDER BY created_at DESC) AS rn
    FROM orders WHERE status = 'pending'
), closed AS (
    UPDATE orders o
       SET status = 'expired', terminal_event_seen = COALESCE(o.terminal_event_seen, 'superseded')
      FROM dup WHERE o.id = dup.id AND dup.rn > 1
    RETURNING dup.event_id, dup.held
)
UPDATE events e
   SET tickets_left = LEAST(e.tickets_total, e.tickets_left + c.n)
  FROM (SELECT event_id, count(*) FILTER (WHERE held) AS n FROM closed GROUP BY event_id) c
 WHERE e.id = c.event_id AND c.n > 0;

CREATE UNIQUE INDEX IF NOT EXISTS orders_one_live_hold_uidx ON orders (event_id, buyer_id) WHERE status = 'pending';

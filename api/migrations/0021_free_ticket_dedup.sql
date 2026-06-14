-- P1 #71: one ACTIVE free ticket per user per event (prevents unlimited free minting).
-- Scoped to price = 0 so paid events can still sell several tickets to one buyer.

-- Self-heal any pre-existing duplicate active free tickets (keep the earliest per
-- user+event, mark the rest returned) so the unique index below can build on a dirty DB.
-- Seat counts are intentionally left as-is — these duplicates should never have existed.
UPDATE tickets SET status = 'returned'
WHERE id IN (
    SELECT id FROM (
        SELECT id, row_number() OVER (PARTITION BY event_id, owner_id ORDER BY created_at) AS rn
        FROM tickets WHERE status = 'active' AND price = 0
    ) ranked WHERE rn > 1
);

CREATE UNIQUE INDEX tickets_free_active_owner_event_uniq
    ON tickets (event_id, owner_id)
    WHERE status = 'active' AND price = 0;

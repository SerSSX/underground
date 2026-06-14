-- TASK-017 M8: organizer promo codes + event drafts.

CREATE TABLE promo_codes (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id    UUID NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    code        TEXT NOT NULL,
    percent_off INT NOT NULL,                 -- 1..100
    max_uses    INT,                          -- NULL = unlimited
    used_count  INT NOT NULL DEFAULT 0,
    expires_at  TIMESTAMPTZ,
    active      BOOLEAN NOT NULL DEFAULT true,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- A code is unique within an event (case-insensitive).
CREATE UNIQUE INDEX promo_codes_event_code_idx ON promo_codes (event_id, lower(code));

-- Which promo code (if any) a paid order used.
ALTER TABLE orders ADD COLUMN promo_code TEXT;

-- 'draft' is a new events.status value (TEXT column — no schema change). Drafts are
-- hidden from the афіша and not checkout-able until the organizer publishes them.

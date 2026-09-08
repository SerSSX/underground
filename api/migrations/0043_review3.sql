-- REVIEW-3 (2026-09-09).
-- 1. Orders from before the seat hold existed (hold_expires_at IS NULL) never took a seat and can
--    never expire; close them. A payment that still lands goes through the lapsed-hold path in
--    fulfill (take a seat if one is left, else refund_due), and the one-buyer-one-hold rule has
--    nothing hold-less left to miss. Idempotent: re-running matches zero rows.
UPDATE orders SET status = 'expired' WHERE status = 'pending' AND hold_expires_at IS NULL;

-- 2. Forensic trail for webhook deliveries we refused on binding grounds. Their dedup row in
--    webhook_events is rolled back on purpose (so a corrected resend is applied, not dropped);
--    this is where the refusal itself is kept.
CREATE TABLE webhook_refusals (
    id          BIGSERIAL PRIMARY KEY,
    provider    TEXT        NOT NULL,
    event_id    TEXT,
    order_ref   TEXT        NOT NULL,
    reason      TEXT        NOT NULL,
    received_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX webhook_refusals_received_idx ON webhook_refusals (received_at);

-- 3. The rate-event sweep deletes by age alone; the only index was (bucket, created_at).
CREATE INDEX rate_events_created_idx ON rate_events (created_at);

-- TASK-017 M6: live Lemon Squeezy webhooks, escrow clearing window, organizer payouts, T&S.

-- Webhook signing secret (HMAC-SHA256) + how long after an event funds clear to the organizer.
ALTER TABLE payment_settings ADD COLUMN ls_webhook_secret TEXT;
ALTER TABLE payment_settings ADD COLUMN clearing_window_days INT NOT NULL DEFAULT 5;

-- Organizer verification badge (trust & safety).
ALTER TABLE users ADD COLUMN verified BOOLEAN NOT NULL DEFAULT false;

-- Payout ledger: each row is a settlement of cleared escrow to an organizer (one per currency).
CREATE TABLE payouts (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organizer_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    amount       NUMERIC(12, 2) NOT NULL,  -- net of platform commission
    currency     TEXT NOT NULL,
    order_count  INT NOT NULL,
    status       TEXT NOT NULL DEFAULT 'paid',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX payouts_organizer_idx ON payouts (organizer_id, created_at DESC);

-- orders.escrow_status gains 'clearing' (event over, window not yet elapsed); column is TEXT (no migration).
-- Lifecycle: none → held → clearing → cleared → paid_out; held/clearing → frozen (dispute) → refunded/cleared.

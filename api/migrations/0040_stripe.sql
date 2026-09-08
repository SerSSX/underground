-- PAY-1 + PAY-4: make the payment seam load-bearing, and add Stripe as a real provider.

-- 1. Provider credentials. Stripe gets its own columns rather than reusing the `ls_*` ones:
-- both providers stay configured side by side while we migrate off Lemon Squeezy, and a
-- shared column would make "which secret is live right now" unanswerable.
--
-- `stripe_publishable_key` is not a secret (it ships to the browser) so the admin GET returns
-- it; the other two never leave the server — the API exposes only a `*_set` flag.
ALTER TABLE payment_settings
    ADD COLUMN stripe_secret_key      TEXT,
    ADD COLUMN stripe_publishable_key TEXT,
    ADD COLUMN stripe_webhook_secret  TEXT;

-- 2. What the provider said. Until now an order recorded WHICH provider was used but nothing
-- it returned, so a refund had no handle to issue against and reconciliation had nothing to
-- match on.
--
-- `application_fee` is our commission in the order's currency, snapshotted at checkout rather
-- than recomputed: `platform_fee_bps` can change, and a refund six weeks later must return
-- what was actually taken, not what the rate is today.
--
-- `terminal_event_seen` exists because webhook delivery is unordered. Without it a `Refunded`
-- that arrives before its `Paid` leaves the order pending, and the late `Paid` then mints a
-- ticket for money that has already gone back to the buyer.
ALTER TABLE orders
    ADD COLUMN provider_ref        TEXT,
    ADD COLUMN connected_account   TEXT,
    ADD COLUMN application_fee     NUMERIC(12,2) NOT NULL DEFAULT 0,
    ADD COLUMN refunded_amount     NUMERIC(12,2) NOT NULL DEFAULT 0,
    ADD COLUMN terminal_event_seen TEXT;

-- One provider reference belongs to exactly one order. Partial, because most rows have none
-- (free tickets never reach a provider) and NULLs must not collide with each other.
CREATE UNIQUE INDEX orders_provider_ref_uidx ON orders (provider, provider_ref) WHERE provider_ref IS NOT NULL;

-- The escrow sweep and every money dashboard filter on this column and had no index.
CREATE INDEX orders_escrow_idx ON orders (escrow_status);

-- 3. Webhook de-duplication BY PROVIDER EVENT ID.
--
-- The existing path relies on the order's own `pending` status plus a SERIALIZABLE
-- transaction. That stops a double-mint of the same event but says nothing about a replayed
-- delivery of a DIFFERENT event for the same order, and nothing about ordering — providers
-- document delivery as at-least-once and unordered, and require consumers to de-duplicate on
-- the event id. This table is that record.
--
-- The primary key IS the lock: a duplicate delivery loses the race on insert and is
-- acknowledged without re-running the side effect.
CREATE TABLE webhook_events (
    provider    TEXT        NOT NULL,
    event_id    TEXT        NOT NULL,
    kind        TEXT        NOT NULL,
    order_ref   TEXT,
    received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (provider, event_id)
);

-- Read back only when investigating a delivery, and by the retention sweep.
CREATE INDEX webhook_events_received_idx ON webhook_events (received_at);

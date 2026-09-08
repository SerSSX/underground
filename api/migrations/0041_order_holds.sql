-- PAY-3: a seat is taken at checkout, for a bounded time, and a payment can no longer fail
-- AFTER the money has moved.
--
-- Before this, checkout only read `tickets_left`; the decrement happened in `fulfill`, minutes
-- later, once the provider said "paid". Two buyers could both pass the read and both pay for
-- the last seat; the second `fulfill` then returned 409 to the provider's webhook, which every
-- PSP treats as a failed delivery and retries for days. Money taken, no ticket, no refund.

-- 1. The hold. NULL means "this order never reserved a seat" (every order created before
-- this migration), and the expiry worker must skip those: releasing a seat that was never
-- taken would create capacity from nothing.
ALTER TABLE orders ADD COLUMN hold_expires_at TIMESTAMPTZ;

-- Where the buyer pays. Stored so a reloaded payment page can be sent back to the same
-- hosted checkout instead of creating a second one (which the idempotency key would refuse
-- anyway, but the client would have nothing to show).
ALTER TABLE orders ADD COLUMN checkout_url TEXT;

-- The worker's scan. Partial: only pending orders can expire, and they are a small minority.
CREATE INDEX orders_hold_idx ON orders (hold_expires_at) WHERE status = 'pending';

-- `orders.status` is TEXT with no CHECK; values are pending | paid | failed | expired.
-- `orders.escrow_status` gains `refund_due`: paid, but no ticket could be issued.

-- 2. Capacity ceiling. `tickets_left >= 0` existed; `<= tickets_total` did not, so returning a
-- ticket after an organiser lowered capacity manufactured a seat. Clamp first so the
-- constraint cannot fail on a row already over the line.
UPDATE events SET tickets_left = LEAST(tickets_left, tickets_total) WHERE tickets_left > tickets_total;
ALTER TABLE events ADD CONSTRAINT events_left_le_total CHECK (tickets_left <= tickets_total);

-- 3. Refund queue. When a payment lands and no ticket can be issued, the webhook still
-- answers 200 — the provider cannot fix our state by retrying — and the money goes back
-- through here. PAY-3 only enqueues; the executor that talks to the provider is PAY-11.
--
-- `order_id` is UNIQUE: exactly one refund per order, however many deliveries arrive.
CREATE TABLE refund_jobs (
    id              UUID          PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id        UUID          NOT NULL UNIQUE REFERENCES orders(id) ON DELETE CASCADE,
    amount          NUMERIC(12,2) NOT NULL,
    currency        TEXT          NOT NULL,
    reason          TEXT          NOT NULL,
    status          TEXT          NOT NULL DEFAULT 'pending',  -- pending | done | failed
    attempts        INT           NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ   NOT NULL DEFAULT now(),
    provider_ref    TEXT,
    created_at      TIMESTAMPTZ   NOT NULL DEFAULT now()
);

CREATE INDEX refund_jobs_due_idx ON refund_jobs (next_attempt_at) WHERE status = 'pending';

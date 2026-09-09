-- PAY-11: a refund stops being a status flip and becomes a job an executor drains against the
-- provider. 0041 created `refund_jobs` as a placeholder with exactly one row per order, written
-- but never read. A real executor needs partial refunds (several rows for one order), a retry
-- schedule, and a terminal "a human must look at this" state.

-- One order may now be refunded more than once — a partial today, the rest after a policy
-- decision — so uniqueness moves from the order to (order, sequence). Dropping the old
-- constraint is what makes a second row possible at all; the new index is what still stops the
-- SAME refund being queued twice.
ALTER TABLE refund_jobs DROP CONSTRAINT refund_jobs_order_id_key;

ALTER TABLE refund_jobs
    ADD COLUMN seq        INT  NOT NULL DEFAULT 1,
    ADD COLUMN last_error TEXT;

CREATE UNIQUE INDEX refund_jobs_order_seq_uidx ON refund_jobs (order_id, seq);

-- Status vocabulary: queued -> sent -> done, or -> needs_attention when the provider refused
-- for a reason retrying cannot fix (charge already refunded, more than the remaining amount).
-- `sent` is not a formality: for a DIRECT charge the provider accepts a refund it has not yet
-- paid out (the organiser's balance may be short) and settles it later, so a job stays `sent`
-- until a webhook says which way it went. `failed` is deliberately gone from the vocabulary: it
-- read as "we gave up" while nobody was ever told to look.
UPDATE refund_jobs SET status = 'queued' WHERE status = 'pending';
-- And the DEFAULT with them: 0041 defaulted to 'pending', so without this every NEW job would be
-- born in a state the worker does not claim and the amount ceiling does not count — invisible
-- debts that never get paid back.
ALTER TABLE refund_jobs ALTER COLUMN status SET DEFAULT 'queued';

DROP INDEX refund_jobs_due_idx;
-- Covers both states the worker claims: freshly queued, and `sent` whose lease has run out
-- because the provider never called back.
CREATE INDEX refund_jobs_due_idx ON refund_jobs (next_attempt_at) WHERE status IN ('queued', 'sent');

-- A refund of zero or of a negative amount is not a refund; it is a bug reaching the provider.
ALTER TABLE refund_jobs ADD CONSTRAINT refund_jobs_amount_positive CHECK (amount > 0);

-- THE money ceiling, held by the database rather than by whoever remembers to check it. However
-- many jobs, retries, admin actions and policy refunds run against one order — concurrently or
-- not — the total returned can never exceed what the buyer actually paid. Every writer of
-- `refunded_amount` takes the order row FOR UPDATE first; this constraint is what catches the
-- writer that forgets.
ALTER TABLE orders ADD CONSTRAINT orders_refund_within_amount
    CHECK (refunded_amount >= 0 AND refunded_amount <= amount);

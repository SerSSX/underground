-- Payout ops (ADM-7): manual settlement workflow. Payouts can be marked failed (with a reason)
-- and retried (back to pending) by an admin, since payout transport is a manual settlement.
ALTER TABLE payouts ADD COLUMN IF NOT EXISTS failure_reason TEXT;

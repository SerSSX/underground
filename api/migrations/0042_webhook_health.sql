-- Review finding (PAY-4 M9): a wrong webhook secret was indistinguishable from a right one.
--
-- Readiness only ever said "a secret is stored". If the operator pasted the whsec_ of another
-- endpoint, or the container clock drifted past the ±300 s tolerance, every delivery was
-- rejected with 401 — silently, forever, while the checklist stayed green and buyers paid for
-- tickets that never minted. Two counters make that visible: when a delivery last passed
-- verification, and how many have been rejected since the secret was last changed.
ALTER TABLE payment_settings
    ADD COLUMN stripe_last_webhook_at     TIMESTAMPTZ,
    ADD COLUMN stripe_webhook_rejections  BIGINT NOT NULL DEFAULT 0;

-- Moderation audit-trail (structural bet): an append-only log of every privileged admin
-- action (blocks, moderation, escrow moves, KYC decisions, payouts, report resolutions).
-- Gives Trust & Safety an accountable record of who did what, when, and to whom.
CREATE TABLE IF NOT EXISTS admin_actions (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- SET NULL (not CASCADE): the trail must outlive a removed admin account.
    admin_id    UUID REFERENCES users(id) ON DELETE SET NULL,
    action      TEXT NOT NULL,         -- e.g. user_block, event_moderate, escrow_release, kyc_set
    target_type TEXT NOT NULL,         -- user | event | post | review | order | organizer | report
    target_id   UUID,
    detail      JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS admin_actions_created_idx ON admin_actions (created_at DESC);
CREATE INDEX IF NOT EXISTS admin_actions_target_idx ON admin_actions (target_type, target_id);

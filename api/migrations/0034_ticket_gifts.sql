-- TASK-110 / BR-018, BR-022, WF-GiftClaim: gift a ticket to anyone by email — even
-- someone with no account yet. The ticket is locked (`pending_claim`) while a single-use,
-- short-TTL claim token is outstanding; whoever redeems the token first becomes the owner.
CREATE TABLE ticket_gifts (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ticket_id    UUID NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
    from_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    to_email     TEXT NOT NULL,
    -- SHA-256 of the raw claim token; the raw token is returned once to the gifter and
    -- never stored, so a DB leak cannot reconstruct a working claim link.
    token_hash   TEXT NOT NULL UNIQUE,
    status       TEXT NOT NULL DEFAULT 'pending',  -- pending | claimed | expired
    expires_at   TIMESTAMPTZ NOT NULL,
    claimed_by   UUID REFERENCES users(id) ON DELETE SET NULL,
    claimed_at   TIMESTAMPTZ,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX ticket_gifts_ticket_idx ON ticket_gifts (ticket_id);

-- At most one outstanding (pending) gift per ticket — a ticket can only be out for claim once.
CREATE UNIQUE INDEX ticket_gifts_one_pending ON ticket_gifts (ticket_id) WHERE status = 'pending';

-- Tickets: e-tickets with a QR token. Paid flow + escrow land with the LS integration.
CREATE TABLE tickets (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id     UUID NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    owner_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    qr_token     TEXT NOT NULL UNIQUE,
    status       TEXT NOT NULL DEFAULT 'active',
    price        NUMERIC(12, 2) NOT NULL DEFAULT 0,
    validated_at TIMESTAMPTZ,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX tickets_owner_idx ON tickets (owner_id);
CREATE INDEX tickets_event_idx ON tickets (event_id);

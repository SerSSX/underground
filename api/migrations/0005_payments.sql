-- Payment configuration (admin-editable; defaults to test mode) + orders with escrow state.

CREATE TABLE payment_settings (
    id               BOOLEAN PRIMARY KEY DEFAULT TRUE,
    mode             TEXT NOT NULL DEFAULT 'test',     -- test | live
    provider         TEXT NOT NULL DEFAULT 'test',     -- test | lemonsqueezy
    ls_api_key       TEXT,
    ls_store_id      TEXT,
    ls_variant_id    TEXT,
    platform_fee_bps INT NOT NULL DEFAULT 1000,        -- platform commission, basis points (10%)
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT payment_settings_singleton CHECK (id)
);

INSERT INTO payment_settings (id) VALUES (TRUE);

CREATE TABLE orders (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id      UUID NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    buyer_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    amount        NUMERIC(12, 2) NOT NULL,
    currency      TEXT NOT NULL DEFAULT 'UAH',
    status        TEXT NOT NULL DEFAULT 'pending',  -- pending | paid | failed
    escrow_status TEXT NOT NULL DEFAULT 'none',     -- none | held | cleared | refunded | paid_out
    provider      TEXT NOT NULL DEFAULT 'test',
    ticket_id     UUID REFERENCES tickets(id),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX orders_buyer_idx ON orders (buyer_id);
CREATE INDEX orders_event_idx ON orders (event_id);

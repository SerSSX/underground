-- M0 foundation: identity + event catalog core.
-- gen_random_uuid() is built into PostgreSQL 13+.

CREATE TABLE users (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    google_sub  TEXT UNIQUE,
    email       TEXT UNIQUE NOT NULL,
    name        TEXT NOT NULL,
    avatar_url  TEXT,
    cover_url   TEXT,
    bio         TEXT,
    role        TEXT NOT NULL DEFAULT 'USER',
    blocked     BOOLEAN NOT NULL DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE events (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organizer_id   UUID REFERENCES users(id) ON DELETE SET NULL,
    title          TEXT NOT NULL,
    poster_url     TEXT,
    category       TEXT NOT NULL DEFAULT 'Other',
    city           TEXT,
    venue_address  TEXT,
    lat            DOUBLE PRECISION,
    lng            DOUBLE PRECISION,
    starts_at      TIMESTAMPTZ,
    ends_at        TIMESTAMPTZ,
    age_rating     TEXT,
    price          NUMERIC(12, 2) NOT NULL DEFAULT 0,
    currency       TEXT NOT NULL DEFAULT 'UAH',
    tickets_total  INT NOT NULL DEFAULT 0,
    tickets_left   INT NOT NULL DEFAULT 0 CHECK (tickets_left >= 0),
    cost_type      TEXT NOT NULL DEFAULT 'paid',
    status         TEXT NOT NULL DEFAULT 'published',
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX events_city_idx ON events (city);
CREATE INDEX events_category_idx ON events (category);
CREATE INDEX events_starts_at_idx ON events (starts_at);

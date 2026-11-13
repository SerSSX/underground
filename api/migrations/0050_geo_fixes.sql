-- TASK-303 G1 fix batch. Idempotent, like 0048/0049.

-- Item 2: a learned city is one per (name, country), not one per name, and records who learned
-- it. Lookups adopt a learned ('osm') row only for a client whose country code (or point) matches,
-- so the first learner of a name no longer decides it for everyone. Discovery joins with LIMIT 1.
ALTER TABLE cities ADD COLUMN IF NOT EXISTS learned_by UUID REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE cities ADD COLUMN IF NOT EXISTS learned_at TIMESTAMPTZ;
DROP INDEX IF EXISTS cities_lower_name_idx;
CREATE UNIQUE INDEX IF NOT EXISTS cities_name_country_idx ON cities (lower(name), coalesce(country_code, ''));
CREATE INDEX IF NOT EXISTS cities_learned_by_idx ON cities (learned_by, learned_at) WHERE learned_by IS NOT NULL;
-- F12: the nearest-city zone fallback filters a lat/lng box.
CREATE INDEX IF NOT EXISTS cities_lat_lng_idx ON cities (lat, lng);

-- F5/F8: each cached answer carries its own expiry (searches 7 days, reverse 24 h, empty 1 h).
-- Old entries predate the v2 key format and the default names, so they are dropped (a cache).
ALTER TABLE geo_cache ADD COLUMN IF NOT EXISTS expires_at TIMESTAMPTZ NOT NULL DEFAULT now();
DELETE FROM geo_cache WHERE key NOT LIKE 'v2|%';
CREATE INDEX IF NOT EXISTS geo_cache_expires_idx ON geo_cache (expires_at);

-- F6: every `city` result the geocoder returned, one row per (default name, country, ~1 km
-- point), so learning a city is an indexed lookup instead of a scan of every cached answer.
CREATE TABLE IF NOT EXISTS geo_city_seen (
    name_key     TEXT NOT NULL,
    country_code TEXT NOT NULL,
    lat_key      INTEGER NOT NULL,
    lng_key      INTEGER NOT NULL,
    place        JSONB NOT NULL,
    seen_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (name_key, country_code, lat_key, lng_key)
);
CREATE INDEX IF NOT EXISTS geo_city_seen_at_idx ON geo_city_seen (seen_at);

-- Item 7: a prefs row can now exist before onboarding (the language chosen at sign-up), so
-- "onboarded" becomes a column. Every existing row was written by onboarding/prefs: true.
-- New rows default to false; PUT /api/me/prefs sets it true.
ALTER TABLE user_prefs ADD COLUMN IF NOT EXISTS onboarded BOOLEAN NOT NULL DEFAULT true;
ALTER TABLE user_prefs ALTER COLUMN onboarded SET DEFAULT false;
-- F10 / item 2: the home city's country code, for the same-country boost and city lookups.
ALTER TABLE user_prefs ADD COLUMN IF NOT EXISTS country_code TEXT;

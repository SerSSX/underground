-- Password reset (structural bet): single-use, time-boxed reset tokens.
-- Only the SHA-256 hash of the token is stored; the raw token travels by email and is
-- never persisted, so a DB read cannot reset anyone's password.
CREATE TABLE IF NOT EXISTS password_resets (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash  TEXT NOT NULL,
    expires_at  TIMESTAMPTZ NOT NULL,
    used_at     TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Lookups during a reset are by token hash; uniqueness also guards against collisions.
CREATE UNIQUE INDEX IF NOT EXISTS password_resets_token_hash_idx ON password_resets (token_hash);
CREATE INDEX IF NOT EXISTS password_resets_user_idx ON password_resets (user_id);

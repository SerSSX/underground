-- Session invalidation on password reset (review finding): JWTs are stateless, so changing the
-- password alone leaves any previously-issued (possibly stolen) token valid until expiry.
-- Each token now carries an `iat` (issued-at); a token is rejected if it was issued before this
-- watermark. Reset bumps the watermark to now(), instantly cutting off older tokens.
--
-- Default is the epoch (not now()) so the deploy does NOT log out existing valid sessions:
-- their iat is already >= epoch. Only an explicit reset moves the watermark forward.
ALTER TABLE users ADD COLUMN IF NOT EXISTS tokens_valid_after TIMESTAMPTZ NOT NULL DEFAULT to_timestamp(0);

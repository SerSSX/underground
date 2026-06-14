-- P1 #72: case-insensitive email auth + uniqueness.
-- Dirty-prod note: if two accounts already differ only by email case, the unique index
-- below will fail — dedupe those rows first. (Ephemeral test DBs are always clean.)
UPDATE users SET email = lower(email) WHERE email <> lower(email);
ALTER TABLE users DROP CONSTRAINT IF EXISTS users_email_key;
CREATE UNIQUE INDEX users_email_lower_idx ON users (lower(email));

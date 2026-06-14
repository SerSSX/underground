-- Email/password auth: argon2id hash (NULL for Google-only accounts).
ALTER TABLE users ADD COLUMN password_hash TEXT;

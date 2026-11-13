-- TASK-304 review: daily budget for account mail (reset links, login codes). One row per mail slot taken,
-- recipient as a SHA-256 hash of the address (no plain PII); rows older than a day are deleted by the API.
CREATE TABLE mail_sends (
    id      BIGSERIAL PRIMARY KEY,
    to_hash TEXT NOT NULL,
    sent_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX mail_sends_sent_at_idx ON mail_sends (sent_at);
CREATE INDEX mail_sends_to_idx ON mail_sends (to_hash, sent_at);

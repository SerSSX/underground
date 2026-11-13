-- TASK-304 review: budget for account mail (reset links, login codes). One row per mail slot taken; the
-- recipient and the requesting IP are stored as SHA-256 hashes (no plain PII). Rows older than a day are deleted
-- by the API. (Edited before it reached any shared database: ip_hash added in the second review round.)
CREATE TABLE mail_sends (
    id      BIGSERIAL PRIMARY KEY,
    to_hash TEXT NOT NULL,
    ip_hash TEXT NOT NULL DEFAULT '',
    sent_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX mail_sends_sent_at_idx ON mail_sends (sent_at);
CREATE INDEX mail_sends_to_idx ON mail_sends (to_hash, sent_at);
CREATE INDEX mail_sends_ip_idx ON mail_sends (ip_hash, sent_at);

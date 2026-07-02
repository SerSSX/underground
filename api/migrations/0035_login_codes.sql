-- TASK-115 / UC-001, WF-Auth, BR-016/017: passwordless email one-time codes.
-- Only the SHA-256 hash of a 6-digit code is stored; codes are short-lived and single-use
-- (used_at stamped atomically on verify), so a DB read can't be replayed to log in.
CREATE TABLE login_codes (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email      TEXT NOT NULL,
    code_hash  TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at    TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX login_codes_email_idx ON login_codes (email);

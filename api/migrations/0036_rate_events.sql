-- TASK-113 / BR-017: fixed-window rate limiting. Each row is one attempt for a bucket
-- (e.g. "login:<email>", "verify:<email>"); a handler counts recent rows in the window
-- and rejects with 429 once the cap is hit. Cheap to prune (delete old rows) later.
CREATE TABLE rate_events (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    bucket     TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX rate_events_bucket_idx ON rate_events (bucket, created_at);

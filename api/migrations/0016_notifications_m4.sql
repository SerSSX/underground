-- TASK-017 M4: push delivery + social/reminder notifications + telemetry.

-- Device push tokens (FCM). One row per device token; re-registering refreshes it.
CREATE TABLE device_tokens (
    token      TEXT PRIMARY KEY,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    platform   TEXT NOT NULL DEFAULT 'android',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX device_tokens_user_idx ON device_tokens (user_id);

-- Dedup ledger: at most one reminder per (event, user).
CREATE TABLE event_reminders (
    event_id   UUID NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (event_id, user_id)
);

-- Lightweight product telemetry (north-star instrumentation).
CREATE TABLE telemetry_events (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    UUID REFERENCES users(id) ON DELETE SET NULL,
    name       TEXT NOT NULL,
    props      JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX telemetry_events_name_idx ON telemetry_events (name, created_at);

-- Notifications gain an optional actor (who liked/commented/followed) for
-- profile deep-linking, and a push-delivery watermark for the outbox worker.
ALTER TABLE notifications ADD COLUMN actor_id UUID REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE notifications ADD COLUMN pushed_at TIMESTAMPTZ;

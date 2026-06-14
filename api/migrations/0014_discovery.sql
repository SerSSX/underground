-- TASK-017 M1: discovery signals — saved/interested + country for geo ranking.

CREATE TABLE event_saves (
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    event_id   UUID NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, event_id)
);
CREATE INDEX event_saves_event_idx ON event_saves (event_id);

ALTER TABLE events ADD COLUMN country TEXT;

-- TASK-306: a person blocks another person (not the admin ban, which is users.blocked).
-- One row per (blocker, blocked); the reverse direction is its own row.
CREATE TABLE user_blocks (
    blocker_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    blocked_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (blocker_id, blocked_id),
    CHECK (blocker_id <> blocked_id)
);
-- "Is there a block either way between X and Y" probes both columns.
CREATE INDEX user_blocks_blocked_idx ON user_blocks (blocked_id, blocker_id);

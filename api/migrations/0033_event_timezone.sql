-- TASK-102 / BR-020: every event carries an IANA timezone.
-- Times stay stored in UTC (starts_at/ends_at TIMESTAMPTZ); the timezone is the
-- display contract so an event's local time renders correctly for any viewer.
-- Backfill existing rows with the launch-market default; NOT NULL enforces the invariant.
ALTER TABLE events ADD COLUMN timezone TEXT NOT NULL DEFAULT 'Europe/Kyiv';

-- TASK-305 fix round 2. Expand-only: nothing is dropped or rewritten in a way the previous
-- release cannot read, so a rollback to it keeps working against this schema.

-- R1: the display-name rule now lives only in `users::DISPLAY_NAME_SQL` and reads `u.name`
-- («Гість» only for a blank name or «Гість» itself). The generated column `name_is_placeholder`
-- from 0048 is left in place and is no longer read by this release; the previous release still
-- reads it, so it is dropped in a later contract migration, not here. (Postgres 16 cannot change
-- a generation expression in place, and dropping it now would break that rollback.)
--
-- Backfill, once: password-less rows created before TASK-302 (2026-11-12) whose name the old rule
-- matched were named from the email automatically (that sign-up used the email's local part);
-- they become «Гість», so the new rule cannot show part of an email. A password account typed
-- its own name at registration, and rows created since then keep whatever was typed, even a
-- name equal to the email's local part (docs/decisions.md 2026-11-14). Idempotent.
UPDATE users SET name = 'Гість'
WHERE created_at < TIMESTAMPTZ '2026-11-12 00:00:00+00'
  AND password_hash IS NULL
  AND name_is_placeholder IS NOT FALSE
  AND name <> 'Гість';

-- UX7: in-app alerts can point somewhere other than the event page. `link` is a relative web
-- path (`/organizer/<event id>` for a new booking). A place offer stores its gift instead: the
-- claim link is rebuilt from the gift id when the recipient lists their alerts and is only
-- shown while the gift is pending, so no claim token is stored (the 0034 rule: a database leak
-- cannot reconstruct a working claim link).
ALTER TABLE notifications ADD COLUMN IF NOT EXISTS link TEXT;
ALTER TABLE notifications ADD COLUMN IF NOT EXISTS gift_id UUID REFERENCES ticket_gifts(id) ON DELETE CASCADE;

-- TASK-305 round 3: telemetry retention deletes by age, and the anonymous daily cap counts
-- today's rows without a user.
CREATE INDEX IF NOT EXISTS telemetry_events_created_idx ON telemetry_events (created_at);
CREATE INDEX IF NOT EXISTS telemetry_events_anon_day_idx ON telemetry_events (created_at) WHERE user_id IS NULL;

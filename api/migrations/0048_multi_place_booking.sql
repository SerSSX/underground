-- TASK-302 Change C + E (docs/backlog/TASK-302-abuse-cases.md).
-- Numbered 0048 because the unmerged payments branch owns 0040-0047. Idempotent: a dev DB
-- that already applied an earlier copy of this file (as 0040) can run it again safely.

-- CT2: up to 5 places per person per event. The one-active-free-ticket index from 0021 is
-- replaced by a count taken under the event row lock in the handlers (reserve, transfer,
-- claim). It must go in the same deploy as that code: with it, a second place is a 500.
DROP INDEX IF EXISTS tickets_free_active_owner_event_uniq;

-- Supports the holdings count (owner's live places on one event) and the guest list.
CREATE INDEX IF NOT EXISTS tickets_event_owner_live_idx
    ON tickets (event_id, owner_id)
    WHERE status IN ('active', 'pending_claim', 'used');

-- CT8: a display name that is empty, equals the email's local part (email-code and Google
-- sign-up default it to that) or equals the whole email is a placeholder. Organiser-facing
-- responses show a neutral name instead, so they never read the email column at all.
ALTER TABLE users ADD COLUMN IF NOT EXISTS name_is_placeholder BOOLEAN GENERATED ALWAYS AS (
    btrim(name) = ''
    OR lower(btrim(name)) = lower(split_part(email, '@', 1))
    OR lower(btrim(name)) = lower(email)
) STORED;

-- P2: an event hidden or blocked by an admin cannot be re-published by its organiser.
-- `admin_locked` tells an admin hide apart from the organiser's own hide.
ALTER TABLE events ADD COLUMN IF NOT EXISTS admin_locked BOOLEAN NOT NULL DEFAULT false;

-- Backfill in the same deploy as the gate: blocked events, and events whose latest admin
-- moderation action set the status they still have.
UPDATE events e SET admin_locked = true
WHERE NOT e.admin_locked AND (e.status = 'blocked'
   OR (e.status = 'hidden' AND EXISTS (
        SELECT 1 FROM admin_actions a
        WHERE a.target_type = 'event' AND a.target_id = e.id AND a.action = 'event_moderate'
          AND a.detail->>'status' = e.status
          AND a.created_at = (
              SELECT max(a2.created_at) FROM admin_actions a2
              WHERE a2.target_type = 'event' AND a2.target_id = e.id AND a2.action = 'event_moderate'))));

-- P13: rate_events is pruned by age; this index keeps the global prune cheap.
CREATE INDEX IF NOT EXISTS rate_events_created_idx ON rate_events (created_at);

-- Fix batch 1, item 1: password-less sign-up used to store the email local part as the display
-- name, which then showed on public profiles, reviews, chats and gift previews. New accounts get
-- "Гість" (users::create_passwordless_user); existing auto-named ones are reset here. Only
-- password-less accounts are touched: a password account typed its own name.
UPDATE users SET name = 'Гість'
WHERE password_hash IS NULL AND deleted_at IS NULL
  AND lower(btrim(name)) = lower(split_part(email, '@', 1));

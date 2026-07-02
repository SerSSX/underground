-- TASK-199-D / TASK-205: deliver retention notifications (event_reminder, review_prompt) via the
-- transactional email outbox too, closing the retention channel outside the app. `email_sent`
-- watermarks each notification once emailed (idempotent re-send guard, mirrors orders.email_sent).
-- Existing rows are backfilled as sent so a first deploy never emails the historical backlog.
ALTER TABLE notifications ADD COLUMN email_sent BOOLEAN NOT NULL DEFAULT false;
UPDATE notifications SET email_sent = true;

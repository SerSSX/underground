-- TASK-017 M5: per-user locale for server-generated content (notifications).
-- 'uk' (Ukrainian) is the launch default; 'en' is the second supported locale.
ALTER TABLE user_prefs ADD COLUMN locale TEXT NOT NULL DEFAULT 'uk';

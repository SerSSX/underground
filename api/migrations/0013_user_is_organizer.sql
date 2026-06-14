-- TASK-015: self-serve organizer label (purely cosmetic — anyone can create events).
ALTER TABLE users ADD COLUMN is_organizer BOOLEAN NOT NULL DEFAULT false;

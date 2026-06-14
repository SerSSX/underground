-- Event media + organizer admin controls.

ALTER TABLE events
    ADD COLUMN video_url    TEXT,
    ADD COLUMN gallery_urls TEXT[] NOT NULL DEFAULT '{}',
    ADD COLUMN links        TEXT[] NOT NULL DEFAULT '{}',
    ADD COLUMN sales_closed BOOLEAN NOT NULL DEFAULT FALSE;

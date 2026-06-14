-- F-10: onboarding preferences (interest categories + home city).

CREATE TABLE user_prefs (
    user_id    UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    categories TEXT[] NOT NULL DEFAULT '{}',
    city       TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

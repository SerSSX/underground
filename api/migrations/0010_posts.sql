-- Wall posts: manual posts, auto event announcements, auto review reposts.

CREATE TABLE posts (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    author_id  UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind       TEXT NOT NULL DEFAULT 'text', -- text | event_share | review
    text       TEXT,
    event_id   UUID REFERENCES events(id) ON DELETE CASCADE,
    rating     INT, -- only for review posts
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX posts_author_idx ON posts (author_id, created_at DESC);

CREATE TABLE post_likes (
    post_id    UUID NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (post_id, user_id)
);

CREATE TABLE post_comments (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    post_id    UUID NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    author_id  UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    text       TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX post_comments_post_idx ON post_comments (post_id, created_at);

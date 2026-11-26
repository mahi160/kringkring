-- A subscription belongs to one signed-in device; logging out removes it.
CREATE TABLE push_subscriptions (
    endpoint TEXT NOT NULL PRIMARY KEY,
    session_token_hash BLOB NOT NULL REFERENCES sessions (token_hash) ON DELETE CASCADE,
    p256dh TEXT NOT NULL,
    auth TEXT NOT NULL,
    -- The device's language at subscribe time, which may override the user's default.
    language TEXT NOT NULL CHECK (language IN ('en', 'bn'))
);

CREATE INDEX push_subscriptions_session ON push_subscriptions (session_token_hash);

-- Written in the same transaction as the request change, drained by the push sender.
CREATE TABLE push_outbox (
    id INTEGER PRIMARY KEY,
    request_id INTEGER NOT NULL REFERENCES requests (id) ON DELETE CASCADE,
    event TEXT NOT NULL CHECK (event IN ('new', 'cancelled', 'accepted', 'rejected', 'done', 'expired'))
);

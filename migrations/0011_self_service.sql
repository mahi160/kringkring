-- Set when an administrator chooses the PIN; the user must replace it before using the app.
ALTER TABLE users ADD COLUMN must_change_pin INTEGER NOT NULL DEFAULT 0 CHECK (must_change_pin IN (0, 1));

CREATE TABLE signup_requests (
    id INTEGER PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    pin_hash TEXT NOT NULL,
    default_language TEXT NOT NULL CHECK (default_language IN ('en', 'bn')),
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);

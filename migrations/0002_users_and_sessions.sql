CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    pin_hash TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('employee', 'staff')),
    is_admin INTEGER NOT NULL DEFAULT 0 CHECK (is_admin IN (0, 1)),
    default_language TEXT NOT NULL DEFAULT 'en' CHECK (default_language IN ('en', 'bn')),
    default_location_id INTEGER,
    failed_pins INTEGER NOT NULL DEFAULT 0 CHECK (failed_pins BETWEEN 0 AND 5)
);

CREATE TABLE sessions (
    token_hash BLOB NOT NULL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE
);

CREATE INDEX sessions_user_id ON sessions (user_id);

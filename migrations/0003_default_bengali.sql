-- Change the default without overwriting saved preferences.
ALTER TABLE users RENAME COLUMN default_language TO previous_language;
ALTER TABLE users ADD COLUMN default_language TEXT NOT NULL DEFAULT 'bn'
    CHECK (default_language IN ('en', 'bn'));
UPDATE users SET default_language = previous_language;
ALTER TABLE users DROP COLUMN previous_language;

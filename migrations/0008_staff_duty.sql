ALTER TABLE users ADD COLUMN staff_paused INTEGER NOT NULL DEFAULT 0
    CHECK (staff_paused IN (0, 1));

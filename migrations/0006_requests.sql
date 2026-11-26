CREATE TABLE requests (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NOT NULL REFERENCES users (id),
    item_id INTEGER REFERENCES menu_items (id),
    custom_text TEXT,
    location_id INTEGER NOT NULL REFERENCES locations (id),
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'accepted', 'done', 'rejected', 'cancelled', 'expired')),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
    accepted_at INTEGER,
    accepted_by INTEGER REFERENCES users (id),
    closed_at INTEGER,
    closed_by INTEGER REFERENCES users (id),
    CHECK ((item_id IS NOT NULL AND custom_text IS NULL) OR
           (item_id IS NULL AND custom_text IS NOT NULL AND length(trim(custom_text)) BETWEEN 1 AND 80))
);

CREATE INDEX requests_employee_open ON requests (employee_id, id)
    WHERE status IN ('pending', 'accepted');

CREATE TABLE request_modifiers (
    request_id INTEGER NOT NULL REFERENCES requests (id),
    modifier_id INTEGER NOT NULL REFERENCES modifiers (id),
    PRIMARY KEY (request_id, modifier_id)
);

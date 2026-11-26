CREATE TABLE reject_reasons (
    id INTEGER PRIMARY KEY,
    name_en TEXT NOT NULL,
    name_bn TEXT NOT NULL,
    sort_order INTEGER NOT NULL,
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    CHECK (length(trim(name_en)) > 0 OR length(trim(name_bn)) > 0)
);

CREATE TABLE cancel_reasons (
    id INTEGER PRIMARY KEY,
    name_en TEXT NOT NULL,
    name_bn TEXT NOT NULL,
    sort_order INTEGER NOT NULL,
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    CHECK (length(trim(name_en)) > 0 OR length(trim(name_bn)) > 0)
);

ALTER TABLE requests ADD COLUMN reject_reason_id INTEGER REFERENCES reject_reasons (id);
ALTER TABLE requests ADD COLUMN cancel_reason_id INTEGER REFERENCES cancel_reasons (id);
ALTER TABLE requests ADD COLUMN reason_en TEXT NOT NULL DEFAULT '';
ALTER TABLE requests ADD COLUMN reason_bn TEXT NOT NULL DEFAULT '';
ALTER TABLE requests ADD COLUMN cancel_after_ms INTEGER;

CREATE INDEX requests_cancel_deadline ON requests (cancel_after_ms)
    WHERE status = 'pending' AND cancel_after_ms IS NOT NULL;

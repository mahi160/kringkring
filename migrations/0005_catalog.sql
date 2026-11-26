CREATE TABLE menu_items (
    id INTEGER PRIMARY KEY,
    name_en TEXT NOT NULL,
    name_bn TEXT NOT NULL,
    sort_order INTEGER NOT NULL,
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    color TEXT NOT NULL,
    icon TEXT NOT NULL,
    CHECK (length(trim(name_en)) > 0 OR length(trim(name_bn)) > 0)
);

CREATE TABLE modifiers (
    id INTEGER PRIMARY KEY,
    name_en TEXT NOT NULL,
    name_bn TEXT NOT NULL,
    sort_order INTEGER NOT NULL,
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    CHECK (length(trim(name_en)) > 0 OR length(trim(name_bn)) > 0)
);

CREATE TABLE locations (
    id INTEGER PRIMARY KEY,
    name_en TEXT NOT NULL,
    name_bn TEXT NOT NULL,
    sort_order INTEGER NOT NULL,
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    CHECK (length(trim(name_en)) > 0 OR length(trim(name_bn)) > 0)
);

CREATE TABLE menu_item_modifiers (
    item_id INTEGER NOT NULL REFERENCES menu_items (id),
    modifier_id INTEGER NOT NULL REFERENCES modifiers (id),
    PRIMARY KEY (item_id, modifier_id)
);

ALTER TABLE users RENAME COLUMN default_location_id TO previous_location_id;
ALTER TABLE users ADD COLUMN default_location_id INTEGER REFERENCES locations (id);
ALTER TABLE users DROP COLUMN previous_location_id;

INSERT INTO menu_items (id, name_en, name_bn, sort_order, color, icon) VALUES
    (1, 'Tea', 'চা', 10, 'blue', 'tea'),
    (2, 'Raw tea', 'রং চা', 20, 'brick', 'tea'),
    (3, 'Black coffee', 'ব্ল্যাক কফি', 30, 'ochre', 'coffee'),
    (4, 'Water', 'পানি', 40, 'teal', 'water');

INSERT INTO modifiers (id, name_en, name_bn, sort_order) VALUES
    (1, 'Sugar', 'চিনি', 10),
    (2, 'Milk', 'দুধ', 20);

INSERT INTO menu_item_modifiers (item_id, modifier_id) VALUES (1, 1), (1, 2), (2, 1), (3, 1);

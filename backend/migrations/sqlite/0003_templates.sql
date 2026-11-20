-- Milestone 4: zone templates.
CREATE TABLE templates (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL
);

CREATE TABLE template_records (
    id INTEGER PRIMARY KEY,
    template_id INTEGER NOT NULL REFERENCES templates(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    type TEXT NOT NULL,
    ttl INTEGER NOT NULL,
    content TEXT NOT NULL
);
CREATE INDEX template_records_template ON template_records (template_id);

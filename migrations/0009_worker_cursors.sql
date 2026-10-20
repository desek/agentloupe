-- @agents-index Makes the worker_cursors table: the last exchange rowid that each background worker processed.
CREATE TABLE worker_cursors (
    name       TEXT PRIMARY KEY,
    last_id    INTEGER NOT NULL,
    updated_at TEXT NOT NULL
);

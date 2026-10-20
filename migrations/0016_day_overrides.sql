-- @agents-index Makes the day_overrides table: the edited summary and the deletion mark of the operator for one day.
CREATE TABLE day_overrides (
    day        TEXT PRIMARY KEY,
    summary    TEXT,
    deleted_at TEXT,
    updated_at TEXT NOT NULL
);

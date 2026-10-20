-- @agents-index Makes the day_summaries table: one scrubbed paragraph and its status for each day in LOUPE_TZ.
CREATE TABLE day_summaries (
    day           TEXT PRIMARY KEY,
    status        TEXT NOT NULL,
    line          TEXT,
    session_lines INTEGER NOT NULL,
    built_at      TEXT NOT NULL
);

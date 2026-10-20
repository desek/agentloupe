-- @agents-index Makes the session_summaries table: one scrubbed line and its status for each closed session segment.
CREATE TABLE session_summaries (
    id              INTEGER PRIMARY KEY,
    session_id      INTEGER NOT NULL UNIQUE REFERENCES sessions (id),
    status          TEXT NOT NULL,
    line            TEXT,
    model           TEXT,
    attempts        INTEGER NOT NULL,
    last_error_kind TEXT,
    next_attempt_at TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);
CREATE INDEX session_summaries_status ON session_summaries (status, next_attempt_at);

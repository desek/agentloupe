-- @agents-index Makes session_full_summaries and session_day_summaries: one scrubbed paragraph and its status per session and day.
CREATE TABLE session_full_summaries (
    id              INTEGER PRIMARY KEY,
    session_key     TEXT NOT NULL,
    day             TEXT NOT NULL,
    status          TEXT NOT NULL,
    text            TEXT,
    model           TEXT,
    attempts        INTEGER NOT NULL,
    last_error_kind TEXT,
    next_attempt_at TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    UNIQUE (session_key, day)
);
CREATE INDEX session_full_summaries_status ON session_full_summaries (status, next_attempt_at);
CREATE TABLE session_day_summaries (
    id              INTEGER PRIMARY KEY,
    session_key     TEXT NOT NULL,
    day             TEXT NOT NULL,
    status          TEXT NOT NULL,
    text            TEXT,
    model           TEXT,
    attempts        INTEGER NOT NULL,
    last_error_kind TEXT,
    next_attempt_at TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    UNIQUE (session_key, day)
);
CREATE INDEX session_day_summaries_status ON session_day_summaries (status, next_attempt_at);
CREATE INDEX session_day_summaries_day ON session_day_summaries (day);
CREATE INDEX sessions_key_hash ON sessions (key_hash);

-- @agents-index Makes the exchanges table: one row per forwarded request, with redacted headers and sealed bodies.
CREATE TABLE exchanges (
    id                 TEXT PRIMARY KEY,
    started_at         TEXT NOT NULL,
    ended_at           TEXT NOT NULL,
    ttfb_ms            INTEGER,
    agent              TEXT NOT NULL,
    upstream           TEXT NOT NULL,
    method             TEXT NOT NULL,
    path               TEXT NOT NULL,
    query              TEXT NOT NULL,
    status             INTEGER NOT NULL,
    request_size       INTEGER NOT NULL,
    response_size      INTEGER NOT NULL,
    request_headers    TEXT NOT NULL,
    response_headers   TEXT NOT NULL,
    request_body       BLOB NOT NULL,
    response_body      BLOB NOT NULL,
    request_truncated  INTEGER NOT NULL,
    response_truncated INTEGER NOT NULL,
    credential_hash    TEXT,
    credential_source  TEXT,
    account_id_hash    TEXT,
    account_id_source  TEXT,
    outcome            TEXT NOT NULL,
    error              TEXT
);
CREATE INDEX exchanges_started_at ON exchanges (started_at);

-- @agents-index Rebuilds exchanges with nullable bodies and adds parsed_at and bodies_deleted_at for the raw-body retention.
-- SQLite cannot drop NOT NULL in place, so the table is copied; calls keep their reference by name.
CREATE TABLE exchanges_new (
    id                  TEXT PRIMARY KEY,
    started_at          TEXT NOT NULL,
    ended_at            TEXT NOT NULL,
    ttfb_ms             INTEGER,
    agent               TEXT NOT NULL,
    upstream            TEXT NOT NULL,
    method              TEXT NOT NULL,
    path                TEXT NOT NULL,
    query               TEXT NOT NULL,
    status              INTEGER NOT NULL,
    request_size        INTEGER NOT NULL,
    response_size       INTEGER NOT NULL,
    request_headers     TEXT NOT NULL,
    response_headers    TEXT NOT NULL,
    request_body        BLOB,
    response_body       BLOB,
    request_truncated   INTEGER NOT NULL,
    response_truncated  INTEGER NOT NULL,
    credential_hash     TEXT,
    credential_source   TEXT,
    account_id_hash     TEXT,
    account_id_source   TEXT,
    outcome             TEXT NOT NULL,
    error               TEXT,
    proxy_token_id      INTEGER REFERENCES proxy_tokens (id),
    session_hint_hash   TEXT,
    session_hint_source TEXT,
    parse_outcome       TEXT,
    parsed_at           TEXT,
    bodies_deleted_at   TEXT
);
INSERT INTO exchanges_new (rowid, id, started_at, ended_at, ttfb_ms, agent, upstream, method, path, query, status, request_size, response_size, request_headers, response_headers, request_body, response_body, request_truncated, response_truncated, credential_hash, credential_source, account_id_hash, account_id_source, outcome, error, proxy_token_id, session_hint_hash, session_hint_source, parse_outcome, parsed_at)
SELECT rowid, id, started_at, ended_at, ttfb_ms, agent, upstream, method, path, query, status, request_size, response_size, request_headers, response_headers, request_body, response_body, request_truncated, response_truncated, credential_hash, credential_source, account_id_hash, account_id_source, outcome, error, proxy_token_id, session_hint_hash, session_hint_source, parse_outcome,
       CASE WHEN parse_outcome IS NULL THEN NULL ELSE strftime('%Y-%m-%dT%H:%M:%fZ', 'now') END
FROM exchanges;
DROP TABLE exchanges;
ALTER TABLE exchanges_new RENAME TO exchanges;
CREATE INDEX exchanges_started_at ON exchanges (started_at);
CREATE INDEX exchanges_parse_outcome ON exchanges (parse_outcome, bodies_deleted_at);

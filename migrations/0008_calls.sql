-- @agents-index Makes the calls table: one row per model call found in an exchange, with the five token counts.
CREATE TABLE calls (
    id             INTEGER PRIMARY KEY,
    exchange_id    TEXT NOT NULL REFERENCES exchanges (id),
    call_index     INTEGER NOT NULL,
    started_at     TEXT NOT NULL,
    agent_label    TEXT NOT NULL,
    upstream       TEXT NOT NULL,
    proxy_token_id INTEGER REFERENCES proxy_tokens (id),
    session_id     INTEGER NOT NULL REFERENCES sessions (id),
    account_id     INTEGER REFERENCES accounts (id),
    model          TEXT,
    service_tier   TEXT,
    status         INTEGER NOT NULL,
    input          INTEGER NOT NULL,
    cache_read     INTEGER NOT NULL,
    cache_write_5m INTEGER NOT NULL,
    cache_write_1h INTEGER NOT NULL,
    output         INTEGER NOT NULL,
    UNIQUE (exchange_id, call_index)
);
CREATE INDEX calls_started_at ON calls (started_at);
CREATE INDEX calls_session_id ON calls (session_id);

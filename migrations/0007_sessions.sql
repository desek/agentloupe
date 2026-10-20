-- @agents-index Makes the sessions table: one row per segment of a session scope, with its state and close time.
CREATE TABLE sessions (
    id             INTEGER PRIMARY KEY,
    agent_label    TEXT NOT NULL,
    proxy_token_id INTEGER REFERENCES proxy_tokens (id),
    key_hash       TEXT NOT NULL,
    key_source     TEXT NOT NULL,
    segment        INTEGER NOT NULL,
    started_at     TEXT NOT NULL,
    last_call_at   TEXT NOT NULL,
    closed_at      TEXT,
    state          TEXT NOT NULL,
    call_count     INTEGER NOT NULL,
    account_id     INTEGER REFERENCES accounts (id)
);
CREATE INDEX sessions_scope ON sessions (agent_label, proxy_token_id, key_hash, state);
CREATE INDEX sessions_state_closed_at ON sessions (state, closed_at);

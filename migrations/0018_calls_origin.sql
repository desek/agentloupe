-- @agents-index Rebuilds calls with a nullable exchange_id, the origin of each call (proxy or log), and the unique log key.
CREATE TABLE calls_new (
    id              INTEGER PRIMARY KEY,
    exchange_id     TEXT REFERENCES exchanges (id),
    call_index      INTEGER NOT NULL,
    started_at      TEXT NOT NULL,
    agent_label     TEXT NOT NULL,
    upstream        TEXT NOT NULL,
    proxy_token_id  INTEGER REFERENCES proxy_tokens (id),
    session_id      INTEGER NOT NULL REFERENCES sessions (id),
    account_id      INTEGER REFERENCES accounts (id),
    model           TEXT,
    service_tier    TEXT,
    status          INTEGER NOT NULL,
    input           INTEGER NOT NULL,
    cache_read      INTEGER NOT NULL,
    cache_write_5m  INTEGER NOT NULL,
    cache_write_1h  INTEGER NOT NULL,
    output          INTEGER NOT NULL,
    cost_usd        TEXT,
    price_commit    TEXT,
    unpriced_reason TEXT,
    origin          TEXT NOT NULL DEFAULT 'proxy' CHECK (origin IN ('proxy', 'log')),
    log_key         TEXT UNIQUE,
    UNIQUE (exchange_id, call_index),
    CHECK ((exchange_id IS NULL) <> (log_key IS NULL))
);
INSERT INTO calls_new (id, exchange_id, call_index, started_at, agent_label, upstream, proxy_token_id, session_id, account_id, model, service_tier, status, input, cache_read, cache_write_5m, cache_write_1h, output, cost_usd, price_commit, unpriced_reason)
    SELECT id, exchange_id, call_index, started_at, agent_label, upstream, proxy_token_id, session_id, account_id, model, service_tier, status, input, cache_read, cache_write_5m, cache_write_1h, output, cost_usd, price_commit, unpriced_reason FROM calls;
DROP TABLE calls;
ALTER TABLE calls_new RENAME TO calls;
CREATE INDEX calls_started_at ON calls (started_at);
CREATE INDEX calls_session_id ON calls (session_id);

-- @agents-index Makes the proxy_tokens table: one row per proxy token, with its SHA-256 hash, unique name, tags, and three times.
CREATE TABLE proxy_tokens (
    id           INTEGER PRIMARY KEY,
    name         TEXT NOT NULL UNIQUE,
    token_hash   TEXT NOT NULL UNIQUE,
    tags         TEXT NOT NULL,
    created_at   TEXT NOT NULL,
    last_used_at TEXT,
    revoked_at   TEXT
);

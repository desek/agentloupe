-- @agents-index Makes the accounts table: one row per provider account hash and source, with first and last seen times.
CREATE TABLE accounts (
    id         INTEGER PRIMARY KEY,
    hash       TEXT NOT NULL,
    source     TEXT NOT NULL,
    first_seen TEXT NOT NULL,
    last_seen  TEXT NOT NULL,
    UNIQUE (hash, source)
);

-- @agents-index Makes the days table: one day record per day in LOUPE_TZ, with its content hash, revision, and review state.
CREATE TABLE days (
    day          TEXT PRIMARY KEY,
    record       TEXT,
    content_hash TEXT,
    revision     INTEGER NOT NULL,
    built_at     TEXT,
    state        TEXT NOT NULL,
    published_at TEXT
);
CREATE INDEX days_state_day ON days (state, day);

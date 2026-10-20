-- @agents-index Makes the agg_queue table: the days to build again, one row per day with the latest reason.
CREATE TABLE agg_queue (
    day       TEXT PRIMARY KEY,
    reason    TEXT NOT NULL,
    queued_at TEXT NOT NULL
);

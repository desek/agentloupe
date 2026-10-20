-- @agents-index Makes the version table that records each applied migration; later product areas add their own tables.
CREATE TABLE schema_version (
    version    INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL
);

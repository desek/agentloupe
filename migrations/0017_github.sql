-- @agents-index Makes the GitHub tables: pull requests by hash, the labels that saw each one, and one fetch row per day and label.
CREATE TABLE github_prs (
    pr_hash    TEXT PRIMARY KEY,
    opened_day TEXT,
    merged_day TEXT,
    additions  INTEGER,
    deletions  INTEGER,
    updated_at TEXT NOT NULL
);
CREATE INDEX github_prs_opened_day ON github_prs (opened_day);
CREATE INDEX github_prs_merged_day ON github_prs (merged_day);
CREATE TABLE github_pr_labels (
    pr_hash TEXT NOT NULL,
    label   TEXT NOT NULL,
    PRIMARY KEY (pr_hash, label)
);
CREATE TABLE github_fetches (
    day        TEXT NOT NULL,
    label      TEXT NOT NULL,
    status     TEXT NOT NULL CHECK (status IN ('ok', 'failed')),
    cause      TEXT,
    fetched_at TEXT NOT NULL,
    PRIMARY KEY (day, label)
);

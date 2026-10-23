-- @agents-index The one table of the telemetry endpoint: per commit, the first and the last UTC date of a start and the count.
CREATE TABLE starts (
  sha TEXT PRIMARY KEY NOT NULL,
  firstseen TEXT NOT NULL,
  lastseen TEXT NOT NULL,
  count INTEGER NOT NULL
);

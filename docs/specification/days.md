<!-- @agents-index Specification of the day record, its storage, the aggregation queue, the publisher, loupe aggregate, and loupe day. -->
# Days

This file describes how `loupe` builds one record for each day with activity, stores it, and publishes it. `api.md` describes the routes that serve the records.

## Day boundaries

A day is a calendar date in the time zone `LOUPE_TZ`. The default is `Europe/Stockholm`. A day starts at 00:00 local time. It ends before 00:00 of the next local date. Thus a day with a change for daylight saving time has 23 or 25 hours.

The binary carries its own time zone database, so the container works with no `/usr/share/zoneinfo`. The binary uses the `chrono-tz` crate, version 0.10. A read of the installed source of `chrono-tz` 0.10.4 confirmed that its build script compiles the time zone files of its `tz` directory into the binary.

An invalid `LOUPE_TZ` stops `loupe serve` and `loupe aggregate` at start. The actionable error names the variable, gives the example value `Europe/Stockholm`, and tells the operator to verify with `loupe config show`.

A call belongs to the day of its start time in `LOUPE_TZ`. A segment counts on each day in `LOUPE_TZ` that has one or more of its calls. Thus a segment that continues after midnight counts on each of those days. A subagent session is its own segment, so it counts as one session. The session line of `summaries.md` keeps its own rule.

## Day record

The day builder reads only the calls, the sessions, the day summaries, the day overrides, and the GitHub tables of `github.md`. It also takes the configured subscriptions as an input. It reads no clock, so the same rows give the same record. The record has exactly these top-level fields, in this order:

| Field | Value |
|---|---|
| `day` | The date, `YYYY-MM-DD`. |
| `input_total` | `input` plus `cache_read` plus `cache_write`. |
| `input` | The sum of the input tokens of the calls of the day. |
| `cache_read` | The sum of the tokens read from the cache. |
| `cache_write` | The sum of the tokens written to the 5-minute cache and to the 1-hour cache. |
| `output` | The sum of the output tokens. |
| `calls` | The number of calls of the day. |
| `sessions` | The number of distinct segments with one or more calls on the day. |
| `cost_usd` | The exact decimal sum of the cost of the priced calls, rounded half to even to 4 fraction digits, as a JSON number. `0` when no call has a price. |
| `unpriced_calls` | The number of calls of the day with no cost. |
| `subscription_cost_usd` | The subscription cost of the day from `cost.md`, rounded half to even to 4 fraction digits, as a JSON number. `0` when no subscription is configured. It never holds a subscription name. |
| `prs_opened` | The number of pull requests opened on the day, from `github.md`. `null` when no GitHub token is set. |
| `prs_merged` | The number of pull requests merged on the day. `null` when no GitHub token is set. |
| `lines_added` | The sum of the added lines of the pull requests merged on the day. `null` when no GitHub token is set. |
| `lines_removed` | The sum of the removed lines of the pull requests merged on the day. `null` when no GitHub token is set. |
| `summary` | The text of the day, or `null`. |
| `detail` | An object with `by_agent` and `by_model`, and `github` with `failed_tokens` when a GitHub token is set. |

The token fields are integer sums. For example, an input of 21000, a cache read of 1250000, and a cache write of 12200 give an `input_total` of 1283200. The four GitHub fields keep their keys, so the shape of the record does not change when the GitHub counts land.

A change of a subscription variable changes only the records that the builder makes after the change. Thus a stored value can differ from the envelope sum of `api.md` until `loupe aggregate --date D` builds the day again.

### Summary

The source of `summary` is in this order:

1. The summary of the day override, when one exists.
2. Else the day line of `summaries.md`, when its status is `done` or `fallback`.
3. Else `null`.

Before the builder reads the day line, it asks the summary area to build the day line of the day. If the answer is "not ready", the build continues. The builder writes the record with the current summary state and puts the day in the aggregation queue with the reason `summary_not_ready`.

### Detail

`by_agent` is an object keyed by agent label, for example `claude-code` or `pi`. Each value holds `input_total`, `input`, `cache_read`, `cache_write`, `output`, `calls`, `sessions`, `cost_usd`, and `unpriced_calls` for the calls and segments of that agent label. The rules of the top-level fields apply.

`by_model` is an object keyed by the model name of the call. Each value holds `input_total`, `input`, `cache_read`, `cache_write`, `output`, `calls`, `cost_usd`, and `unpriced_calls`. It holds no `sessions`, because one segment can use more than one model.

The keys of `by_agent` and `by_model` are sorted in byte order. Thus two builds of the same rows give the same bytes and the same content hash.

## Activity

A day gets a record only when it has activity: one or more calls, or GitHub activity that an activity source reports. A day with no activity gets no record. The API returns only the days with a record, and the dashboard shows a day with no record as zero. A subscription cost alone does not give a day activity.

An activity source has two functions. The first function tells if a day has activity. The second function gives the oldest day with activity, or none. One source file defines this interface. Two sources are registered. The calls source reads only the calls. The GitHub source of `github.md` reads only the pull request table. A day has activity when one or more registered sources tell that it has activity. The GitHub source registers through the same interface, with no change to the publisher.

A source that gets new data for a day puts that day in the aggregation queue with a reason that names the source. The GitHub source uses the reason `github`.

## Privacy

Model names, agent labels, and exact token counts for each day are public. The record never holds a count of provider accounts, a proxy token ID, name, or tag, an account ID or hash, a session ID, a hash of a session key, an upstream base URL, a credential, a price commit, an unpriced reason, or a body.

One source file holds the allowlist of the field names of the record, at each level except the keys of `by_agent` and `by_model`. The allowlist also names the fields of the envelope of `api.md`, `subscription_cost_usd` included. A test fails if a built record holds a field name that is not in the allowlist.

## Storage

The `days` table holds one row for each day:

| Column | Holds |
|---|---|
| `day` | The date, `YYYY-MM-DD`, the primary key. |
| `record` | The JSON text of the record. Null in a tombstone. |
| `content_hash` | The SHA-256 of `record`, lowercase hex. Null in a tombstone. |
| `revision` | 1 for the first write. 0 in a tombstone that has no build. |
| `built_at` | The time of the last write with a new content hash. |
| `state` | `pending_review`, `published`, or `deleted`. |
| `published_at` | The first time that the state became `published`. Later writes do not change it. Null before. |

A write of a record is an upsert. If the new content hash is equal to the stored one, the write does not change `revision` or `built_at`. Else the write adds 1 to `revision` and sets `built_at`. A rebuild does not change `state`.

The `agg_queue` table holds the days to build again. Its columns are `day` (the primary key), `reason`, and `queued_at`. A second put of the same day keeps one row and updates `reason` and `queued_at`.

The `day_overrides` table holds the changes of the operator. Its columns are `day` (the primary key), `summary` (null when no edit), `deleted_at` (null when the day is not deleted), and `updated_at`. Only the `loupe day` commands write it.

## Queue writers

| Writer | Reason | When |
|---|---|---|
| The parser worker | `call` | For the day of each new call, in the same transaction as the call row. |
| The idle step and the close on a gap | `session_closed` | For each day of each closed segment, in the same transaction as the close. |
| The summary worker | `summary_final` | For the day of a segment when the segment gets a final summary status, in the same transaction. |
| The day builder | `summary_not_ready` | When the day line is not ready. |
| An activity source | the name of the source | When the source gets new data for a day. |

A queue write adds no network call and no lock that the proxy waits on.

## Publisher

`loupe serve` starts one publisher worker. It wakes once at start and then at an interval of `LOUPE_PUBLISH_INTERVAL_SECS` seconds. The default is 60. The worker reads the time through one injected clock, so tests can set the time.

A day D is due when the local time in `LOUPE_TZ` is at or after `LOUPE_PUBLISH_AT` on the date D plus one. The default of `LOUPE_PUBLISH_AT` is `01:00`.

On each wake, the worker does these steps:

1. It finds the oldest first day with activity of all registered sources.
2. From that day, it builds and writes each due day that has activity and no `days` row, oldest first. Thus the worker catches up each day that it missed while the container was stopped, and a restart repeats no work.
3. It builds again each queued day that has a `days` row. It deletes the queue row in the same transaction as the write, unless the build puts the day back with `summary_not_ready`.

A first write sets the state to `published` when `LOUPE_DAY_REVIEW` is `auto`, and to `pending_review` when it is `manual`. A queued day that is not due stays in the queue and gets no record from the worker. The worker writes it when it is due. A tombstone is a `days` row, so the worker never creates a deleted day again.

A late session updates its day. A segment that starts on day D and closes after the publish of D puts D in the queue. The next wake writes a new revision of D. This late update is silent: the record holds no revision and no mark of an update, and the dashboard shows no mark.

Each publish and each update writes one log line with the day, the revision, the reason, `calls`, `sessions`, and `unpriced_calls`. The line never holds the summary or a hash.

## `loupe aggregate`

`loupe aggregate` builds one day now and writes it with the upsert rule, also when the day is not due. With no `--date`, the day is the previous day in `LOUPE_TZ`. `--date today` selects the current day in `LOUPE_TZ`, so a reader can see a session of today. The API shows that record at once. The publisher builds the day again when it is due, because each later call puts the day in the queue. A first write sets the state with the rule of the publisher.

The command prints `key: value` lines on stdout:

| Key | Value |
|---|---|
| `day` | The date. |
| `calls` | The number of calls. |
| `sessions` | The number of segments. |
| `cost_usd` | The cost. |
| `unpriced_calls` | The number of calls with no cost. |
| `summary` | `present` or `absent`. Never the text. |
| `revision` | The revision after the write. |
| `written` | `true` or `false`. |

`--dry-run` prints the same lines with `written: false`. It writes no `days`, `agg_queue`, or `day_summaries` row. In dry-run, the builder reads the day line as it is and does not ask for a new build of the day line.

A day with no activity prints `calls: 0` and `written: false`, and exits 0. A day with activity and no call is written, with `calls: 0`.

| Exit code | Meaning |
|---|---|
| 0 | Success. |
| 2 | Bad input, for example a `--date` that is not a valid `YYYY-MM-DD` date and not `today`. The actionable error on stderr names the flag and gives an example. |
| 1 | Internal error. |

`loupe aggregate --help` lists each flag and these exit codes.

## Day review

`LOUPE_DAY_REVIEW` is `auto` or `manual`. The default is `auto`. Another value stops start with an actionable error. In `manual`, a new day stays `pending_review` and is not public until the operator approves it.

| State | Meaning |
|---|---|
| `pending_review` | The record exists and is not public. |
| `published` | The record is public. |
| `deleted` | The day is a tombstone and is not public. |

## `loupe day`

Each `loupe day` command takes `--date D` and supports `--dry-run`. Each command prints `key: value` lines: `day`, `state`, `summary` as `present` or `absent`, and `written`. It never prints the summary text. It uses the exit codes of `loupe aggregate`. A bad `--date` exits 2 with an actionable error. A second run of a command gives the same end state.

### `loupe day approve`

| Day state | Result |
|---|---|
| `pending_review` | Sets the state to `published` and sets `published_at`. |
| `published` | Changes nothing. Exit 0. |
| `deleted` | Changes nothing. Exit 2 with an actionable error that names `loupe day restore --date D`. |
| No `days` row | Changes nothing. Exit 2 with an actionable error that names `loupe aggregate --date D`. |

### `loupe day edit --summary TEXT`

The command writes `TEXT` as the override summary, builds the day again, and writes the record. The edit stays after each later rebuild. An empty `TEXT` exits 2. On a day with no `days` row, the command writes only the override, and the next build of the day uses it.

### `loupe day delete`

The command sets the deletion time of the override and the state `deleted`. The `days` row stays as a tombstone, so the publisher and the catch-up never create the day again. A day with no `days` row also gets a tombstone row, with a null record, a null content hash, and revision 0.

### `loupe day restore`

The command builds the day and writes the record with the upsert rule. Then it clears the deletion time of the override and sets the state to `published`. The state is `pending_review` when `LOUPE_DAY_REVIEW` is `manual` and the operator never approved the day. When the day has no activity, the command exits 2 with an actionable error, and the tombstone stays.

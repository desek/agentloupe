<!-- @agents-index Specification of loupe backfill: log sources, discovery, field mapping, session keys, processed sessions, privacy, and summaries of log segments. -->
# Session log backfill

The backfill reads the session logs that an agent wrote on disk before the proxy captured its calls. It adds each call of a log to the same `calls` and `sessions` tables as a proxied call. The proxy, the capture, and `exchanges` do not change.

## Command

`loupe backfill --source claude-code|pi [--path DIR] [--from YYYY-MM-DD] [--to YYYY-MM-DD] [--pi-label LABEL] [--dry-run] [--human]`

| Flag | Behaviour |
|---|---|
| `--source` | Required. The log source: `claude-code` or `pi`. |
| `--path` | The root directory of the logs. The default is `~/.claude/projects` for `claude-code` and `~/.pi/agent/sessions` for `pi`. |
| `--from`, `--to` | Dates in `LOUPE_TZ`, inclusive. They filter the calls by the timestamp of the entry. Without them, the range is all entries. The processed-session rule applies in every range. |
| `--pi-label` | The agent label of the proxied Pi exchanges. The default is `pi`. See "Pi rule". |
| `--dry-run` | Reads the files and prints the same counters. It writes nothing to the database and sends no summary request. |
| `--human` | Prints a table. |

### Exit codes and errors

| Exit code | Cause | Actionable error |
|---|---|---|
| 0 | The run completed, also when it added no call. | None. |
| 2 | `--source` is missing or has an unknown value. | Names the flag and the two valid values. |
| 3 | The directory is missing or unreadable. | Names what failed, the fixes (pass `--path`, check the permissions), and the check (`ls` of the directory). |

An error can print the value of `--path` or the default directory. It never prints a path from inside a log. A missing `LOUPE_ACCOUNT_KEY` fails with the actionable error of the configuration loader.

### Counters

When stdout is not a terminal, the command prints one `key: value` line per counter on stdout, in this order: `files_read`, `entries_read`, `calls_added`, `calls_existing`, `calls_already_captured`, `sessions_already_captured`, `calls_out_of_range`, `entries_skipped`, `sessions_opened`, `days_queued`, `summaries_done`, `summaries_skipped`, `summaries_retry_later`, `summaries_failed`, `summaries_disabled`. Errors go to stderr.

In a dry run, `calls_added` counts the calls that a real run would add, and each summary counter is 0.

### Malformed lines

A malformed line does not stop the run. The backfill counts it in `entries_skipped`. It logs the file index and the line number, never the line or a path from inside the log.

## Log sources and discovery

| Log source | Files that the backfill reads |
|---|---|
| `claude-code` | Each `*.jsonl` file directly under each project directory of the root is a main session file. Each `*.jsonl` file under `<session uuid>/subagents/` is a subagent file. |
| `pi` | Each `*.jsonl` file directly under each directory of the root. The backfill reads no other file under `~/.pi/agent`. |

The backfill reads a file as a stream of lines, in line order. It keeps at most one file of entries in memory. Then it sorts all calls of the run by timestamp before the session step, so the segments do not depend on the order of the files.

## Claude Code call mapping

A call is one assistant message: the lines with `type: assistant` and a `message.usage` object, grouped by `message.id`. When more than one line has the same `message.id`, the backfill takes the usage of the last line of the group and adds one call only.

| Column | Source |
|---|---|
| `input` | `input_tokens` |
| `cache_read` | `cache_read_input_tokens` |
| `cache_write_5m` | `cache_creation.ephemeral_5m_input_tokens`; the whole `cache_creation_input_tokens` when the split is absent |
| `cache_write_1h` | `cache_creation.ephemeral_1h_input_tokens`; 0 when the split is absent |
| `output` | `output_tokens` |
| `model` | `message.model` |
| `service_tier` | `usage.service_tier` |
| `started_at` | The `timestamp` of the first line of the `message.id` group. |
| `upstream` | `anthropic` |

A message with the model `<synthetic>`, or with all five counts at 0, makes no call.

## Pi call mapping

A call is one of these entries:

1. An entry of `type: message` with `message.role: assistant` and a `message.usage` object.
2. An entry of `type: usage`.
3. A `compaction` or `branch_summary` entry that has a `usage` field.

| Column | Source |
|---|---|
| `input` | `input` |
| `cache_read` | `cacheRead` |
| `cache_write_1h` | `cacheWrite1h`; 0 when absent |
| `cache_write_5m` | `cacheWrite` minus `cache_write_1h` when `cacheWrite1h` is present, else `cacheWrite` |
| `output` | `output` |
| `model` | `model` |
| `upstream` | `provider` |
| `started_at` | The `timestamp` of the entry (ISO 8601). |
| `service_tier` | null |

Verified on the source of Pi 1.3.0: `cacheWrite` includes `cacheWrite1h`. The usage type declares `cacheWrite1h` as the subset of `cacheWrite` that has the 1-hour retention. The Anthropic adapter fills `cacheWrite` from `cache_creation_input_tokens` and `cacheWrite1h` from `cache_creation.ephemeral_1h_input_tokens`. Thus the subtraction gives the 5-minute part.

An entry with no `model` or `provider` of its own, for example a `compaction` entry, takes the model and the provider of the latest assistant message or `model_change` entry before it in the file. An entry with no provider at all makes no call, because `upstream` is required.

The backfill does not store the `cost` field of Pi, and does not use it as the cost of the call.

## Calls of origin log

The `calls` table has two columns for the backfill. See `parser.md`, section "Calls".

| Column | Value |
|---|---|
| `origin` | `proxy` for a call from an exchange, `log` for a call from a log entry. Not null, default `proxy`. |
| `log_key` | The log key of a call of origin `log`, else null. Unique. |

`exchange_id` is null for a call of origin `log`. A check requires exactly one of `exchange_id` and `log_key`. The unique key on (`exchange_id`, `call_index`) stays. Each reader of `calls` that joins `exchanges` uses a left join, so a log call is not dropped.

The log key is the HMAC-SHA256 under `LOUPE_ACCOUNT_KEY` of the log source plus the stable ID of the entry: for Claude Code the `sessionId` and the `message.id`, for Pi the session `id` of the file and the entry `id`. A second run over the same files adds no call and counts the entry in `calls_existing`.

A backfilled call has these values:

| Column | Value |
|---|---|
| `agent_label` | The log source, `claude-code` or `pi`. |
| `proxy_token_id` | null |
| `status` | 200 |
| `origin` | `log` |
| `account_id` | null, because the logs hold no account ID and no credential. See `accounts.md`. |

## Session key

The session key of a backfilled call is the HMAC-SHA256 under `LOUPE_ACCOUNT_KEY` of the session ID of the log. The `key_source` is `log`.

| Log source | Session ID |
|---|---|
| `claude-code`, main file | `sessionId` |
| `claude-code`, subagent file | The file name stem of the subagent file. The key is the HMAC of the stem, never the stem. Thus a subagent is its own session. |
| `pi` | The session `id` of the first line of the file. |

A line with `isSidechain: true` inside a main file takes the key of its file. The scope is (agent label, null proxy token ID, key).

## Session step

The backfill appends each call to its scope with the shared session step of the parser, with the gap rule and `LOUPE_IDLE_SECS`. See `parser.md`, section "Segments". A backfilled call that is older than a closed segment of its scope starts a new segment. It does not open the closed segment again.

At the end of the run, the backfill closes each open backfilled segment whose `last_call_at` plus `LOUPE_IDLE_SECS` is before the start time of the run. It sets `closed_at` to `last_call_at` plus `LOUPE_IDLE_SECS`.

## Processed sessions

Before it adds a call, the backfill computes the session key and looks for a row of `sessions` with that `key_hash` and a `key_source` other than `log`. This row is a session that the proxy capture made. When the row exists, the backfill adds no call of that log session. It counts the session in `sessions_already_captured` and its calls in `calls_already_captured`.

For Claude Code, only the key sources `header` (from `x-claude-code-session-id`) and `metadata` (the `session_id` inside `metadata.user_id`) carry the session ID, so only they can match. A `cache_key` or `opening` key cannot match. A subagent file uses the decision of its main session, because the proxied key of a subagent comes from `x-claude-code-agent-id`, and the log has no verified field with the same value.

A log session that the backfill wrote before is not a processed session. The log key prevents a second insert. A log session that started before the proxy and continued after it is skipped as a whole, so its calls before the proxy are not added.

## Pi rule

Pi sends no session header, and its proxied key source is `opening`, so no session ID of a Pi log matches a proxied key. Thus the backfill reads the `started_at` of the oldest exchange whose agent label is the value of `--pi-label`. The agent label is free text from the proxy route, so the flag lets the operator name it.

1. A Pi log session is backfilled only when its last entry is before that time.
2. A Pi log session with an entry at or after that time is already captured.
3. With no exchange of that label, each Pi log session is backfilled. When other exchanges exist, the command prints one warning line on stderr that names `--pi-label` and the agent labels that exist.

Reason: the proxy sees each Pi call after its first proxied Pi exchange. A test on the whole session keeps each session complete in one origin. A match on the opening hash needs a decrypt of each Pi exchange.

## Price and days

A backfilled call gets its cost from the embedded prices with the same rules as a proxied call. See `cost.md`. It is an unpriced call when no price matches.

Each file is one transaction: the calls, the segments, and the queue rows. The backfill puts each day in `LOUPE_TZ` that holds a new call into the aggregation queue in the same transaction, and counts the days in `days_queued`. An interrupted run leaves whole files, and a new run completes the rest. See `days.md`.

## Privacy

The backfill never stores or logs these fields: a prompt, a content block, a tool input or output, a `cwd`, a file path from inside a log, a `gitBranch`, a raw session ID, a raw message ID, a `uuid`, a `requestId`, a `version`, a file name, a directory name (the cwd slug is a path in clear text), or any other log field that is not in the mapping tables above. It stores only the mapped columns, the columns of "Calls of origin log" and "Session key", and the scrubbed summary line. The transcript exists only in memory during the run.

A log line of the backfill holds only counters, a summary status and an error kind, the log source, a file index, and a line number. An error on stderr can also hold the value of `--path` or the default directory, and nothing else from the file system.

## Configuration

The backfill uses `LOUPE_ACCOUNT_KEY`, `LOUPE_IDLE_SECS`, `LOUPE_TZ`, and the summary variables (`LOUPE_SUMMARY_*`, `LOUPE_PUBLIC_PROJECTS`). It adds no variable. See `configuration.md`.

## Summaries of backfilled sessions

Each closed backfilled segment gets a session summary row. Each backfilled session also gets one full session summary and one session day summary for each day that has a call of the session. `summaries.md` gives the rules of the two texts and of their tables.

The transcript comes from the log entries of the session. The transcript of the session for day D holds the entries of the whole log session in line order, from the first entry up to the end of day D in `LOUPE_TZ`. The day transcript holds the entries of day D only. For Claude Code, a subagent file gives the transcript of its own session, and the transcript of a main session does not include its entries.

The transcript follows the rules of `summaries.md`, section "Transcript": the conversation first, the tool inputs and the tool results only when the conversation is shorter than `LOUPE_SUMMARY_MIN_CONVERSATION_CHARS`, the clip of 300 characters with `[clipped]`, and the clip at 24000 characters. The transcript exists only in memory.

A backfilled session uses the same summary path as a proxied session: the request rules, the credentials, the response and retry rules, the scrubber, and the tables. The transcript source is the only difference.

1. The backfill stores no log content and no path, so the backfill run builds the transcripts and sends the summary requests. A run adds the rows of `session_full_summaries` and `session_day_summaries`.
2. The summary worker builds no transcript for a session with calls of origin `log`.
3. A new run of the backfill builds the transcript again for each backfilled row in status `pending` whose time of the next attempt is null or past. This includes a row that `loupe summarize --retry` or `loupe summarize --rebuild-days` set back to `pending`.

The day line reads the session day summaries of backfilled sessions like those of proxied sessions.

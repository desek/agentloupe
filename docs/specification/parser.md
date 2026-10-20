<!-- @agents-index Specification of the parser worker, the call formats, the token columns, the session keys, and the segments. -->
# Parser and sessions

The parser reads the stored exchanges. It finds the model calls from the usage block of each response. It groups the calls into sessions. It never changes an exchange body, and it never stores a raw session ID, cache key, or prompt outside the encrypted bodies.

## Session hint capture

The capture reads the session hint before it redacts the request headers. It reads the first present header of an ordered list of session headers. One source file holds the list. The initial list is `x-claude-code-session-id`, `x-session-id`, `session_id`, `session-id`. The section "Verified sources per agent" can correct the list.

The capture stores the HMAC-SHA256 of the header value under `LOUPE_ACCOUNT_KEY`, as lowercase hex, in `session_hint_hash`. It stores the lowercase header name in `session_hint_source`. Both are null when no listed header is present. See `proxy.md`, section "Exchange record".

The capture keeps every rule of the proxy. It does not delay the stream, it redacts the header value, and it writes no header value to a log line.

## Parser worker

`loupe serve` starts one parser worker. The worker wakes every `LOUPE_PARSE_INTERVAL_SECS` seconds (default `5`). It reads the exchanges with an id above its cursor, in id order, in batches of at most 100.

The table `worker_cursors` holds the cursor: the name, the last id, and the updated time. For each exchange, the insert of its calls, the session changes, and the cursor advance are in one transaction. Thus a restart does not lose a call and does not repeat a call.

The parse is idempotent. The table `calls` has a unique key on (exchange id, call index). A second parse of the same exchange adds no row.

The worker keeps at most one decrypted body pair in memory for each exchange in the batch. It drops the plain bytes after the parse. A log line of the parser holds only the exchange id, the format, the parse outcome, and the counts. It never holds a body, a header value, a session ID, an account ID, or a hash.

## Formats

The worker finds the format from the upstream path and the response `content-type`.

| Format | Path ends with | Body | Source of the usage |
|---|---|---|---|
| Anthropic Messages, JSON | `/v1/messages` | JSON | The `usage` object of the body. |
| Anthropic Messages, SSE | `/v1/messages` | `text/event-stream` | The input counts come from `message_start`. The output count is the last `usage.output_tokens` of a `message_delta`. |
| OpenAI Responses | `/responses` | JSON or SSE | The `response.completed` event, or the top-level `usage`. |
| OpenAI Chat Completions | `/chat/completions` | JSON or SSE | The `usage` object of the body, or of the last chunk that has one. |

## Parse outcomes

The worker records the result of each exchange in `exchanges.parse_outcome`. An exchange with no usage block makes no call, and the worker advances the cursor past it.

| Outcome | Meaning |
|---|---|
| `call` | The worker found one or more calls. |
| `no_usage` | The response has no usage block, for example `count_tokens`, a model list, or an error. |
| `truncated` | The capture truncated the body, so the usage block is not complete. |
| `decrypt_failed` | The worker cannot decrypt a body. |
| `unknown_format` | The path and the content type match no format. |

## Decrypt failure

A decrypt failure does not stop the worker. The worker writes one actionable error line. The line holds the exchange id, the fix (compare `LOUPE_DATA_KEY` with the key of the stored rows), and the check (run `loupe config show`). The worker records `decrypt_failed` and continues.

## Calls

Each row of `calls` holds these columns:

| Column | Value |
|---|---|
| `id` | A unique id. |
| `exchange_id` | The id of the exchange. |
| `call_index` | The position of the call in the exchange, from 0. |
| `started_at` | The start time of the exchange. |
| `agent_label` | The agent label. |
| `upstream` | The upstream name. |
| `proxy_token_id` | The ID of the proxy token, or null. |
| `session_id` | The id of the session segment. |
| `account_id` | The id of the provider account, or null. See `accounts.md`. |
| `model` | The model. |
| `service_tier` | The service tier, or null. |
| `status` | The response status. |
| `input` | The input tokens that are not cached. |
| `cache_read` | The input tokens read from the cache. |
| `cache_write_5m` | The input tokens written to the cache with a 5 minute lifetime. |
| `cache_write_1h` | The input tokens written to the cache with a 1 hour lifetime. |
| `output` | The output tokens. |

The proxy token ID and the account id are independent columns. Thus an analysis can group by token, by account, or by both.

### Token columns

| Column | Anthropic | OpenAI |
|---|---|---|
| `input` | `usage.input_tokens` | The input total minus `cache_read`. |
| `cache_read` | `usage.cache_read_input_tokens` | The cached tokens of the input details. |
| `cache_write_5m` | `usage.cache_creation.ephemeral_5m_input_tokens`. If only `cache_creation_input_tokens` is present, the full value goes here. | 0 |
| `cache_write_1h` | `usage.cache_creation.ephemeral_1h_input_tokens` | 0 |
| `output` | `usage.output_tokens` | The output total. |

### Model and service tier

The model is the `model` field of the response. If it is absent, the model is the `model` field of the request. The service tier is `usage.service_tier` or the top-level `service_tier`. If both are absent, the service tier is null.

## Session key

The session key is the first present source, in this order:

| Order | Source | Value |
|---|---|---|
| 1 | `header` | The session hint of the exchange. |
| 2 | `metadata` | The session ID in `metadata.user_id` of the request body. |
| 3 | `cache_key` | `prompt_cache_key` of the request body. |
| 4 | `opening` | The SHA-256 of the system prompt or `instructions`, plus the first message of the request. |

The worker stores the key as an HMAC-SHA256 under `LOUPE_ACCOUNT_KEY` of the value of the source. It stores the name of the source in `key_source`.

A subagent is a session. The key rule gives a subagent a key that differs from the key of its parent. The section "Verified sources per agent" records which field or header separates the calls of a subagent from the main calls.

## Session scope

The scope of a session is (agent label, proxy token ID, key). Two calls with the same key but a different agent label or a different proxy token ID are in different sessions. A session never holds calls of two proxy tokens.

## Segments

Each row of `sessions` is one segment. A segment is a run of calls in one scope with no gap above `LOUPE_IDLE_SECS`.

| Column | Value |
|---|---|
| `id` | A unique id. |
| `agent_label` | The agent label. |
| `proxy_token_id` | The ID of the proxy token, or null. |
| `key_hash` | The HMAC of the session key. |
| `key_source` | `header`, `metadata`, `cache_key`, or `opening`. |
| `segment` | The number of the segment in its scope, 1 for the first. |
| `started_at` | The start time of the first call. |
| `last_call_at` | The start time of the last call. |
| `closed_at` | The close time, or null while the segment is open. |
| `state` | `open` or `closed`. |
| `call_count` | The number of calls. |
| `account_id` | The account of the first call. |

The worker applies these rules:

1. Join open: a call joins the open segment of its scope.
2. New segment: if no segment is open, the worker starts a new segment with the next segment number for the scope. A resumed conversation after a close starts a new segment. The worker never reopens a closed segment.
3. Close on gap: if the gap from `last_call_at` of the open segment to the `started_at` of the call is more than `LOUPE_IDLE_SECS`, the worker closes the segment with `closed_at` equal to `last_call_at` plus `LOUPE_IDLE_SECS`. Then it starts a new segment for the call.
4. Close on idle: an idle step runs with the parser worker. It closes an open segment only when `last_call_at` plus `LOUPE_IDLE_SECS` is before the parse watermark. It sets `closed_at` to `last_call_at` plus `LOUPE_IDLE_SECS`. The parse watermark is the `started_at` of the oldest exchange that is not parsed, or the current time when no backlog exists. Thus a late parse of a backlog gives the same segments as a live parse.

The worker reads the idle time from configuration. A short value, for example `60`, closes sessions quickly for a test.

The close of a segment is the event that the summary step reads to start its work. An index on (`state`, `closed_at`) lets that step find the closed segments.

A session that spans midnight counts on each day that has a call. The day record applies this rule, and it reads the `started_at` of each call. A segment is not a part of a day.

## Retention

The rows of `calls` and `sessions` stay forever. No step deletes them by age. The deletion of raw bodies after `LOUPE_RAW_RETENTION_HOURS` does not delete a call row or a session row.

## `loupe parse`

`loupe parse --once` runs one parse pass and one idle step. Then it prints one line for each count on stdout, as `key: value`: `exchanges_read`, `calls_added`, `sessions_opened`, `sessions_closed`. It exits with 0. It is safe to run again.

`loupe parse --once --dry-run` reports the same counts and writes nothing.

## Verified sources per agent

The verification captures real requests through the proxy. It records only header names, field paths, and booleans. It never records a header value, a body, or a credential.

| Agent | Version | Session header | `metadata.user_id` shape | `prompt_cache_key` present | Account source | Subagent separation |
|---|---|---|---|---|---|---|
| Claude Code | 2.1.304 | to be verified | to be verified | to be verified | to be verified | to be verified |
| Pi | 1.3.0 | to be verified | to be verified | to be verified | to be verified | to be verified |

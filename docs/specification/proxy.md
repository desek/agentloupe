<!-- @agents-index Specification of the proxy routes, allowlist, forwarding, capture record, redaction, encryption, spill, and failure rules. -->
# Proxy

## Routes

The router gives the reserved routes precedence over the proxy. The reserved routes are `GET /healthz`, `GET /live`, `/`, the static files of the dashboard, and every path under `/v1/`. Until the dashboard exists, `/` returns status 404.

One source file holds the list of reserved first path segments. The list is `healthz`, `v1`, `assets`, `favicon.ico`, `private`, and `live`.

Every other path has the form `/<agent>/<upstream>/<path>`. The route parser returns one typed route: the agent label, the upstream name, the rest of the path, and the query. The parser is one function behind one interface, so a later change can add an optional leading segment without a change to the forwarder or the capture.

A path can start with an optional token segment, as `/<token>/<agent>/<upstream>/<path>`. A first segment that starts with `lpt_` is the presented token, and the route carries it. The token gate checks it before the forwarder. See `proxy.tokens.md`.

## Names

The agent label and the upstream name must match `^[a-z0-9][a-z0-9-]{0,31}$`. A name in the reserved list is not a valid agent label or upstream name.

## Allowlist

The file `upstreams.toml` at the root of the repository is the allowlist. The container image also holds it. Each entry is a table that has the upstream name as its key. The table holds exactly two keys: `base_url` and `litellm_provider`. The provider selects the prices of the upstream. See `cost.md`. The file holds at least these entries:

| Upstream | `base_url` | `litellm_provider` |
|---|---|---|
| `anthropic` | `https://api.anthropic.com` | `anthropic` |
| `openai` | `https://api.openai.com` | `openai` |
| `chatgpt` | `https://chatgpt.com` | `openai` |

An entry has this shape:

```toml
[anthropic]
base_url = "https://api.anthropic.com"
litellm_provider = "anthropic"
```

An inline table is the same TOML value, and the loader also accepts it:

```toml
anthropic = { base_url = "https://api.anthropic.com", litellm_provider = "anthropic" }
```

`LOUPE_UPSTREAMS_FILE` overrides the path of the file. `loupe config show` prints the path with its source.

`loupe serve` loads and validates the file before it opens the port. The start stops with an actionable error if one of these is true:

1. The file is missing.
2. The file is not valid TOML.
3. A name does not match the name rule.
4. A base URL is not `https`. The only exception is a host of `127.0.0.1` or `localhost`, which the tests use.
5. An entry is a string, for example `anthropic = "https://api.anthropic.com"`. This is the old shape.
6. An entry has no `base_url` or no `litellm_provider`.
7. An entry has a key that is not `base_url` or `litellm_provider`.
8. A `litellm_provider` is not a non-empty string.

The error names the file, the entry, the key, the expected shape, the fix, and the check, which is `loupe config show`.

A request for an upstream name that is not in the allowlist gets status 404. The JSON error body names the unknown upstream, lists the known names, and points to `upstreams.toml`. The proxy does not forward this request and stores no exchange for it.

## Forward rules

The forwarder sends the request to `<base URL><rest of the path>?<query>`. It keeps the method, the path bytes, the query bytes, the headers, and the body bytes. It changes the headers in these ways only:

1. It removes `accept-encoding`.
2. It removes the hop-by-hop headers: `connection`, `keep-alive`, `proxy-connection`, `te`, `trailer`, `transfer-encoding`, `upgrade`, and each header that `connection` names.
3. It sets `host` to the host of the base URL.
4. If the request has no `accept` header, the HTTP client of the forwarder adds `accept: */*`. The client library adds this default and gives no way to remove it. A request that has an `accept` header is not changed.

Because the forwarder removes `upgrade`, the proxy does not support a WebSocket request.

The forwarder streams the request body to the upstream and the response body to the client, one chunk at a time. It never holds the full body in a buffer. A server-sent events response (`text/event-stream`) reaches the client as each chunk arrives.

The forwarder returns the upstream status code and the upstream response headers with no change, minus the hop-by-hop headers.

## Timeouts

| Phase | Default | Override |
|---|---|---|
| Connect and TLS | 10 seconds | `LOUPE_UPSTREAM_CONNECT_TIMEOUT_SECS` |
| End of the request send to the response headers | 120 seconds | `LOUPE_UPSTREAM_HEADERS_TIMEOUT_SECS` |

A slow first byte that arrives before the timeout passes with no change. After the response headers arrive, no timeout applies to the response body, because a model can stream for a long time.

If the proxy cannot reach the upstream, it returns status 502. The causes are a DNS failure, a connect failure, a TLS failure, or a timeout before the response headers. The JSON error body names the upstream and the cause. The proxy stores an exchange with the status 502, an error field, and the outcome `upstream_unreachable`.

## Exchange record

The proxy stores one exchange for each forwarded request in the table `exchanges`. A migration adds the table. Each row holds these columns:

| Column | Value |
|---|---|
| `id` | A unique id. |
| `started_at` | The start time in UTC, with millisecond resolution. |
| `ended_at` | The end time in UTC, with millisecond resolution. |
| `ttfb_ms` | The time to the first response byte, in milliseconds. |
| `agent` | The agent label. |
| `upstream` | The upstream name. |
| `method` | The request method. |
| `path` | The rest of the path. It never holds the token segment. |
| `query` | The query. |
| `status` | The response status. |
| `proxy_token_id` | The ID of the proxy token that passed the token gate, or null in no-token mode. See `proxy.tokens.md`. |
| `request_size` | The size of the request body in bytes, as forwarded. |
| `response_size` | The size of the response body in bytes, as forwarded. |
| `request_headers` | The redacted request headers, as JSON. |
| `response_headers` | The redacted response headers, as JSON. |
| `request_body` | The encrypted request body. |
| `response_body` | The encrypted response body. |
| `request_truncated` | True if the capture truncated the request body. |
| `response_truncated` | True if the capture truncated the response body. |
| `credential_hash` | The credential hash, or null. |
| `credential_source` | `authorization` or `x-api-key`, or null. |
| `account_id_hash` | The account ID hash, or null. |
| `account_id_source` | The lowercase name of the account ID header, or null. |
| `session_hint_hash` | The HMAC of the first present session header, or null. See `parser.md`, section "Session hint capture". |
| `session_hint_source` | The lowercase name of that session header, or null. |
| `parse_outcome` | The result of the parse, or null before the parse. See `parser.md`, section "Parse outcomes". |
| `parsed_at` | The parse time in UTC, or null before the parse. The parser sets it. |
| `bodies_deleted_at` | The time in UTC when the raw-body retention set the two body columns to null, or null. See `summaries.md`, section "Raw-body retention". |
| `outcome` | `complete`, `client_aborted`, `upstream_aborted`, or `upstream_unreachable`. |
| `error` | The cause of a failure, or null. |

## Redaction

The capture keeps each header name and replaces a redacted value with the literal `redacted`. A later product area reads the header names to find which credential type a request used.

The capture redacts a request header if its lowercase name is one of these:

1. `authorization`, `proxy-authorization`, `x-api-key`, `cookie`, `chatgpt-account-id`, or `x-claude-code-agent-id`.
2. A name that contains `key`, `token`, `secret`, or `session`.

The capture redacts a response header if its lowercase name is one of these:

1. `set-cookie`.
2. A name that contains `key`, `secret`, or `session`.

The substring `token` does not apply to response headers. A response header whose lowercase name starts with `anthropic-ratelimit-` or `x-ratelimit-` stays unredacted, because it holds counts and not credentials.

## Credential hash

Before the capture redacts the request headers, it computes the HMAC-SHA256 of the credential under `LOUPE_ACCOUNT_KEY`. The input is the value after `Bearer ` of `authorization` if that header is present. Else the input is the value of `x-api-key`. The row stores the hash as lowercase hex, and the header name as the credential source. If no credential header is present, both columns are null. The proxy stores the raw credential in no other form.

## Account ID hash

An account ID header is a request header that the README section "Accounts" lists as an account ID source. Now the only such header is `chatgpt-account-id`. Before the redaction, the capture computes the HMAC-SHA256 of the header value under `LOUPE_ACCOUNT_KEY`. The row stores the hash as lowercase hex, and the lowercase header name as the account ID source. If no such header is present, both columns are null. A later product area gets a header account ID only from these columns. An account ID in a body stays in the encrypted body.

## Encryption

The capture encrypts each body with AES-256-GCM under `LOUPE_DATA_KEY`. Each body gets a new random 96-bit nonce. The additional authenticated data is the exchange id plus the part name, `request` or `response`. Thus a body that moves to a different row or part fails to decrypt.

The stored value is one versioned blob:

| Offset | Length | Content |
|---|---|---|
| 0 | 1 byte | The version, now `1`. |
| 1 | 12 bytes | The nonce. |
| 13 | the rest | The ciphertext and the 16-byte tag. |

The source has one decrypt function. It takes the key, the exchange id, the part name, and the blob, and returns the body bytes. The decrypt of a stored body gives the bytes that the client and the upstream sent.

## Capture limit

The tap copies each chunk into a bounded buffer for each body. It never delays, blocks, changes, or fails the forwarded stream. The capture limit is the maximum count of bytes of one body that the capture keeps. `LOUPE_CAPTURE_MAX_BYTES` sets it, and the default is 33554432 (32 MiB). If a body is larger than the capture limit, the tap stops the copy and sets the truncated flag. The forwarded body stays complete, and the size column holds the full size.

The memory of the capture is at most the capture limit times the count of concurrent exchanges, plus the queue.

## Queue and spill

The forwarder gives each exchange record to the capture writer through a bounded queue that never waits. `LOUPE_CAPTURE_QUEUE` sets the size of the queue, and the default is 256 records.

If the queue is full, the proxy does not drop the record. A task off the request path seals the record under `LOUPE_DATA_KEY` and writes it as one file in `<LOUPE_DATA_DIR>/spill/`. The record does not stay in memory after the file is written. The proxy logs one warning line with the count of spilled records and increments a spill counter.

The capture writer imports the spill files in the order of their start time. It does this at the start of `loupe serve` and when the queue has space. It deletes each file after its row is committed. A second import of the same file makes no second row.

A failure of the capture writer or of a spill write logs an actionable error. Examples are an encryption failure, a database failure, and a disk failure. The failure does not stop `loupe serve` and does not change a later forwarded request or the forwarded response.

## Aborted streams

| Event | Proxy action | Outcome |
|---|---|---|
| The client closes the connection during a streamed response. | Closes the upstream request. Stores the bytes that it received. | `client_aborted` |
| The upstream closes during the response. | Ends the client stream. Stores the bytes that it received. | `upstream_aborted` |
| The upstream cannot be reached. | Returns status 502. | `upstream_unreachable` |

## Logs

A log line never holds a header value, a body, or a part of a body. A log line for an exchange holds only the exchange id, the agent label, the upstream name, the method, the path, the status, the sizes, the timing, and the outcome.

## Fake upstream

The test tree holds a fake upstream in Rust. It serves these responses:

1. A JSON response.
2. A server-sent events stream of chunks, with a set delay between the chunks.
3. A stream that it closes in the middle.
4. A slow response.
5. An echo of the request method, path, query, headers, and body.

An example program starts the fake upstream on a port from an argument, so that the latency script can use it.

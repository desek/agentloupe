<!-- @agents-index Specification of proxy tokens: format, table, token subcommands, bootstrap token, token gate states, and privacy rules. -->
# Proxy tokens

A proxy token lets a client use the proxy. The proxy uses the token to accept or refuse a request, and the capture records which token sent each exchange.

## Token format

A raw token is the prefix `lpt_` followed by the base64url encoding, with no padding, of 32 bytes from the random source of the operating system. The full token has 47 characters.

The binary stores only the SHA-256 hash of a token. The raw token is never written to the database, a log line, stderr, or a file. It appears one time, on stdout of the command that makes it.

## Table

A migration adds the table `proxy_tokens`:

| Column | Value |
|---|---|
| `id` | An integer primary key. |
| `name` | The token name. It is unique. |
| `token_hash` | The lowercase hex of the SHA-256 of the raw token. It is unique. |
| `tags` | A JSON object of string keys to string values. |
| `created_at` | The time of the create. |
| `last_used_at` | The time of the last accepted request. It is null until the first use. |
| `revoked_at` | The time of the revoke. It is null while the token is active. |

All times are UTC in the RFC 3339 format with millisecond resolution, as in `exchanges`.

## Names and tags

A token name must match `^[a-z0-9][a-z0-9-]{0,31}$`. The name `bootstrap` is reserved for the bootstrap token. A name is never reused, also after a revoke, so that past exchanges keep one meaning for each name.

A tag has the form `KEY=VALUE`. The key must match `^[a-z0-9][a-z0-9_.-]{0,31}$`. The value has 1 to 128 printable characters and no control character. A token has at most 16 tags. A repeated key is refused.

## Subcommands

| Subcommand | Behaviour |
|---|---|
| `loupe token generate` | Prints one new raw token on stdout. It does not open the database and stores nothing. |
| `loupe token create --name NAME [--tag KEY=VALUE]...` | Makes an active token, stores its hash, name, tags, and `created_at`, and prints the raw token one time. |
| `loupe token list` | Shows each token with its name, tags, `created_at`, `last_used_at`, `revoked_at`, and the status `active` or `revoked`. It never shows the raw token or the hash. |
| `loupe token revoke NAME` | Sets `revoked_at` on the token. The exchanges of the token do not change. |

Output:

1. In the machine format, `create` prints one JSON object with the fields `name`, `token`, `tags`, and `created_at`.
2. In the machine format, `list` prints one JSON object per line.
3. Under `--human`, or on a terminal, `list` prints a table.

Errors and repeated runs:

1. `create` with a name that exists, active or revoked, fails with a non-zero exit code. The error gives two fixes: select another name, or revoke and use another name, because a name is never reused. The check is `loupe token list`.
2. `create` refuses the name `bootstrap`.
3. `revoke` of a token that is already revoked exits 0 and keeps the first `revoked_at`.
4. `revoke` of an unknown name fails with a non-zero exit code. The error points to `loupe token list`.

`create` and `revoke` support `--dry-run`. The command validates the input, prints what it would do, and writes nothing. Each subcommand has a complete `--help`. A missing required argument names the flag.

## Bootstrap token

The bootstrap token is the token with the name `bootstrap`. In the role `full`, `loupe serve` makes, replaces, or revokes it from `LOUPE_BOOTSTRAP_TOKEN` before it opens the port. The role `public` has no bootstrap token step:

1. If the variable is set, the value must be a valid raw token. If not, the start stops with an actionable error. The error names the variable, the fix `loupe token generate`, the file `.env.example`, and the check `loupe config show`.
2. If the variable is set and no row named `bootstrap` exists, the step makes the row with no tags.
3. If the variable is set and the row exists with a different hash, the step replaces the hash and clears `revoked_at`. The row keeps its ID.
4. If the variable is not set and an active row named `bootstrap` exists, the step sets its `revoked_at`.

The step is idempotent. A second start with the same environment makes no change to `proxy_tokens`.

## Token segment in the route

A proxied path can start with an optional token segment: `/<token>/<agent>/<upstream>/<path>`. A first segment that starts with `lpt_` is the presented token. The rest of the path then parses with the rules in `proxy.md`. The agent label pattern has no `_`, so a segment that starts with `lpt_` is never an agent label.

## Token gate

The token gate exists only in the role `full`. The role `public` has no token gate and no proxy route. The token gate runs after the route parser and before the forwarder. It reads the token state from the database for each request, so a `create` or a `revoke` from another process takes effect on the next request with no restart. A cache of the state is at most 5 seconds old. The gate reads the age of the cache through an injected clock. The binary uses the system clock; a test uses a set clock, so it can make the cache expire without a wait.

The gate has three states:

| State | When | Request with no token segment | Request with a token segment |
|---|---|---|---|
| No-token mode | The table `proxy_tokens` has never held a row. | Accepted. | Refused with `token_mode_off`. |
| Required | The table holds at least one active row. | Refused with `token_missing`. | Accepted if the token is active, else refused with `token_unknown` or `token_revoked`. |
| Closed | The table holds rows, and every row is revoked. | Refused with `token_missing`. | Refused with `token_unknown` or `token_revoked`. |

A revoke of the last active token does not open the proxy. The proxy stays closed until a new token is active.

The gate finds a presented token by its SHA-256 hash, then compares the hash in constant time. Thus the time of a refusal does not show a part of a token.

## Refusals

A refused request gets status 401 and a JSON error body. The proxy does not forward it and stores no exchange for it.

| Error | Cause |
|---|---|
| `token_missing` | Tokens exist, and the request has no token segment. |
| `token_unknown` | The presented token matches no row. |
| `token_revoked` | The presented token matches a revoked row. |
| `token_mode_off` | No token has ever existed, and the request has a token segment. The client expects a check that the server does not do. |

The body of `token_missing`, `token_unknown`, and `token_revoked` states what failed, the fix, and the check. The fix is to put a valid token first in the base URL, as `/<token>/<agent>/<upstream>`. The check is `loupe token list`. No body echoes the presented token.

## Warnings

1. In no-token mode, `loupe serve` logs one warning line at start. The line states that the proxy is open to every client that can reach the port. It names the fix: `loupe token create` or `LOUPE_BOOTSTRAP_TOKEN`.
2. In the closed state, `loupe serve` logs one warning line at start. It logs the same line at the first request after the last active token is revoked while it runs. The line states that the proxy refuses all proxied requests and names the same fix.

## Last use

After a request passes the gate, the binary updates `last_used_at` of its token. The update happens at most one time per minute for each token. It does not delay or fail the forwarded request. The gate reads the interval of one minute through the same injected clock as the cache.

## Exchanges

A migration adds the column `proxy_token_id` to `exchanges`. It is a nullable integer that refers to `proxy_tokens.id`. The capture stores the ID of the token that passed the gate. It stores null for a request that no-token mode accepted.

## Privacy of the raw token

1. The log line of an exchange holds the token name, or `none`. It never holds the raw token or the hash.
2. Every log line, access log, and trace or span event uses the request path without the leading token segment. The binary removes the segment before any log or trace layer records the path.
3. The path in `exchanges.path` excludes the token segment.
4. A log line or a row that holds `lpt_` is a defect.

## Public API

The public API and the dashboard never show a token name, a tag, a token ID, or a hash. No route under `/v1/` reads `proxy_tokens`.

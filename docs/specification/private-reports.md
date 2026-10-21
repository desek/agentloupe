<!-- @agents-index Specification of the private page: admin token, login, cookie, rate limit, report and session routes, page rules, and headers. -->
# Private reports

The private page shows the usage, the cost, and the sessions per proxy token and per tag. It is at `/private/`, and its API is under `/private/api/`. Only the holder of the admin token can open it. The public API and the public dashboard never show this data.

## Admin token

1. The variable `LOUPE_ADMIN_TOKEN` holds the admin token. It is a secret and has no default.
2. If the value is not set or is empty, the private page is off. Every path under `/private/` then returns `404` with the same body as any unknown path.
3. If the value is set and has fewer than 32 characters, `loupe serve` stops before it opens the port. The error names the variable, the fix `loupe token generate`, the file `.env.example`, and the check `loupe config show`.
4. A change of the admin token makes every admin session cookie invalid.

## Authorization

1. A request to `/private/api/...` is authorized if the header `Authorization: Bearer <admin token>` matches, or if a valid admin session cookie is present.
2. The server compares the admin token in constant time, on the SHA-256 digests of the two values.
3. A proxy token never authorizes a private route.
4. The admin token never authorizes a proxied request.
5. A private API request without valid authorization returns `401`.
6. A private page request without valid authorization returns `303` to `/private/login`.

## Login and logout

1. `GET /private/login` returns a form with one password field.
2. `POST /private/login` with the correct token sets the admin session cookie and returns `303` to `/private/`.
3. `POST /private/login` with a wrong token returns `401` and the form with a neutral message. The message does not tell if the token was near or wrong in format.
4. `POST /private/logout` clears the cookie and returns `303` to `/private/login`.
5. The server logs each failed login with the remote address only. The line holds no token value.
6. After 10 failed logins from one remote address in 10 minutes, the server returns `429` to that address for 10 minutes. The server keeps this count in memory only.
7. The two `POST` routes refuse a request whose `Origin` header is present and is not the same origin.

## Admin session cookie

1. The name of the cookie is `loupe_admin`.
2. The cookie has `HttpOnly`, `SameSite=Strict`, `Path=/private`, and `Max-Age=43200` (12 hours).
3. The cookie has `Secure` when the request uses HTTPS or has `X-Forwarded-Proto: https`.
4. The value is an expiry time and an HMAC-SHA-256 over that time.
5. The key of the HMAC comes from the admin token. The server derives it at start and keeps it in memory only.
6. A cookie with a bad HMAC or an expiry time in the past is not valid.

## Report route

`GET /private/api/v1/report` returns the groups of calls in a range.

| Parameter | Meaning |
|---|---|
| `from`, `to` | Dates in `LOUPE_TZ`. The defaults and the 400-day limit are the same as for `/v1/days` (`api.md`). |
| `group` | `token` or `tag`. |
| `tag_key` | With `group=tag`, the tag key that makes the groups. |

1. With `group=tag`, a proxy token without the tag key counts in the group `(none)`.
2. A call with no proxy token counts in the group `(no token)`. These calls are from before the proxy tokens existed.
3. Each group row has these fields: `group`, `input_tokens`, `output_tokens`, `cache_read_tokens`, `cache_write_tokens`, `total_tokens`, `calls`, `sessions`, `api_equivalent_cost_usd` (exact decimal text), `unpriced_calls`, and `days`.
4. `days` has one entry for each day with a call of the group. Each entry has the same count fields and the date.
5. With `group=token`, each row also has the tags, `created_at`, `last_used_at`, and the status `active` or `revoked`.
6. A session counts on each day that has a call of it, the same rule as in `days.md`.
7. A session counts in a group if a call of it is in that group.

## Sessions route

`GET /private/api/v1/sessions` lists the sessions of one proxy token in the range.

1. Each row has `agent_label`, `started_at`, `last_call_at`, `call_count`, `total_tokens`, `api_equivalent_cost_usd`, and the session day summary of the session for the start day of the segment in `LOUPE_TZ` (`summaries.md`). It is never the full session summary. Two rows of one session on one day show the same text.
2. The route returns at most 200 rows.
3. If more rows exist, the response has a `next` cursor. A request with that cursor returns the next rows.

## Providers route

`GET /private/api/providers` returns the provider status of each known provider.

1. The route exists only when `LOUPE_ADMIN_TOKEN` is set. It uses the authorization of this file. A request without valid authorization returns `401`.
2. Each entry has `name`, `status`, and `checked_at`. The status is `ok`, `unauthorized`, `unreachable`, or `not_configured`. `checked_at` is UTC, RFC 3339.
3. The response never holds a credential or a base URL.
4. A read uses the cached status. It never starts a ping. `operations.md` gives the ping rules.

## Configuration route

`GET /private/api/config` returns the configuration that the process loaded at start.

1. The route exists only when `LOUPE_ADMIN_TOKEN` is set. It uses the authorization of this file. A request without valid authorization returns `401`, as the other private API routes do.
2. The response is JSON, with one object for each entry: `name`, `value`, and `source`. It holds the same content as `loupe config show`: each effective value with its source, the provider lines, `summary_provider`, and the derived lines of `configuration.md`.
3. The endpoint and `loupe config show` use one shared render of the entries, so the masking is the same on the two surfaces. Each secret is `set` or `unset`.
4. The response never holds a credential value, the admin token, a proxy token, or a part of one.
5. A read never starts a ping and never reloads the configuration.

## Page

1. The page shows the range selector (3M, 6M, 1Y).
2. The page shows a group selector: token, or tag with a list of the tag keys.
3. The page shows a table of the groups.
4. The page shows the tokens per day for the selected group.
5. The page shows the session list of a selected proxy token.
6. The page uses the design tokens, the fonts, and the dark theme of the dashboard (`dashboard.md`).
7. The binary contains the page files at compile time, as it does for the dashboard.
8. The page gets data only from `/private/api/v1/...` on the same origin.
9. The page files are at most 200 KiB, apart from the shared fonts.

## Headers

Every private response has these headers:

1. `cache-control: no-store`.
2. `x-frame-options: DENY`.
3. A strict Content Security Policy.

Private responses send no CORS header.

## Privacy

1. No route under `/v1/` and no file of the public dashboard reads the proxy tokens table or returns a token name, a tag, or a token ID.
2. The admin token, the cookie value, and the key of the HMAC never occur in a log line, in the database, or in a response.

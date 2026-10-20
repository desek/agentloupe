<!-- @agents-index Specification of the public API: the /v1/days routes, parameters, envelope, errors, CORS, cache headers, and version policy. -->
# Public API

The public API serves the stored day records of `days.md`. Other sites can read it with no credential.

## `GET /v1/days`

The query parameters are `from` and `to`, each a `YYYY-MM-DD` date. Without `to`, `to` is the current day in `LOUPE_TZ`. Without `from`, `from` is `to` minus 365 days.

A valid request returns `200` with `content-type: application/json` and this envelope:

| Field | Value |
|---|---|
| `from` | The first day of the range. |
| `to` | The last day of the range. |
| `updated_at` | The latest build time of the published days, as RFC 3339 in UTC, or `null` when no day is published. |
| `subscription_cost_usd` | The sum of the subscription cost of each day from `from` to `to`, both ends included, days with no record included, rounded half to even to 4 fraction digits, as a JSON number. It uses the current configuration. |
| `days` | The records of the published days in the range, both ends included, sorted by `day` ascending. |

## `GET /v1/days/{day}`

The route returns `200` with the record alone when the day is published. It returns `404` with the code `not_found` when the day is not published. It returns `400` with the code `invalid_date` when `{day}` is not a valid date.

## Errors

An error returns the body `{"error": {"code": ..., "message": ..., "fix": ...}}`. The `fix` names a valid example query.

| Code | Status | Cause |
|---|---|---|
| `invalid_date` | 400 | A `from`, `to`, or `{day}` that is not a valid `YYYY-MM-DD` date. |
| `inverted_range` | 400 | A `from` after `to`. |
| `range_too_long` | 400 | A range of more than 400 days. |
| `unknown_parameter` | 400 | A query parameter other than `from` and `to`. |
| `not_found` | 404 | No published record for the day. |

## Stored records only

The API returns the stored record text. It never builds a record in the path of a request. The routes under `/v1/` read only the `days` table. They never read the proxy tokens, the accounts, the exchanges, the calls, the sessions, or the summary tables. A day in the state `pending_review` or `deleted` never appears in a response.

## Field allowlist

Each field name of a record and of the envelope is in the allowlist of `days.md`, at each level except the keys of `by_agent` and `by_model`. A test fails if a response holds a field name outside the allowlist.

## Headers and methods

1. Each response under `/v1/` and of `GET /healthz` carries `access-control-allow-origin: *`.
2. An `OPTIONS` request under `/v1/` returns `204` with `access-control-allow-methods: GET, OPTIONS` and `access-control-allow-headers: content-type`.
3. A method other than `GET` and `OPTIONS` returns `405` with `allow: GET, OPTIONS`.
4. A `200` carries `cache-control: public, max-age=60`. A `400` or `404` carries `cache-control: no-store`.
5. The API reads no cookie and no `authorization` header. It sets no cookie.

## Access

The routes under `/v1/` need no proxy token. A path with a token segment, `/<token>/v1/days`, does not serve the API.

## Version policy

The path prefix `/v1/` is the version. A change within `/v1/` only adds fields or routes. A change that removes or changes a field or a route uses a new prefix, such as `/v2/`. The project publishes no JSON Schema and no OpenAPI file for the API.

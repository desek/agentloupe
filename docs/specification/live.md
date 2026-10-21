<!-- @agents-index Specification of the live route, the live hub, the live events and their allowlist, the snapshot, the rate limit, and the limits. -->
# Live feed

The live feed sends the activity of the current day to the dashboard while it occurs. It carries counts and token sums only.

## Live route

1. The binary serves `GET /live` as a WebSocket upgrade route.
2. A request without a valid upgrade returns `426` with a JSON error body and the header `upgrade: websocket`.
3. `live` is a reserved first path segment. It is not a valid agent label or upstream name. A path under `/live/` other than `/live` returns `404`.
4. The route ignores a query string.

## Origin rule

1. The route accepts the upgrade when the request has no `origin` header. A non-browser client sends no `origin` header.
2. The route accepts the upgrade when the host and port of the `origin` header equal the `Host` header of the request. The route makes a default port explicit on the two sides before the comparison. The route ignores the scheme, because a front proxy can end TLS.
3. If `LOUPE_LIVE_ALLOWED_ORIGIN` is set, the route also accepts an `origin` equal to that value. This covers a front proxy that changes the `Host` header.
4. The route returns `403` for every other origin.

## No credentials

The route does not set or read a cookie. The route does not accept a proxy token in the path or in a query parameter. The route needs no authentication.

## Frames from the client

1. The route answers a ping control frame with a pong.
2. The route answers a close control frame with a close.
3. The route discards each text or binary data frame of 1 KiB or less.
4. The route closes the socket with code `1009` on a data frame larger than 1 KiB.

## Disabled state

If `LOUPE_LIVE_ENABLED` is `false`, `GET /live` returns `404` and the live hub does not start.

## Live hub

The binary holds one live hub. The hub is one bounded broadcast channel with a capacity of `LOUPE_LIVE_BUFFER` events. A send to the hub never waits. A send when no client is connected does nothing. The capture, the proxy, and the parser never wait for a client.

## Senders

1. The capture sends `exchange.start` when it starts to forward a request.
2. The capture sends `exchange.end` when the response ends, with or without an error.
3. The parser sends `call.stored` after it stores a call.
4. After each parse pass, the parser counts the open segments. It sends `segments.open` when the count changed since the last pass.

Only the parser worker inside `loupe serve` sends `call.stored`. A `loupe parse --once` or a `loupe backfill` run in a second process stores calls that live clients see in the next snapshot, not as events.

## Event allowlist

Each live event holds only the fields of the column for its route. The five token counts are `input`, `output`, `cache_read`, `cache_write`, and `reasoning`. `status` is the HTTP status. `cost_usd` is the API-equivalent cost of `cost.md`, or `null` if no price is known.

| Kind | Public route `/live` | Private feed (a later change) |
|---|---|---|
| `exchange.start` | `ts` | `ts`, `agent` |
| `exchange.end` | `ts`, `status` | `ts`, `agent`, `status`, `ttfb_ms`, `duration_ms` |
| `call.stored` | `ts`, the five token counts, `cost_usd` | `ts`, `agent`, `model`, the five token counts, `cost_usd` |
| `segments.open` | `ts`, `count` | the same as public |
| `heartbeat` | `ts` | the same as public |
| `snapshot` | `date`, `calls`, `tokens` (the five token counts), `cost_usd`, `segments_open`, `in_flight`, `live_enabled` | the same as public |
| `calls.batch` | `ts`, `calls`, the five token counts, `cost_usd` | the public fields, and `by_agent` with `calls`, the five token counts, and `cost_usd` for each agent label |
| `exchanges.batch` | `ts`, `started`, `ended`, `in_flight` | the same as public |

The public route uses the public column. The private column applies only to a later private feed on the private page. That feed is not part of the binary yet. In that feed, `agent` and `by_agent` hold the agent label of the exchange, the same value as `by_agent` in the public API.

## Privacy rule

A live event does not hold a prompt, a body, a path, a query, a header, a session key, a segment ID, an account, a proxy token, a token ID, a token name, an upstream URL, or a summary. A test of the serializer proves the allowlist for each kind.

## Envelope

Each frame is one JSON text frame with the shape `{"v":1,"kind":"<kind>","data":{...}}`. A change that removes or renames a field increments `v`. A change that adds a field keeps `v`.

## Snapshot

After the upgrade, the route sends one `snapshot` frame first. The frame holds the date and the totals of the current day:

1. `date` is the current day in `LOUPE_TZ`, as `YYYY-MM-DD`.
2. `calls`, `tokens`, and `cost_usd` are the totals of the calls of the current day. The current day uses the rule of `days.md`: a call is in the day if its start time is inside the bounds of the day in `LOUPE_TZ`.
3. `segments_open` is the count of segments with the state open.
4. `in_flight` is the count of exchanges that started and did not end.
5. `live_enabled` is `true`.

## Rate limit and coalescing

1. The route sends at most `LOUPE_LIVE_MAX_RATE` frames per second to one client.
2. Above the rate, the route adds the `call.stored` events into one `calls.batch` frame. The public route sends no sums for each agent.
3. Above the rate, the route keeps only the last `segments.open` event.
4. Above the rate, the route changes the `exchange.start` and `exchange.end` events into one `exchanges.batch` frame with `started`, `ended`, and `in_flight`.

## Heartbeat

The route sends a `heartbeat` frame when it sent no other frame for 15 seconds.

## Connection limit

The route accepts at most `LOUPE_LIVE_MAX_CONNECTIONS` sockets at one time. The next upgrade returns `503` with the header `retry-after: 30`.

## Slow clients

The route closes a client with code `1008` in two cases:

1. The receiver of the client falls behind the hub, and the hub drops events for it.
2. A write to the client does not complete in 5 seconds.

## Configuration

| Variable | Range | Default |
|---|---|---|
| `LOUPE_LIVE_ENABLED` | `true` or `false` | `true` |
| `LOUPE_LIVE_ALLOWED_ORIGIN` | one origin | unset |
| `LOUPE_LIVE_MAX_RATE` | 1 to 50 frames per second | `10` |
| `LOUPE_LIVE_MAX_CONNECTIONS` | 1 to 1024 | `32` |
| `LOUPE_LIVE_BUFFER` | 64 to 65536 events | `1024` |

A value out of its range stops the start with an actionable error that names the variable. `loupe config show` prints the lines `live_enabled`, `live_allowed_origin`, `live_max_rate`, `live_max_connections`, and `live_buffer`, each with its source.

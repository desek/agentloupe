<!-- @agents-index User guide: What leaves the machine, what stays, and the start request of the release builds. -->

# Privacy model

## Privacy model

| Data | What happens to it |
|---|---|
| Credentials (`authorization`, `x-api-key`, cookies, key and token headers) | Redacted before storage, never logged |
| Account identity | HMAC-SHA256 hash only, never in the public API |
| Proxy tokens | SHA-256 hash, name, tags, and dates. Never the raw token, never in the public API |
| GitHub tokens | Environment only, never stored |
| Provider credentials (`CLAUDE_CODE_OAUTH_TOKEN`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`) | Environment only, never stored, never logged, never in `loupe config show` or an API response |
| Pull requests | HMAC-SHA256 hash, token label, dates, line counts. Never the repository, number, or URL |
| Request and response bodies | Encrypted with `LOUPE_DATA_KEY`, deleted 168 hours after parse and summary |
| Exchange metadata | Agent, upstream, method, path, status, sizes, timing |
| Calls | Model, status, token counts, cost |
| Public mirror (`public/public.db`) | A derived copy that the role `full` writes and the role `public` reads, read-only. It has two tables only: `days` (the published day records) and `live_state` (the live counters of today). No raw request, session, call, token, or key |
| Public data | Daily totals, totals per agent and per model, PR and line counts, one summary line per day |
| Live feed (`/live`) | Counts and token sums only: exchange start and end with the HTTP status, the five token counts and the cost of each call, the open segment count. No agent, model, prompt, body, path, header, key, or token |
| Start request (release builds only) | One GET of `https://<host>/<commit>` at each start of `loupe serve`, with no body. It holds the commit of the build in the path and `agentloupe/<version> (<target triple>)` in `User-Agent`. No data of the user, the host, or the dashboard |

A release asset and the release image send one start request when `loupe serve` starts: a GET of
`https://<host>/<commit>`, where the path is the full 40-character commit of the build and the host
is fixed at build time. The request has no body, no query, no cookie, and no credential. Its
`User-Agent` header is `agentloupe/<version> (<target triple>)`, for example
`agentloupe/0.2.0 (aarch64-apple-darwin)`. The purpose is a count of the commits and versions in use;
the request holds no data of the user, the host, the calls, or the dashboard. It starts after the
port is open, has one attempt with a timeout of 5 seconds, and its result never changes the start.
The start log line `telemetry` names the state at each start (`sent`, `disabled`, or `built-out`),
and `loupe config show` prints the line `telemetry` with the build state. The receiver is the
Cloudflare Worker `agentloupe-telemetry` on the custom domain `telemetry.agentloupe.dev`, whose
source is under [`telemetry/`](../../telemetry/). It stores per commit only the first date, the last date (UTC dates),
and a count. It stores no IP address, no `User-Agent`, no header, no query, and no time of day; its
logs are off, and it has no read route. A build from source and a local image build
(`docker compose up -d --build`) hold no host and send no request.
[`docs/specification/telemetry.md`](../specification/telemetry.md) states the rules.

The role `public` reads only the public mirror. It stops at start when a secret is set or when
`loupe.db` is in its data directory, so the public container cannot reach the private data.

A call that `loupe backfill` reads from a session log stores only the token counts, the model, the
time, and HMAC keys; it stores no prompt, no path, no directory name, and no raw ID of the log.

A private page exists behind `LOUPE_ADMIN_TOKEN`. It shows usage, cost, and sessions per proxy
token and per tag. Its data never reaches the public API. Without the admin token, every path
under `/private/` returns 404. The section Private page gives the use.

The summary model gets the transcript as data, without the system prompt, the tool definitions,
or system-reminder blocks. Its rules: a neutral tone in the third person, with no opinion and no
praise; one line of at most 12 words for a session, which starts with a past-tense verb; no names of
people, clients, companies, products, repositories, files, or tools; no paths, URLs, email
addresses, or numbers; no other non-public information. Only the projects in
`LOUPE_PUBLIC_PROJECTS` can be named. A trivial or failed session also gets a line that says what
was tried. The full session summary is one paragraph of at most 8 sentences and the session day
summary one paragraph of at most 4 sentences, under the same rules; both are stored for reference
and are never published. The day line is one paragraph of 3 to 8 sentences, built from the session
day summaries of the day under the same rules.

A scrubber runs on every line before storage. It removes URLs, email addresses, paths, and every
token with a digit, except a name in `LOUPE_PUBLIC_PROJECTS`. Raw bodies are deleted 168 hours
(`LOUPE_RAW_RETENTION_HOURS`) after parse and summary; the calls and sessions stay.

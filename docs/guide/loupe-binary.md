<!-- @agents-index User guide: The subcommands of the loupe binary and what each one does. -->

# The loupe binary

## The `loupe` binary

One binary holds the full product: the proxy, the workers, the public API, and the dashboard.

- Language: Rust. The crate name is `agent-loupe`, the binary name is `loupe`, and the
  configuration variables start with `LOUPE_`.
- Storage: SQLite on one volume.
- Runs in two containers (`docker compose`) from one image, each port bound to `127.0.0.1`.
  `LOUPE_ROLE` selects the role of `loupe serve`: `full` (the default) runs the proxy, the
  workers, the private page, and the public routes, and writes the public mirror; `public` reads
  only the public mirror and serves only the public routes (see "Public hosting").
- Commands:
  - `loupe serve`: in the role `full`, the proxy, the workers, the public API, and the dashboard;
    in the role `public`, the dashboard, the public API, and the live feed only.
  - `loupe aggregate [--date YYYY-MM-DD|today] [--dry-run]`: build one day now.
  - `loupe backfill --source claude-code|pi [--path DIR] [--from YYYY-MM-DD] [--to YYYY-MM-DD]
    [--pi-label LABEL] [--dry-run]`: read the session logs on disk into the calls and the sessions,
    for the work before the proxy. A session that the proxy captured is skipped.
  - `loupe gen-key`: print a new 32-byte key for `LOUPE_DATA_KEY` or `LOUPE_ACCOUNT_KEY`.
  - `loupe --version`: print `loupe <version>`, the version of `Cargo.toml` at build time.
  - `loupe day approve --date YYYY-MM-DD`: publish a day that waits for review.
  - `loupe day edit --date YYYY-MM-DD --summary TEXT`: replace the summary of a day.
  - `loupe day delete --date YYYY-MM-DD`: remove a day. A deleted day is marked, so the nightly
    publish does not create it again.
  - `loupe day restore --date YYYY-MM-DD`: build a deleted day again and publish it.
  - `loupe token generate | create | list | revoke`: manage the proxy tokens (see "Proxy
    tokens").
- Routes:
  - Proxy: `/<agent>/<upstream>/<path>`, for example `/claude-code/anthropic/v1/messages`. The
    file `upstreams.toml` maps each upstream name to a base URL. It is also the allowlist of the
    hosts that the proxy can forward to. When a proxy token is active, the routes start with
    `/<token>/`.
  - Dashboard: `GET /` and its static files.
  - Public API (open CORS, so other sites can also read it): `GET /v1/days?from=YYYY-MM-DD&to=YYYY-MM-DD`
    returns `{"from", "to", "updated_at", "subscription_cost_usd", "days": [...]}`,
    `GET /v1/days/{day}` returns one record, and `GET /healthz`. An error returns
    `{"error": {"code", "message", "fix"}}` with the code `invalid_date`, `inverted_range`,
    `range_too_long`, `unknown_parameter`, or `not_found`.

Configuration:

| Variable | Required | Purpose |
|---|---|---|
| `LOUPE_ROLE` | no | Role of `loupe serve`: `full` (default) or `public`. `compose.yaml` sets `public` for `loupe-public` only. |
| `LOUPE_DATA_KEY` | yes, in the role `full` | Encrypts the captured request and response bodies. |
| `LOUPE_ACCOUNT_KEY` | yes, in the role `full` | Secret for the HMAC hashes of accounts and pull requests. |
| `LOUPE_BOOTSTRAP_TOKEN` | no | A proxy token from a secret (see "Proxy tokens"). |
| `LOUPE_GITHUB_TOKEN_<LABEL>` | no | One or more GitHub tokens (see "GitHub"). |
| `LOUPE_GITHUB_API_URL` | no | Base URL of the GitHub API. Default `https://api.github.com`. Must use `https`, except for `127.0.0.1` or `localhost`. |
| `CLAUDE_CODE_OAUTH_TOKEN` | no | Claude subscription token from `claude setup-token`, sent as `Authorization: Bearer`. Works with `claude-haiku-5-5` only. Detects the `anthropic` provider. |
| `ANTHROPIC_API_KEY` | no | Console API key. Detects the `anthropic` provider. If both Anthropic credentials are set, the key is used. |
| `OPENAI_API_KEY` | no | OpenAI API key, sent as `Authorization: Bearer`. Detects the `openai` provider. |
| `ANTHROPIC_BASE_URL` | no | Base URL of the Anthropic API, without `/v1`. Default `https://api.anthropic.com`. Must use `https`, except for `127.0.0.1` or `localhost`. |
| `OPENAI_BASE_URL` | no | Base URL of the OpenAI API, with `/v1`. Default `https://api.openai.com/v1`. Must use `https`, except for `127.0.0.1` or `localhost`. |
| `LOUPE_SUMMARY_PROVIDER` | no | `anthropic` or `openai`. Default: `anthropic` if it is detected, else `openai`. A detected provider enables the summaries; with no detected provider, no summaries. |
| `LOUPE_SUMMARY_MODEL` | no | Model for the summaries. Default `claude-haiku-5-5` for `anthropic`, no default for `openai`. With `CLAUDE_CODE_OAUTH_TOKEN` as the active credential, a different model needs `ANTHROPIC_API_KEY`. |
| `LOUPE_PROVIDER_PING_SECS` | no | Seconds between two pings of the models list of each provider. Default `300`, minimum `60`. |
| `LOUPE_TZ` | no | Time zone that cuts the days, an IANA name such as `Europe/Stockholm`. Default `UTC`. |
| `LOUPE_PUBLISH_AT` | no | Time to publish the previous day. Default `01:00`. |
| `LOUPE_PUBLISH_INTERVAL_SECS` | no | Seconds between two checks for days to publish. Default `60`. |
| `LOUPE_IDLE_SECS` | no | Seconds without calls before a session closes. Default `3600`. |
| `LOUPE_RAW_RETENTION_HOURS` | no | Hours to keep raw bodies after parse and summary. Default `168`. |
| `LOUPE_SITE_TITLE` | no | Title of the public page. Default `Agentloupe`, the product name. Set it to show another title. |
| `LOUPE_PUBLIC_PROJECTS` | no | Comma-separated names of public projects that summaries may name. Default empty. |
| `LOUPE_DAY_REVIEW` | no | `auto` publishes days with no review, `manual` waits for `loupe day approve`. Default `auto`. |
| `LOUPE_SUBSCRIPTION_<NAME>` | no | Monthly price in USD of a subscription, shown next to the API-equivalent cost. |
| `LOUPE_ADMIN_TOKEN` | no | Secret for the private page. Without it, the private page is off. |

`LOUPE_SUMMARY_API_KEY`, `LOUPE_SUMMARY_OAUTH_TOKEN`, and `LOUPE_SUMMARY_BASE_URL` are removed
names. If one of them is set, `loupe serve`, `loupe summarize`, and `loupe config show` stop at
start with an error that names the new name. Rename the variable in `.env`. Then
`loupe config show` prints the `provider.anthropic` line.

A day has a record when it has agent calls or GitHub activity. A day with neither has no record,
and the dashboard shows it as zero. The day line is published with no manual review. To change a
day later, run `loupe aggregate --date` again.

Example day record. The pull request and line fields are `null` when no GitHub token is set.
`detail.github.failed_tokens` is the number of GitHub tokens whose fetch of the day failed:

```json
{
  "day": "2026-10-17",
  "input_total": 1283200, "input": 21000, "cache_read": 1250000, "cache_write": 12200,
  "output": 103969, "calls": 212, "sessions": 4,
  "cost_usd": 2.4744, "unpriced_calls": 0, "subscription_cost_usd": 6.4516,
  "prs_opened": 3, "prs_merged": 2, "lines_added": 412, "lines_removed": 97,
  "summary": "shipped UI fixes to match brand identity",
  "detail": { "by_agent": { "claude-code": {}, "pi": {} }, "by_model": {}, "github": { "failed_tokens": 0 } }
}
```

# Agentloupe

> A minimal AI gateway that gives you the illusion that you can keep up with your agents.

Insight into your work with AI coding agents, and transparency about it.

Agentloupe publishes a public, nightly record of the work that I do with AI coding agents. It
shows how many tokens the agents used, what those tokens cost at API list prices, how many pull
requests I opened and merged, how many lines changed, and one line per day that describes the
kind of work. No prompt, no code, and no detail that identifies a project leaves the machine.

The product domains are `agentloupe.dev` and `agentloupe.app`.

## Status

The project is at the brainstorm stage. No code base exists yet. The two sources in
[`brainstorm/`](brainstorm/) record the brainstorm:

| Source | What it is | Use |
|---|---|---|
| `brainstorm/agent-ledger.zip` | Untested prototype code for the proxy, in Rust. | Reference only, to understand the brainstorm. The code base is written from scratch to this README. |
| `brainstorm/dashboard.grenemark.se-html.zip` | A design mockup of the public dashboard, with sample data. | Reference design for colors, sizes, and layout. Not production code. |

This README is the authority. Where a source differs from it, the README is correct.

## Scope

In scope:

- One container that runs on this machine, reached through `localhost`.
- Claude Code and Pi, which send their model requests through the proxy.
- Pull requests from GitHub, read through the GitHub API.

Out of scope for now:

- Public hosting. It will be managed separately later.
- claude.ai chats (web, desktop, mobile). They go directly to claude.ai, so the proxy cannot
  record them. The dashboard footer states which sources it covers.

## How it works

```
Claude Code ─┐                            ┌─► api.anthropic.com
             ├─► proxy ── forwards as-is ─┤
Pi ──────────┘     │                      └─► api.openai.com, chatgpt.com, …
                   ▼ capture (encrypted)
               exchanges ─► parser ─► calls ─► sessions ─(idle)─► summarize ─► agg_queue
                                                                                   │
GitHub API ─► GitHub worker (PRs, lines) ──────────────────────► days ◄─(nightly)──┘
                                                                   │
                                                            GET /v1/days
                                                                   │
                                                                   ▼
                                                    dashboard at GET / (same binary)
```

1. The agents (Claude Code and Pi) send their model requests to the proxy through a base URL.
2. The proxy forwards each request unchanged and stores the exchange. It encrypts the bodies
   with AES-256-GCM and redacts the credentials.
3. A parser reads the stored exchanges. It finds model calls from the usage block in the
   response (Anthropic Messages, OpenAI Responses, OpenAI Chat Completions).
4. The parser prices each call from the vendored LiteLLM price file (see "Cost").
5. The parser groups the calls into sessions. A session closes after 30 minutes with no calls.
6. A small model writes one line for each closed session, under strict rules. A scrubber then
   removes URLs, paths, email addresses, and numbers from the line.
7. The GitHub worker counts the pull requests and the changed lines of the day (see "GitHub").
8. At 01:00 (Europe/Stockholm) the binary builds the previous day and publishes it on
   `GET /v1/days`.
9. The same binary serves the dashboard at `/`. The page reads `/v1/days` and shows the data.

## The `loupe` binary

One binary holds the full product: the proxy, the workers, the public API, and the dashboard.

- Language: Rust. The crate name is `agent-loupe`, the binary name is `loupe`, and the
  configuration variables start with `LOUPE_`.
- Storage: SQLite on one volume.
- Runs as one container (`docker compose`), with the port bound to `127.0.0.1`.
- Commands:
  - `loupe serve`: the proxy, the workers, the public API, and the dashboard.
  - `loupe aggregate [--date YYYY-MM-DD]`: build one day now.
  - `loupe gen-key`: print a new 32-byte key for `LOUPE_DATA_KEY` or `LOUPE_ACCOUNT_KEY`.
  - `loupe token generate | create | list | revoke`: manage the proxy tokens (see "Proxy
    tokens").
- Routes:
  - Proxy: `/<agent>/<upstream>/<path>`, for example `/claude-code/anthropic/v1/messages`. The
    file `upstreams.toml` maps each upstream name to a base URL. It is also the allowlist of the
    hosts that the proxy can forward to. When a proxy token is active, the routes start with
    `/<token>/`.
  - Dashboard: `GET /` and its static files.
  - Public API (open CORS, so other sites can also read it): `GET /v1/days?from=YYYY-MM-DD&to=YYYY-MM-DD`,
    `GET /v1/days/{day}`, `GET /healthz`.

Configuration:

| Variable | Required | Purpose |
|---|---|---|
| `LOUPE_DATA_KEY` | yes | Encrypts the captured request and response bodies. |
| `LOUPE_ACCOUNT_KEY` | yes | Secret for the HMAC hashes of accounts and pull requests. |
| `LOUPE_BOOTSTRAP_TOKEN` | no | A proxy token from a secret (see "Proxy tokens"). |
| `LOUPE_GITHUB_TOKEN_<LABEL>` | no | One or more GitHub tokens (see "GitHub"). |
| `LOUPE_SUMMARY_API_KEY` | no | Anthropic API key for the one-line summaries. Without it, no summaries. |
| `LOUPE_SUMMARY_MODEL` | no | Model for the summaries. Default `claude-haiku-5-5`. |
| `LOUPE_TZ` | no | Time zone that cuts the days. Default `Europe/Stockholm`. |
| `LOUPE_PUBLISH_AT` | no | Time to publish the previous day. Default `01:00`. |
| `LOUPE_IDLE_SECS` | no | Seconds without calls before a session closes. Default `1800`. |
| `LOUPE_RAW_RETENTION_HOURS` | no | Hours to keep raw bodies after parse and summary. Default `24`. |

Example day record:

```json
{
  "day": "2026-10-17",
  "input_total": 1283200, "input": 21000, "cache_read": 1250000, "cache_write": 12200,
  "output": 103969, "calls": 212, "sessions": 4,
  "cost_usd": 2.4744, "unpriced_calls": 0,
  "prs_opened": 3, "prs_merged": 2, "lines_added": 412, "lines_removed": 97,
  "summary": "shipped UI fixes to match brand identity",
  "detail": { "by_agent": { "claude-code": {}, "pi": {} }, "by_model": {} }
}
```

## Dashboard

The dashboard is a stateless single-page app. The `loupe` binary serves its files at `/`, built
into the binary, so one artifact ships the full product. The page gets all its data from the
public API. It has no login, no cookies, and no state on the server, because all the data that
it shows is public.

| Section | Data |
|---|---|
| Header | Title "Agentloupe", time of the last update, range selector (3M, 6M, 1Y) |
| Pull requests | PRs opened and PRs merged for the range, bar chart per week |
| Lines changed, all time | Lines added, lines removed, net lines |
| Tokens per day | Input and output, two line charts, each on its own scale |
| Cost (API-equivalent) | A card in the totals row: the total for the range, and a daily line chart in the style of the token charts. The label says that the figure is a list price, not money paid. |
| Token usage, last 12 months | Heatmap per day, five levels from quartiles |
| Daily log | One line per day, with the PR count and the token count |
| Footer | The sources that the page covers, today Claude Code and Pi |

The design uses the Geist and Geist Mono fonts. The values to copy (colors, sizes, spacing)
are in the inline styles of `Main.dc.html` in the mockup archive. An implementation must
replicate these values in its own components. It must not copy the mockup runtime
(`support.js`, `vendor/`).

## Proxy tokens

A proxy token lets a client use the proxy. You can make many proxy tokens, for example one for
each machine, each person, or each CI pipeline. The binary records which token sent each
request, so you can later analyse the usage by token and by the metadata of the token.

- **Format.** A random value with the prefix `lpt_`. The prefix lets secret scanners find a
  leaked token.
- **Storage.** The binary stores only the SHA-256 hash of the token. It shows the raw token one
  time, when you make it. A plain hash is sufficient here, because the token is made from
  random bytes. The HMAC of the "Accounts" section is necessary for values that the binary does
  not make.
- **Metadata.** Each token has a unique name and free `key=value` tags, for example
  `machine=work-laptop`, `owner=daniel`, or `purpose=ci`. The binary also records when the
  token was made, when it was last used, and when it was revoked.
- **Tracking.** Each exchange, call, and session stores the ID of its proxy token. A session
  never mixes two tokens. The provider account hash of the "Accounts" section is recorded
  independently, so an analysis can group by token, by account, or by both.
- **Privacy.** Token names and tags stay in the database. The public API does not show them.
- **Use.** The client puts the token first in the route: `/<token>/<agent>/<upstream>/<path>`.
  When at least one token is active, every proxied request needs a valid token. With no active
  token, the routes have no token part. The binary then logs a warning at start, because anyone
  who can reach the port can use the proxy.
- **Bootstrap.** A deployment without a shell can set `LOUPE_BOOTSTRAP_TOKEN` from a secret. At
  start, the binary records it as the token named `bootstrap`, as a hash only. If the value
  changes, the binary replaces the hash. If the variable is removed, the binary revokes the
  `bootstrap` token.
- **Commands.**
  - `loupe token generate`: print a new random token and store nothing. Use it to make the value
    of `LOUPE_BOOTSTRAP_TOKEN`.
  - `loupe token create --name NAME [--tag KEY=VALUE]...`: make a token and print it one time.
  - `loupe token list`: show the name, the tags, and the dates of each token, never the token.
  - `loupe token revoke NAME`: stop the token. Its past records stay.

## Accounts

The binary identifies the provider account between sessions without a sign-up. For each
request, it uses the first source that exists:

1. A stable account ID that the agent sends:
   - Claude Code: the account UUID in `metadata.user_id` of the request body.
   - Codex (ChatGPT login): the `ChatGPT-Account-Id` header.
2. The credential that the agent sends to the provider: the `authorization` bearer token or the
   `x-api-key` header. This is the fallback that catches every request without an account ID.

The account ID sources are not verified yet. The implementation must confirm them with a
capture of real requests from the agent versions in use, and update this list.

The binary computes an HMAC-SHA256 of the value with the server secret `LOUPE_ACCOUNT_KEY`,
before it redacts the headers. It stores only the hash and the name of the source. It never
stores the raw credential or the raw account ID. `LOUPE_ACCOUNT_KEY` is separate from
`LOUPE_DATA_KEY`, so a rotation of the data key does not change the accounts. Without the server
secret, nobody can compute the hash from a known key, so a copy of the database does not show
which rows belong to which key.

Known limits:

- A subscription login with no account ID in the request falls back to the provider token. That
  token refreshes, so each refresh starts a new account.
- An API key that you rotate starts a new account.
- One person who uses more than one provider gets one account for each provider.
- The account ID sources depend on the request format of each agent, and that format can change.

## Cost

Agentloupe shows what each call costs at the list price of the provider API. It does this also
when the agent uses a subscription. For a subscription, the figure is the API-equivalent cost:
what the same tokens would cost on the API, not what you paid. The dashboard and the API label
it as such.

The prices come from `vendor/litellm/model_prices_and_context_window.json`, the LiteLLM price
file. `vendor/litellm/model_prices_and_context_window.schema.json` is its JSON schema, from the
same commit. `vendor/litellm/SOURCE.md` records the pinned commit and the checksums. Both files
are MIT licensed (`vendor/litellm/LICENSE`). The binary builds the price file in at compile
time.

To refresh the prices, run `.agents/scripts/prices.vendor.sh`, then rebuild the binary. The
script validates the price file against the schema before it writes anything. A test of the
binary validates it again, so a refresh that breaks the format fails the build. The price fields
that the binary reads use the names that the schema defines. The schema uses Python regex
syntax, for example `(?i)`, so the validator must use a Python-compatible regex dialect.

The binary prices each call with these rules:

1. **Find the entry.** Use the model name from the response, and the entry whose
   `litellm_provider` matches the upstream. If no entry matches, try the name without a
   trailing date (`-YYYYMMDD`).
2. **Add the token costs.**
   - Uncached input × `input_cost_per_token`
   - Cache reads × `cache_read_input_token_cost`
   - Cache writes with a 5-minute lifetime × `cache_creation_input_token_cost`
   - Cache writes with a 1-hour lifetime × `cache_creation_input_token_cost_above_1hr`
   - Output, which includes reasoning tokens × `output_cost_per_token`
3. **Apply the prompt-size tier.** The prompt size is uncached input + cache reads + cache
   writes. If it is above the threshold of an `…_above_<N>k_tokens` field, use the price of
   that tier for every token in the call.
4. **Apply the service tier.** If the response reports the `priority` or `flex` service tier,
   use the `…_priority` or `…_flex` field when one exists.
5. **Do not guess.** If no entry matches, or a price field is missing for a token count that is
   not zero, the call has no price. Its cost is `null`, and the call counts in
   `unpriced_calls`.
6. **Keep history stable.** The binary stores the cost on each call, together with the commit
   of the price file. A later refresh of the prices does not change past days.

To apply these rules, the parser must record 1-hour cache writes apart from 5-minute cache
writes, and the service tier of each response.

## GitHub

A worker in the binary reads pull requests from the GitHub API for each day, at the same time
as the day is published. It catches up missed days in the same way as the rest of the day.

- **Tokens.** Set one or more variables `LOUPE_GITHUB_TOKEN_<LABEL>`, for example
  `LOUPE_GITHUB_TOKEN_PERSONAL` and `LOUPE_GITHUB_TOKEN_ACME`. The binary records the label with
  each pull request, so an analysis can group by it. The tokens stay in the environment. The
  database never stores them.
- **What it counts.** For each token, the worker gets the user of the token (`GET /user`). It
  then searches (`GET /search/issues`) for pull requests by that user that were opened on the
  day and that were merged on the day. The day is the date in `LOUPE_TZ`, sent as a time range
  with its UTC offset. For each merged pull request, it gets the additions and the deletions
  (`GET /repos/{owner}/{repo}/pulls/{pull_number}`).
- **No double counts.** Two tokens can see the same pull request. The worker identifies a pull
  request by the HMAC-SHA256 of `owner/repo#number` with `LOUPE_ACCOUNT_KEY`, and counts it one
  time. It never stores the repository name, the number, or the URL.
- **Day fields.** `prs_opened`, `prs_merged`, `lines_added`, and `lines_removed`. The lines
  count on the day of the merge.
- **Failures.** If a token fails (expired, revoked, not approved), the worker logs the label and
  the cause, and the day records which labels are missing.

Required permissions for a fine-grained personal access token (recommended):

| Setting | Value |
|---|---|
| Resource owner | Your user account, or one organization. A fine-grained token reaches one owner only, so make one token for each owner. |
| Repository access | All repositories, or only the repositories to count. |
| Repository permissions: Pull requests | Read-only |
| Repository permissions: Metadata | Read-only. GitHub adds this permission automatically. |
| Account permissions | None |
| Organization permissions | None |

An organization can require approval for fine-grained tokens. Such a token works only after the
organization approves it. The search endpoint needs no extra permission. It returns only the
pull requests in repositories that the token can see, plus pull requests in public repositories.

A classic token also works, but it is not recommended. For private repositories it needs the
`repo` scope, which also gives write access to all of them. For public repositories only, a
classic token with no scope is sufficient.

## Privacy model

| Data | What happens to it |
|---|---|
| Credentials (`authorization`, `x-api-key`, cookies, key and token headers) | Redacted before storage, never logged |
| Account identity | HMAC-SHA256 hash only, never in the public API |
| Proxy tokens | SHA-256 hash, name, tags, and dates. Never the raw token, never in the public API |
| GitHub tokens | Environment only, never stored |
| Pull requests | HMAC-SHA256 hash, token label, dates, line counts. Never the repository, number, or URL |
| Request and response bodies | Encrypted with `LOUPE_DATA_KEY`, deleted 24 hours after parse and summary |
| Exchange metadata | Agent, upstream, method, path, status, sizes, timing |
| Calls | Model, status, token counts, cost |
| Public data | Daily totals, totals per agent and per model, PR and line counts, one summary line per day |

The summary model gets the transcript as data, without the system prompt. It must not write
names of people, companies, products, repositories, files, paths, URLs, or numbers.

## Open work

- **Code base.** Write the `loupe` binary from scratch to this README. Use the prototype only to
  understand the brainstorm.
- **Account ID sources.** Confirm the Claude Code and Codex account IDs with a capture of real
  requests during the implementation, and update the "Accounts" section.
- **Dashboard design.** The mockup has no cost card and shows merged PRs only. Design the cost
  card and the opened PRs in the style of the mockup.

## License

The vendored LiteLLM files are MIT licensed. The license of Agentloupe itself is not decided.

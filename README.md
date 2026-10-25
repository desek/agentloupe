# Agentloupe

> A minimal AI gateway that gives you the illusion that you can keep up with your agents.

Insight into your work with AI coding agents, and transparency about it.

Agentloupe publishes a public, nightly record of the work that I do with AI coding agents. It
shows how many tokens the agents used, what those tokens cost at API list prices, how many pull
requests I opened and merged, how many lines changed, and one paragraph per day that describes the
kind of work. No prompt, no code, and no detail that identifies a project leaves the machine.

The product domain is `agentloupe.dev`.

## Quickstart

Everything runs in one container on this machine. The proxy listens on `127.0.0.1:8787`, and the
agents reach it through a base URL that carries a proxy token, the agent label, and the upstream,
as `http://127.0.0.1:8787/<token>/<agent>/<upstream>`. A bare `ANTHROPIC_BASE_URL=http://localhost:8787`
does not work, because the root of the proxy serves the public API and the dashboard.

1. Make the configuration. Copy `.env.example` to `.env` and fill in:
   - `LOUPE_DATA_KEY` and `LOUPE_ACCOUNT_KEY`, required, each from `loupe gen-key` (or
     `docker compose run --rm loupe loupe gen-key` before the first start). They encrypt the
     captured bodies and hash the accounts; keep them, a new key makes the old data unreadable.
   - `CLAUDE_CODE_OAUTH_TOKEN` from `claude setup-token`, for the summaries on `claude-haiku-5-5`.
     Without it the days get no summary paragraph. Do not set `ANTHROPIC_API_KEY` for that.
   - `LOUPE_TZ`, the time zone that cuts the days, for example `Europe/Stockholm`. Default `UTC`.
   - `LOUPE_SUBSCRIPTION_<NAME>=<USD per month>` for each subscription you pay, optional.
   - `LOUPE_GITHUB_TOKEN_<NAME>` for the pull request counts, optional. See [GitHub](#github).
2. Start the container: `docker compose up -d --build`. Check `curl -s http://127.0.0.1:8787/healthz`.
3. Make a proxy token into a file outside the repository, so it never lands in a shell history:
   `(umask 077; docker compose exec -T loupe loupe token create --name laptop | sed -E 's/.*"token":"([^"]+)".*/\1/' > ~/.config/agentloupe-token)`.
4. Connect Claude Code in the shell that runs it, then run it as usual:
   `eval "$(.agents/scripts/agent.connect.sh --agent claude-code --token-file ~/.config/agentloupe-token --check)"`.
   The script exports `ANTHROPIC_BASE_URL` with the full path and changes nothing under `~/.claude`.
5. Connect Pi the same way, with `--agent pi`. The script exports `PI_CODING_AGENT_DIR` with an
   isolated directory that holds only `models.json`; Pi takes its credential from
   `ANTHROPIC_OAUTH_TOKEN` or from a credential extension. See [Connect the agents](#connect-the-agents).
6. Open the dashboard at `http://127.0.0.1:8787/`. The live card moves as the agents work, and the
   day records appear after the nightly publish, or at once with
   `docker compose exec loupe loupe aggregate --date today`.
7. To load the sessions that predate the proxy, mount the session logs read-only in an untracked
   `compose.override.yaml` and run `docker compose exec loupe loupe backfill --source claude-code`
   and `--source pi`. See [The `loupe` binary](#the-loupe-binary).

A release asset or the registry image replaces step 2 for an install without the source. See
[Install](#install).

## Status

The `loupe` binary is implemented and runs in the container. 27 change requests under
[`docs/cr/`](docs/cr/) carry the specification of each part, and
[`docs/specification/`](docs/specification/) describes the behaviour of the code. The binary
captures Claude Code and Pi through the proxy, prices and summarizes the calls, publishes the day
records, serves the dashboard with its live section and the private page, and backfills the session
logs that predate the proxy.

Releases are automated. release-please keeps one release pull request from the Conventional Commits
on `main`; its merge makes the tag `v<version>` and the GitHub release. The release workflow then
attaches one archive per platform with its checksum file, and publishes the multi-platform image
`ghcr.io/desek/agentloupe`. See [Install](#install) and
[`docs/specification/release.md`](docs/specification/release.md).

The design values of the dashboard came from a design mockup with sample data. The archive
`brainstorm/agent-ledger.zip` records the brainstorm and stays as reference only: it holds untested
prototype code for the proxy, in Rust, and the code base was written from scratch to this README.

This README is the authority. Where a source differs from it, the README is correct.

## Scope

In scope:

- One container that runs on this machine, reached through `localhost`.
- The public instance: a second container in the role `public` that serves only the public
  page, the public API, and the live feed from the public mirror (see "Public hosting").
- Claude Code and Pi, which send their model requests through the proxy.
- Pull requests from GitHub, read through the GitHub API.

Out of scope for now:

- The Cloudflare Tunnel in front of the public instance. It is later work.
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
5. The parser groups the calls into sessions. A session closes after 60 minutes with no calls. A subagent is its own session.
   A session that spans midnight counts on each day that has a call.
6. A small model writes one line for each closed session segment. For each session and each day
   with a call of that session, it writes a full session summary, which covers the session up to
   the end of that day, and a session day summary, which says what the session did on that day.
   It reads the whole session, the conversation first. The paragraph of 3 to 8 sentences for each
   day comes from the session day summaries of that day, in a neutral tone. A scrubber then
   removes URLs, paths, email addresses, and numbers from the line.
7. The GitHub worker counts the pull requests and the changed lines of the day (see "GitHub").
8. At 01:00 (`LOUPE_TZ`, default UTC) the binary builds the previous day and publishes it on
   `GET /v1/days`.
9. The same binary serves the dashboard at `/`. The page reads `/v1/days` and shows the data.

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

## Dashboard

The dashboard is a stateless single-page app. The `loupe` binary serves its files at `/`, built
into the binary, so one artifact ships the full product. The page gets all its data from the
public API. It has no login, no cookies, and no state on the server, because all the data that
it shows is public. The two roles serve the same page: `loupe` on port 8787 and `loupe-public`
on port 8788, which reads the published days and the live counters from the public mirror.

| Section | Data |
|---|---|
| Header | One row: the loupe mark, the title from `LOUPE_SITE_TITLE` (default `Agentloupe`), and the range selector (1M, 3M, 6M, 1Y). Below it, token velocity as the headline |
| Live | Below the headline, the card "Today": a dot that pulses while the WebSocket route `/live` sends frames, then in flight, calls, tokens, tokens per minute, and API-equivalent cost of the current day, and a sparkline of tokens per 10 seconds over the last 10 minutes |
| Pull requests | PRs opened and PRs merged for the range, bar chart per week |
| Lines changed | The pull request card ends with one line that shows lines added and lines removed for the selected range, with no net value |
| Tokens per day | Input and output, two line charts, each on its own scale |
| Cost (API-equivalent) | A card in the totals row: the total for the range, and a daily line chart in the style of the token charts. The label says that the figure is a list price, not money paid. Next to it, the subscription cost from `LOUPE_SUBSCRIPTION_<NAME>`, spread over the days of the range. |
| Token usage, last 12 months | Heatmap per day, five levels from quartiles |
| Daily log | One line per day, with the PR count and the token count |
| Footer | The sources that the page covers, today Claude Code and Pi |

The headline is the token velocity: the input and output tokens of the range divided by the
days of the range, in tokens per day. The ranges are 1M (the last 30 days), 3M (the last 91 days), 6M (the last 182
days), and 1Y (the last 365 days), each ending on the newest day of the API. 1M is the default. The URL fragment
keeps the range, for example `#range=6M`. The live section updates while calls occur. Calls that `loupe parse --once` or `loupe backfill` store from a second process appear at the next page load, not as live events. When the
socket closes it shows "Paused, reconnecting" and connects again; `LOUPE_LIVE_ENABLED=false`
removes the route, and the page then hides the section. The subscription cost card shows the subscription
cost of the range next to the API-equivalent cost. The page shows only sums for the range. It
shows no streaks and no record days.

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
  Before the first token exists, the routes have no token part, and the binary logs a warning at
  start, because anyone who can reach the port can use the proxy. After the first token exists,
  every proxied request needs a valid active token. If no token is active, for example after you
  revoke the last one, the proxy refuses all proxied requests until you make a new token.
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

## Install

There are three ways to get `loupe`. Each release has a tag `v<version>`, for example `v0.1.0`, and
`<version>` below is the version with no `v`. [`docs/specification/release.md`](docs/specification/release.md)
states the versions, the release flow, and the assets.

### From a release asset

Each GitHub release of [desek/agentloupe](https://github.com/desek/agentloupe/releases) has one
archive per platform and one `.sha256` checksum file per archive:

| Platform | Archive |
|---|---|
| macOS arm64 | `loupe-v<version>-aarch64-apple-darwin.tar.gz` |
| Linux x86_64 (glibc 2.35 or newer) | `loupe-v<version>-x86_64-unknown-linux-gnu.tar.gz` |
| Linux arm64 (glibc 2.35 or newer) | `loupe-v<version>-aarch64-unknown-linux-gnu.tar.gz` |

1. Download the archive of your platform and its checksum file:
   ```sh
   V=<version>; A=loupe-v$V-aarch64-apple-darwin.tar.gz
   curl -fLO https://github.com/desek/agentloupe/releases/download/v$V/$A
   curl -fLO https://github.com/desek/agentloupe/releases/download/v$V/$A.sha256
   ```
2. Check the checksum: `shasum -a 256 -c $A.sha256` on macOS, `sha256sum -c $A.sha256` on Linux.
   Each prints `OK`.
3. Unpack: `tar -xzf $A`. The directory `loupe-v$V-<target>/` holds `loupe`, `upstreams.toml`,
   `config/`, `LICENSE`, `README.md`, and `THIRD-PARTY-NOTICES` (the licenses of the third-party
   parts of the binary). The binary reads `upstreams.toml` and `config/` from the
   working directory, so run it from that directory.
4. Run: `cd loupe-v$V-*/ && ./loupe --version` prints `loupe <version>`. Then
   `LOUPE_DATA_KEY=... LOUPE_ACCOUNT_KEY=... ./loupe serve`, with two keys from `./loupe gen-key`.
5. The macOS binary is not signed. A download through a browser can get the quarantine attribute;
   remove it with `xattr -d com.apple.quarantine loupe`. A download with `curl` does not get it.

### From the registry image

The image `ghcr.io/desek/agentloupe` has the tags `<version>` (no `v`) and `latest`, for
`linux/amd64` and `linux/arm64`. A pull needs no login. The command below uses the same port, volume,
and env file as `compose.yaml`:

```sh
docker pull ghcr.io/desek/agentloupe:<version>
docker run -d --name loupe --restart unless-stopped \
  -p 127.0.0.1:8787:8787 -v loupe-data:/data --env-file .env -e LOUPE_ENV=container \
  ghcr.io/desek/agentloupe:<version>
```

`docker run --rm ghcr.io/desek/agentloupe:<version> gen-key` prints a key for `.env`.

### From source

Install Rust with rustup. In the repository, rustup reads `rust-toolchain.toml` and uses that
toolchain. Then build and run:

```sh
cargo build --release --locked --bin loupe
./target/release/loupe --version
```

The local image build of [Connect the agents](#connect-the-agents) (`docker compose up -d --build`)
also builds from source.

A build from source sends no start request: the build variable `LOUPE_TELEMETRY_HOST` is unset, so
the binary holds no telemetry host and `loupe config show` prints `telemetry` as `built-out`. Only
the release workflow sets it (see [Privacy model](#privacy-model)).

The receiver of the start request lives under [`telemetry/`](telemetry/). Only the owner deploys it,
with `.agents/scripts/telemetry.deploy.sh` and the `wrangler` of `.tool-versions`: `--test` runs the
Worker test on a local database, and `--dry-run` shows the steps and changes nothing. The Worker
answers on the custom domain `telemetry.agentloupe.dev` only; its `workers.dev` host is off.

### Check a release

`.agents/scripts/release.check.sh --version <version>` checks a published release on a macOS or Linux
machine with `curl` and Docker: it downloads the six assets with no credential, verifies the
checksums, runs `loupe --version` of each binary, pulls the image with no login, checks
`GET /healthz`, the version in the first log line, and the line `telemetry`, and checks
`THIRD-PARTY-NOTICES` in each archive and in the image. Its starts send no start request. It exits 0 when each check passes or is
skipped with a stated reason.

## Connect the agents

The connect script `.agents/scripts/agent.connect.sh` points one agent at the proxy for one
shell. It does not change `~/.claude`, `~/.pi/agent`, or a file in the repository. It takes the
proxy token from a file (`--token-file`) or from `LOUPE_PROXY_TOKEN`, never as an argument value.

1. Put `LOUPE_DATA_KEY`, `LOUPE_ACCOUNT_KEY`, and `CLAUDE_CODE_OAUTH_TOKEN` in `.env`. The
   summarizer uses the subscription token from `claude setup-token` and the default model
   `claude-haiku-5-5`. Do not set `LOUPE_SUMMARY_MODEL` or `ANTHROPIC_API_KEY`.
2. Start the container: `docker compose up -d --build`.
3. Make a proxy token into a file outside the repository:
   `(umask 077; docker compose exec -T loupe loupe token create --name laptop | sed -E 's/.*"token":"([^"]+)".*/\1/' > ~/.config/agentloupe-token)`.
   The file holds only the token value. `docker compose exec loupe loupe token list` shows the name.
4. Connect Claude Code in the shell that runs it:
   `eval "$(.agents/scripts/agent.connect.sh --agent claude-code --token-file ~/.config/agentloupe-token --check)"`.
   The script prints `export ANTHROPIC_BASE_URL=...`, which points Claude Code at the proxy.
5. Run Claude Code: `claude -p "Reply with the word ok." --model claude-haiku-5-5`.
6. Connect Pi: `eval "$(.agents/scripts/agent.connect.sh --agent pi --token-file ~/.config/agentloupe-token)"`.
   The script writes only `models.json` into the isolated agent directory
   `${XDG_STATE_HOME:-$HOME/.local/state}/agentloupe/pi-agent`. That directory holds no credential.
   The script prints `export PI_CODING_AGENT_DIR=...`, which points Pi at that directory.
7. Give Pi its credential from the environment. Use one of the two routes:
   - The plain route: run `claude setup-token` one time. Then export `ANTHROPIC_OAUTH_TOKEN`
     with that token in the shell that runs `pi -p`.
   - The alternative route: load a Pi extension that supplies the credential, for example an
     extension that reads the Claude Code login from the keychain (`pi -e <extension>`).
8. Check the model, then run Pi:
   `pi --list-models claude-haiku-5-5`, then
   `pi -p --provider anthropic --model claude-haiku-5-5 "Reply with the word ok."`.
9. Open the dashboard at `http://127.0.0.1:8787/`.

The nightly publish writes the day record. To see a day before the nightly publish, run
`docker compose exec loupe loupe aggregate --date YYYY-MM-DD`. A session closes after
`LOUPE_IDLE_SECS` with no calls, by default `3600` seconds. A short value in an untracked
`compose.override.yaml` (`services.loupe.environment.LOUPE_IDLE_SECS: "60"`) closes the sessions
sooner.

## Accounts

The binary identifies the provider account between sessions without a sign-up. For each
request, it uses the first source that exists:

1. A stable account ID that the agent sends:
   - Codex (ChatGPT login): the `ChatGPT-Account-Id` header.
   - Claude Code: the account UUID in `metadata.user_id` of the request body.
2. The credential that the agent sends to the provider: the `authorization` bearer token or the
   `x-api-key` header. This is the fallback that catches every request without an account ID.

A capture of real requests on 2026-10-20 verified these sources per agent version:

| Agent | Version | Account source | Session source |
|---|---|---|---|
| Claude Code | 2.1.304 | `metadata.user_id`, a JSON object string with `account_uuid`, `device_id`, `session_id` | the header `x-claude-code-session-id`; a subagent call also sends `x-claude-code-agent-id`, which makes it its own session |
| Pi | 1.3.0 | the credential in `authorization` | no session header and no `prompt_cache_key`, so the hash of the opening of the conversation |

The Codex source is not verified yet.

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
  the cause. The day records the number of failed tokens in `detail.github.failed_tokens`. The
  label names stay in the private database and in the logs. If a search finds more than 1000
  pull requests, the fetch fails with the cause `search_limit`, because the search API returns
  at most 1000 results and a part count is wrong.
- **Retry.** On each wake, the worker fetches again each published day of the last 7 days that
  has a failed token. A day that changes gets a new revision. After you fix a token, you can
  also run `loupe aggregate --date <day>`.

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

## Private page

A private page at `/private/` shows usage, cost, and sessions per proxy token and per tag. It
needs `LOUPE_ADMIN_TOKEN`, sent as a header or through a login form. Its data never appears in
the public API.

Two more routes need the same authorization:

- `GET /private/api/providers` returns the provider status of each provider: `ok`,
  `unauthorized`, `unreachable`, or `not_configured`, with `checked_at`. The status comes from a
  cached ping of the models list. It never changes `GET /healthz`.
- `GET /private/api/config` returns the same entries as `loupe config show`. Each secret is `set`
  or `unset`.

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
source is under [`telemetry/`](telemetry/). It stores per commit only the first date, the last date (UTC dates),
and a count. It stores no IP address, no `User-Agent`, no header, no query, and no time of day; its
logs are off, and it has no read route. A build from source and a local image build
(`docker compose up -d --build`) hold no host and send no request.
[`docs/specification/telemetry.md`](docs/specification/telemetry.md) states the rules.

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

## Public hosting

`compose.yaml` runs two services from one image:

| Service | Role | Port |
|---|---|---|
| `loupe` | `full`: the proxy, the workers, the private page, the public routes, and the mirror writer | `127.0.0.1:8787` |
| `loupe-public` | `public`: the dashboard, the public API, and the live feed only | `127.0.0.1:8788` |

Expose only the port 8788. Never expose the port 8787, because it holds the proxy and the
private page.

The public container reaches only the public mirror, read-only, with the published days and the
live counters. It cannot reach `loupe.db`, the raw requests, the session data, the spill files,
the logs of the role `full`, or the secrets. It never mounts the volume `loupe-data` and never
reads `.env`. A local `compose.override.yaml` that mounts the session logs for `loupe backfill`
must name only the service `loupe`, never `loupe-public`.

A Cloudflare Tunnel is later work. Put only the public port, or the service `loupe-public`,
behind the tunnel. A tunnel on the compose network can reach the service `loupe`, so the tunnel
configuration must name `loupe-public` only.

A rate limit and a cache in front of the public port are the job of the host, for example the
rules of Cloudflare. The binary has no rate limit on the public routes beyond the live connection
limit.

Put the settings of the public service in `.env.public`, from `.env.public.example`, with no
secret. `LOUPE_LIVE_ALLOWED_ORIGIN` covers a front proxy that changes the `Host` header, and
`LOUPE_LIVE_MAX_CONNECTIONS` limits the live sockets. Copy `LOUPE_TZ`, `LOUPE_SITE_TITLE`, and
the `LOUPE_SUBSCRIPTION_` lines from `.env`, so the two pages match.

1. Run `cp .env.public.example .env.public` and fill in the values.
2. Run `docker compose up -d --build`.
3. Check with `curl -s http://127.0.0.1:8788/healthz`. The body is `{"status":"ok"}`.

## Open work

- **Codex account source.** Confirm the `ChatGPT-Account-Id` header of Codex with a capture of
  real requests, and update the "Accounts" section.
- **Build provenance.** Attest the release assets and sign the image, with an SBOM.
- **Draft releases.** Keep a release as a draft until its assets exist.
- **Release build cache.** Build the release assets with no cargo cache.
- **Pinned actions text.** Align the "Pinned actions" text of `release.md` with the Dependabot
  updates of the actions.
- **Secret scanning of fixtures.** Add a `.gitleaksignore` for the test fixtures.
- **Repository files.** Add `CODEOWNERS` and `.editorconfig`, and sign the commits.
- **Release check after publish.** Run `release.check.sh` automatically after each release.

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) before you open a pull request, and follow the
[Code of Conduct](CODE_OF_CONDUCT.md). Report a vulnerability in private as
[SECURITY.md](SECURITY.md) describes, never in a public issue.

## License

Agentloupe is MIT licensed, see [`LICENSE`](LICENSE). The vendored LiteLLM files are MIT licensed, and the vendored Geist fonts are under the SIL Open Font License (`web/assets/fonts/OFL.txt`). Each release archive and the release image carry `THIRD-PARTY-NOTICES`, with the licenses of the crates of the binary, the LiteLLM license, and the Geist font license.

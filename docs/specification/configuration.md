<!-- @agents-index How the loupe binary resolves, validates, and shows its configuration values and their sources, the role, and the build variables of the start request. -->
# Configuration

## Precedence

The loader resolves each value from the first source that has it, in this order:

1. A command-line flag.
2. An environment variable.
3. The file for the environment, `config/<env>.toml`.
4. The built-in default.

The source of a value is one of `flag`, `env`, `file`, or `default`.

The subcommands `serve` and `config show` take the flags `--env`, `--listen`, `--data-dir`, and `--idle-secs`. Each flag overrides the variable with the same name.

## Environment name

The flag `--env` or the variable `LOUPE_ENV` gives the name of the environment. The default is `local`. The file `config/<env>.toml` in the repository holds the values for that environment. This file holds only values that are not secret. The repository has `config/local.toml` and `config/container.toml`.

## Variables

| Variable | Required | Secret | Default |
|---|---|---|---|
| `LOUPE_ENV` | no | no | `local` |
| `LOUPE_ROLE` | no | no | `full` (`full` or `public`) |
| `LOUPE_LISTEN` | no | no | `0.0.0.0:8787` in the container |
| `LOUPE_DATA_DIR` | no | no | `/data` in the container, `./data` outside it |
| `LOUPE_DATA_KEY` | yes | yes | none |
| `LOUPE_ACCOUNT_KEY` | yes | yes | none |
| `LOUPE_BOOTSTRAP_TOKEN` | no | yes | none |
| `LOUPE_GITHUB_TOKEN_<LABEL>` | no | yes | none |
| `LOUPE_GITHUB_API_URL` | no | no | `https://api.github.com`, `https` or a loopback host only |
| `CLAUDE_CODE_OAUTH_TOKEN` | no | yes | none |
| `ANTHROPIC_API_KEY` | no | yes | none |
| `OPENAI_API_KEY` | no | yes | none |
| `ANTHROPIC_BASE_URL` | no | no | `https://api.anthropic.com`, `https` or a loopback host only |
| `OPENAI_BASE_URL` | no | no | `https://api.openai.com/v1`, `https` or a loopback host only |
| `LOUPE_SUMMARY_PROVIDER` | no | no | selected from the detected providers |
| `LOUPE_PROVIDER_PING_SECS` | no | no | `300` (60 or more) |
| `LOUPE_SUMMARY_MODEL` | no | no | `claude-haiku-5-5` for `anthropic`, none for `openai` |
| `LOUPE_SUMMARY_INTERVAL_SECS` | no | no | `30` |
| `LOUPE_SUMMARY_MAX_ATTEMPTS` | no | no | `5` |
| `LOUPE_SUMMARY_MIN_CONVERSATION_CHARS` | no | no | `200` |
| `LOUPE_TZ` | no | no | `UTC` |
| `LOUPE_PUBLISH_AT` | no | no | `01:00` |
| `LOUPE_PUBLISH_INTERVAL_SECS` | no | no | `60` |
| `LOUPE_IDLE_SECS` | no | no | `3600` |
| `LOUPE_PARSE_INTERVAL_SECS` | no | no | `5` |
| `LOUPE_RAW_RETENTION_HOURS` | no | no | `168` |
| `LOUPE_SITE_TITLE` | no | no | `Agentloupe` |
| `LOUPE_PUBLIC_PROJECTS` | no | no | empty |
| `LOUPE_DAY_REVIEW` | no | no | `auto` |
| `LOUPE_SUBSCRIPTION_<NAME>` | no | no | none |
| `LOUPE_ADMIN_TOKEN` | no | yes | none |
| `LOUPE_UPSTREAMS_FILE` | no | no | `upstreams.toml` in the working directory |
| `LOUPE_CAPTURE_MAX_BYTES` | no | no | `33554432` |
| `LOUPE_CAPTURE_QUEUE` | no | no | `256` |
| `LOUPE_UPSTREAM_CONNECT_TIMEOUT_SECS` | no | no | `10` |
| `LOUPE_UPSTREAM_HEADERS_TIMEOUT_SECS` | no | no | `120` |
| `LOUPE_LIVE_ENABLED` | no | no | `true` |
| `LOUPE_LIVE_ALLOWED_ORIGIN` | no | no | unset |
| `LOUPE_LIVE_MAX_RATE` | no | no | `10` (1 to 50) |
| `LOUPE_LIVE_MAX_CONNECTIONS` | no | no | `32` (1 to 1024) |
| `LOUPE_LIVE_BUFFER` | no | no | `1024` (64 to 65536) |

A `LOUPE_LISTEN` port of `0` makes the kernel select a free port, and the `listening` log line shows the bound address, so a test can run many servers at one time.

The loader parses and validates every value. A value that only a later product area uses does not change the behaviour of the binary until that area exists.

## Key encoding

`LOUPE_DATA_KEY` and `LOUPE_ACCOUNT_KEY` are keys. The encoding of a key is standard base64 with padding. A key is valid only if it decodes to exactly 32 bytes. `loupe gen-key` makes a valid key.

## Missing or bad values

If a required value is missing or not valid, `loupe serve` stops before it opens the port. It returns a non-zero exit code and writes an actionable error to stderr. The error:

1. Names the variable.
2. Gives the labeled fixes. For a key, the fix is to run `loupe gen-key` and put the value in `.env`.
3. Points to `.env.example`.
4. States what to verify: run `loupe config show` and see `set` for the variable.

## `loupe config show`

The subcommand prints each effective value with its source. It prints every secret as `set` or `unset`. It never prints a secret value or a part of a secret value.

The subcommand also prints these lines:

1. `price_commit`, the commit of the embedded price file.
2. The number of entries in the price index.
3. One line `subscription.<name>: <USD per month>` for each subscription.
4. `subscription_total_per_month: <sum>`.
5. One line for each known provider: `provider.<name>: credential=<kind or none> base_url=<url> source=<source>`. The line never prints the credential.
6. `summary_provider: <name or none>`, the provider of the summarizer.
7. `LOUPE_TZ`, `LOUPE_PUBLISH_AT`, `LOUPE_PUBLISH_INTERVAL_SECS`, and `LOUPE_DAY_REVIEW`, each with its source.
8. The next due publish time, in `LOUPE_TZ` and in UTC.
9. `site_title: <value>`, the effective site title.
10. `live_enabled`, `live_allowed_origin`, `live_max_rate`, `live_max_connections`, and `live_buffer`, each with its source.
11. `role`, the role of `loupe serve`, with its source.
12. `telemetry`, with the source `build`. The value is `built-out` when the binary holds no telemetry host, and `built-in host=<host>` when the binary holds a telemetry host. The line shows the build state only. It reads no run-time variable.

`GET /private/api/config` returns the same entries as `loupe config show`, from the same render. `private-reports.md` gives the rules.

## Role

`LOUPE_ROLE` selects the role of `loupe serve`. The values are `full` and `public`. The default is `full`. The loader resolves it from the environment, then the file of the environment, then the default. The role has no command-line flag.

1. In the role `full`, `loupe serve` runs the proxy, the workers, the private page, and the public routes, and it writes the public mirror. `days.md` (section "Public mirror") gives the mirror.
2. In the role `public`, `loupe serve` reads only the public mirror and serves only the public routes. It does not load or validate the upstreams file, so `LOUPE_UPSTREAMS_FILE` has no effect. The validation of each value that is not secret is the same in the two roles.
3. A value other than `full` or `public` stops `loupe serve` and `loupe config show` before the port opens. The actionable error names `LOUPE_ROLE`, the two values, the fix, `.env.example`, and the check `loupe config show`.
4. In the role `public`, `LOUPE_DATA_KEY` and `LOUPE_ACCOUNT_KEY` are not required, for `loupe serve` and for `loupe config show`. `loupe config show` prints each secret as `set` or `unset`, as in the role `full`.
5. The other subcommands (`aggregate`, `backfill`, `day`, `token`, `gen-key`, `summarize`, `parse`, `price`) ignore `LOUPE_ROLE`.

The role `public` has two guards. They run at start, before the process opens a file, and they fail closed:

1. **Secret guard.** When a secret variable is set to a value that is not empty, the start stops. The secret variables are each variable that the table "Variables" marks secret and each variable with the prefix `LOUPE_GITHUB_TOKEN_`, so a new secret variable joins the guard with no second list. The actionable error names each set variable and never holds a value. The fix is to remove the variable from the environment of the public process and to use `.env.public` with no secret, as `.env.public.example` shows. The check is `loupe config show`, which prints each secret as `unset`.
2. **Private database guard.** When `<LOUPE_DATA_DIR>/loupe.db` exists, the start stops. The actionable error states that the private database is reachable from the public process. The fix is to give the public process its own data directory with only the mirror volume, as in `compose.yaml`. The check is `loupe config show` and the list of the mounts of the service.

## Build variables

The build reads two variables of its own at build time: `LOUPE_TELEMETRY_HOST` and `LOUPE_BUILD_COMMIT`. The binary never reads them at run time. They are not in the table "Variables", and `loupe config show` does not list them as variables. A value of one of them in the environment of `loupe serve` has no effect. A change of either value at build time builds the binary again. `telemetry.md` gives the use of the values.

`LOUPE_TELEMETRY_HOST` is the host of the start request.

1. When the variable is absent, or empty after the removal of white space at the start and the end, the binary holds no telemetry host. The build state is `built-out`.
2. A set value is a host name with an optional port, for example `telemetry.agentloupe.dev`, `127.0.0.1:8899`, or `localhost`. The value has no scheme, no path, no query, no fragment, no user name, no password, and no empty label.
3. The scheme of the request is `https`. It is `http` only when the host is `127.0.0.1` or `localhost`, for a local fake endpoint. This follows the loopback rule of the provider base URLs.
4. A value that is not valid stops the build with an actionable error. The error names `LOUPE_TELEMETRY_HOST`, the rule, and the fix. It does not print the full value.

`LOUPE_BUILD_COMMIT` is the commit of the build. The build reads it only when `LOUPE_TELEMETRY_HOST` is set. It finds the commit in this order:

1. `LOUPE_BUILD_COMMIT`, when it holds exactly 40 lowercase hex characters. Any other value counts as absent.
2. The commit of `HEAD` from git in the source tree, when git and the repository exist and the result is 40 lowercase hex characters. A new commit builds the binary again with the new commit.
3. Else the build stops with an actionable error that names `LOUPE_BUILD_COMMIT`, the rule of 40 lowercase hex characters, and the fix.

When `LOUPE_TELEMETRY_HOST` is set, the build also puts the target triple of the build, as cargo gives it, into the binary. A `built-out` build needs no commit. The build does not mark a tree with local changes.

## Provider credentials

The binary reads three provider credentials:

| Variable | Provider | Kind | Auth header |
|---|---|---|---|
| `CLAUDE_CODE_OAUTH_TOKEN` | `anthropic` | `oauth_token` | `Authorization: Bearer`, with the OAuth header set and the billing block of `summaries.md` |
| `ANTHROPIC_API_KEY` | `anthropic` | `api_key` | `x-api-key` and `anthropic-version` |
| `OPENAI_API_KEY` | `openai` | `api_key` | `Authorization: Bearer` |

1. A variable that is empty or that holds only white space counts as not set.
2. If `ANTHROPIC_API_KEY` and `CLAUDE_CODE_OAUTH_TOKEN` are both set, the `anthropic` provider uses `api_key`.
3. A subscription OAuth token works with Haiku only. If the active `anthropic` kind is `oauth_token` and the summary provider is `anthropic`, a summary model other than `claude-haiku-5-5` stops `loupe serve` and `loupe summarize` at start. The actionable error names `CLAUDE_CODE_OAUTH_TOKEN`, `ANTHROPIC_API_KEY`, and `LOUPE_SUMMARY_MODEL`.
4. A log line, the `loupe config show` output, and an API response never hold a credential value or a part of it.

## Repository secrets

The GitHub repository holds two secrets for its workflows. The binary never reads them.

| Secret | Credential of | Read by |
|---|---|---|
| `RELEASE_PLEASE_TOKEN` | release-please and the writes of the dependency autofix | the release-please workflow and the step of `deps.autofix.sh` |
| `CLAUDE_CODE_OAUTH_TOKEN` | the headless `claude -p` run of the dependency autofix | only the step of `deps.autofix.sh` in the deps-autofix workflow |

1. The secret `CLAUDE_CODE_OAUTH_TOKEN` has the same name as the provider credential of the binary. It is a different value with a different use: the subscription token that the owner makes with `claude setup-token` for the workflow run.
2. No workflow file, script, log line, issue, pull request, or commit holds a secret value. A script that finds a secret missing names the secret, never its value.
3. `release.md` (Release token) and `dependencies.md` (Tokens) state the rotation of each secret.

## Provider base URLs

`ANTHROPIC_BASE_URL` (default `https://api.anthropic.com`) and `OPENAI_BASE_URL` (default `https://api.openai.com/v1`) set the base URL of each provider. The vendor SDKs read the same names. The OpenAI base URL includes `/v1`, and the Anthropic base URL does not. Each base URL must use `https`, except for a host of `127.0.0.1` or `localhost`. A different value stops the start with an actionable error that names the variable. This is the rule of `proxy.md` for an upstream.

## Provider detection and the summary provider

At start, the binary finds the detected providers. A provider is detected when at least one of its credentials is set. `loupe serve` writes one log line at start with the detected providers and the summary provider. The line holds no credential.

The summary provider is selected from the detected providers in this order:

1. `LOUPE_SUMMARY_PROVIDER` (`anthropic` or `openai`), if it is set.
2. `anthropic`, if it is detected.
3. `openai`, if it is detected.
4. Else none.

A `LOUPE_SUMMARY_PROVIDER` that names a provider that is not detected, or a value that is not known, stops the start. The actionable error names `LOUPE_SUMMARY_PROVIDER` and the detected providers. A detected provider enables the summaries with no other step. With no detected provider, the summaries are disabled. `LOUPE_SUMMARY_MODEL` has no default for `openai`. If the summary provider is `openai` and the model is not set, the start stops with an actionable error that names `LOUPE_SUMMARY_MODEL`. `summaries.md` gives the full rules.

## Provider ping interval

`LOUPE_PROVIDER_PING_SECS` is the interval of the provider ping in seconds. The default is `300` and the minimum is `60`. A value under `60`, or a value that is not an integer, stops the start with an actionable error that names `LOUPE_PROVIDER_PING_SECS`. `operations.md` gives the ping rules.

## Removed names

The binary does not read `LOUPE_SUMMARY_API_KEY`, `LOUPE_SUMMARY_OAUTH_TOKEN`, or `LOUPE_SUMMARY_BASE_URL`. If one of them is set and not empty, `loupe serve`, `loupe summarize`, and `loupe config show` stop at start with an actionable error. The error:

1. Names the set legacy variable and its new name: `ANTHROPIC_API_KEY`, `CLAUDE_CODE_OAUTH_TOKEN`, or `ANTHROPIC_BASE_URL`.
2. Gives the fix: rename the variable in `.env`.
3. States what to verify: `loupe config show` prints the provider line.

The error never prints the value. `LOUPE_SUMMARY_MODEL`, `LOUPE_SUMMARY_INTERVAL_SECS`, and `LOUPE_SUMMARY_MAX_ATTEMPTS` keep their names, because they configure the summarizer and not a provider. A set value of one of these names does not stop the start.

## Summary variables

`LOUPE_SUMMARY_MIN_CONVERSATION_CHARS` is the minimum count of characters of the conversation transcript. Below it, the transcript also holds the clipped tool inputs and tool results. The value is an integer of 0 or more. A bad value stops the start of `loupe serve` and of `loupe backfill` with exit code 2 and an actionable error that names the variable and the valid range.

## Day variables

`LOUPE_TZ` is an IANA time zone name, for example `Europe/Stockholm`. The default is `UTC`. A value from the environment or from `.env` keeps its effect. `LOUPE_PUBLISH_AT` has the format `HH:MM`, with an hour from 00 to 23 and a minute from 00 to 59. `LOUPE_PUBLISH_INTERVAL_SECS` is the interval of the publisher in seconds. `LOUPE_DAY_REVIEW` is `auto` or `manual`. A value that is not valid stops the start with an actionable error that names the variable and an example. `days.md` gives the full rules.

## Admin token

`LOUPE_ADMIN_TOKEN` is not required. If it is set and has fewer than 32 characters, `loupe serve` stops before it opens the port with an actionable error. If it is not set or is empty, the private page is off. `private-reports.md` gives the rules.

## Site title

`LOUPE_SITE_TITLE` sets the title of the dashboard page. The default is `Agentloupe`, the product name. A value of the variable replaces it. The loader removes the white space at the start and the end of the value. A value that is empty after this, or that is longer than 80 characters, stops the start with an actionable error that names `LOUPE_SITE_TITLE`. `dashboard.md` gives the use of the title.

## Live variables

`LOUPE_LIVE_ENABLED` is `true` or `false`. `LOUPE_LIVE_ALLOWED_ORIGIN` is one origin. `LOUPE_LIVE_MAX_RATE` is 1 to 50, `LOUPE_LIVE_MAX_CONNECTIONS` is 1 to 1024, and `LOUPE_LIVE_BUFFER` is 64 to 65536. A value out of range stops the start with an actionable error that names the variable. `live.md` gives the full rules.

## Subscription variables

`LOUPE_SUBSCRIPTION_<NAME>` is a prefix, not one variable. Each variable with this prefix declares one subscription. `<NAME>` matches `[A-Z0-9_]+`. The value is a non-negative decimal in US dollars per month, with at most 2 fraction digits. A name or a value that is not valid stops the start with an actionable error. `cost.md` gives the full rules.

When stdout is not a terminal, the output is one JSON object per line. Under `--human`, or on a terminal, the output is a table.

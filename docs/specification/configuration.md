<!-- @agents-index How the loupe binary resolves, validates, and shows its configuration values and their sources. -->
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
| `LOUPE_LISTEN` | no | no | `0.0.0.0:8787` in the container |
| `LOUPE_DATA_DIR` | no | no | `/data` in the container, `./data` outside it |
| `LOUPE_DATA_KEY` | yes | yes | none |
| `LOUPE_ACCOUNT_KEY` | yes | yes | none |
| `LOUPE_BOOTSTRAP_TOKEN` | no | yes | none |
| `LOUPE_GITHUB_TOKEN_<LABEL>` | no | yes | none |
| `LOUPE_SUMMARY_API_KEY` | no | yes | none |
| `LOUPE_SUMMARY_OAUTH_TOKEN` | no | yes | none |
| `LOUPE_SUMMARY_MODEL` | no | no | `claude-haiku-5-5` |
| `LOUPE_SUMMARY_BASE_URL` | no | no | `https://api.anthropic.com` |
| `LOUPE_SUMMARY_INTERVAL_SECS` | no | no | `30` |
| `LOUPE_SUMMARY_MAX_ATTEMPTS` | no | no | `5` |
| `LOUPE_TZ` | no | no | `Europe/Stockholm` |
| `LOUPE_PUBLISH_AT` | no | no | `01:00` |
| `LOUPE_PUBLISH_INTERVAL_SECS` | no | no | `60` |
| `LOUPE_IDLE_SECS` | no | no | `3600` |
| `LOUPE_PARSE_INTERVAL_SECS` | no | no | `5` |
| `LOUPE_RAW_RETENTION_HOURS` | no | no | `168` |
| `LOUPE_SITE_TITLE` | no | no | `AI agent activity` |
| `LOUPE_PUBLIC_PROJECTS` | no | no | empty |
| `LOUPE_DAY_REVIEW` | no | no | `auto` |
| `LOUPE_SUBSCRIPTION_<NAME>` | no | no | none |
| `LOUPE_ADMIN_TOKEN` | no | yes | none |
| `LOUPE_UPSTREAMS_FILE` | no | no | `upstreams.toml` in the working directory |
| `LOUPE_CAPTURE_MAX_BYTES` | no | no | `33554432` |
| `LOUPE_CAPTURE_QUEUE` | no | no | `256` |
| `LOUPE_UPSTREAM_CONNECT_TIMEOUT_SECS` | no | no | `10` |
| `LOUPE_UPSTREAM_HEADERS_TIMEOUT_SECS` | no | no | `120` |

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
5. `summary_credential`, the active credential kind of the summarizer: `api_key`, `oauth_token`, or `none`.
6. `LOUPE_TZ`, `LOUPE_PUBLISH_AT`, `LOUPE_PUBLISH_INTERVAL_SECS`, and `LOUPE_DAY_REVIEW`, each with its source.
7. The next due publish time, in `LOUPE_TZ` and in UTC.

## Summary credential

`LOUPE_SUMMARY_API_KEY` and `LOUPE_SUMMARY_OAUTH_TOKEN` are the two credentials of the summarizer. Either one enables the summaries. If both are set, `api_key` is active. A model other than `claude-haiku-5-5` needs `LOUPE_SUMMARY_API_KEY`. `LOUPE_SUMMARY_BASE_URL` must use `https`, except for a loopback host. `summaries.md` gives the full rules.

## Day variables

`LOUPE_TZ` is an IANA time zone name, for example `Europe/Stockholm`. `LOUPE_PUBLISH_AT` has the format `HH:MM`, with an hour from 00 to 23 and a minute from 00 to 59. `LOUPE_PUBLISH_INTERVAL_SECS` is the interval of the publisher in seconds. `LOUPE_DAY_REVIEW` is `auto` or `manual`. A value that is not valid stops the start with an actionable error that names the variable and an example. `days.md` gives the full rules.

## Subscription variables

`LOUPE_SUBSCRIPTION_<NAME>` is a prefix, not one variable. Each variable with this prefix declares one subscription. `<NAME>` matches `[A-Z0-9_]+`. The value is a non-negative decimal in US dollars per month, with at most 2 fraction digits. A name or a value that is not valid stops the start with an actionable error. `cost.md` gives the full rules.

When stdout is not a terminal, the output is one JSON object per line. Under `--human`, or on a terminal, the output is a table.

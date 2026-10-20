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
| `LOUPE_TZ` | no | no | `Europe/Stockholm` |
| `LOUPE_PUBLISH_AT` | no | no | `01:00` |
| `LOUPE_IDLE_SECS` | no | no | `3600` |
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

When stdout is not a terminal, the output is one JSON object per line. Under `--human`, or on a terminal, the output is a table.

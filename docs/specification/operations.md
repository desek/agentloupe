<!-- @agents-index How the loupe binary runs: subcommands, logging, storage, health, shutdown, container, and the local pipeline verdict. -->
# Operations

## Subcommands

| Subcommand | Behaviour |
|---|---|
| `loupe serve` | Loads the configuration, opens the database, applies the migrations, then listens on `LOUPE_LISTEN`. |
| `loupe gen-key` | Prints one new random 32-byte key in standard base64 with padding, then a newline. The key comes from the random source of the operating system. The subcommand stores nothing. |
| `loupe config show` | Prints the effective configuration. See `configuration.md`. |

Each subcommand has a complete `--help`.

## Logging

The binary writes each log event as one line to stderr and to `<LOUPE_DATA_DIR>/logs/loupe.log`. Each line in the file is one JSON object. Each line has the time in UTC, the level, the source location as `path/file.rs:LINE`, and the message with its key fields. No log line contains a secret value. A normal start with valid configuration logs no warning and no error.

## Storage

The binary stores its data in the SQLite file `<LOUPE_DATA_DIR>/loupe.db`. The journal mode is WAL.

If the capture queue is full, the binary writes each sealed exchange record as one file in `<LOUPE_DATA_DIR>/spill/`. See `proxy.md`.

At start, the binary applies the versioned migrations from `migrations/` in order. It records each applied version in a version table in the database. A second start applies no migration again.

## Health

`GET /healthz` returns status 200 and the body `{"status":"ok"}`. The route answers only after the database is open and migrated. The route has no CORS header.

## Shutdown

On SIGTERM or SIGINT, `loupe serve` stops new connections. It lets open requests finish within 10 seconds. Then it closes the database and exits with code 0.

## Container

The `Dockerfile` has two stages. The build stage compiles a release binary with the toolchain that `rust-toolchain.toml` pins. The runtime stage holds only the binary and its runtime files. It runs as a user that is not root. Its `HEALTHCHECK` calls `GET /healthz`.

The runtime image is `debian:bookworm-slim`. It has the same C library as the build stage, and it has `bash`. The `HEALTHCHECK` uses the `/dev/tcp` feature of `bash`, so the image needs no `curl`. A smaller distroless image has no shell, so it cannot run a `HEALTHCHECK` without an extra program. The runtime user is `loupe` with UID and GID `10001`.

`compose.yaml` has one service. It binds the port as `127.0.0.1:8787:8787`. It mounts a named volume at `/data`, reads the secrets from `.env`, and sets `LOUPE_ENV`. `.env.example` lists each variable name that `compose.yaml` passes, with no value.

### An old volume that the runtime user cannot write

The image makes `/data` and gives it to the runtime user. Docker copies this ownership to a new named volume. A volume that an earlier image made can have a different owner. Then the binary cannot write the database or the log file.

To fix the volume, do these steps:

1. Stop the service with `docker compose down`.
2. Change the owner of the volume to the runtime user. For example: `docker compose run --rm --user root --entrypoint chown loupe -R <uid>:<gid> /data`, where `<uid>:<gid>` is the runtime user of the image.
3. Start the service with `docker compose up -d`.
4. Verify that `GET /healthz` returns status 200.

## Pipeline

The pipeline entry points are in `.agents/scripts/`. Each entry point does one stage:

| Entry point | Stage |
|---|---|
| `pipeline.format.sh` | `cargo fmt --check` |
| `pipeline.lint.sh` | `cargo clippy` with warnings as errors |
| `pipeline.test.sh` | `cargo test` |
| `pipeline.image.sh` | Builds the container image. `--dry-run` prints the build command and does not run it. |

Each entry point has `--help`, a machine output by default, and `--human` for the friendly view. It writes actionable errors to stderr and returns documented exit codes. The entry points read the toolchain version from `rust-toolchain.toml` and state no version of their own.

`pipeline.verdict.sh` runs the stages in the order format, lint, test, image. It only calls the stage entry points. It prints one line for each stage with `status: passed` or `status: failed`, and the time of the stage. It also prints the total time. It returns a non-zero exit code if a stage fails. The stages format, lint, and test are hermetic: they use no network after the dependencies are fetched, and no assertion reads the wall clock.

## Measurable targets

| Metric | Method | Baseline | Threshold |
|---|---|---|---|
| Verdict result | `.agents/scripts/pipeline.verdict.sh; echo $?` | exit code 0, every stage `status: passed`, measured on 2026-10-20 | exit code 0, every stage `status: passed` |
| Time to a full verdict, warm cache | total time that `pipeline.verdict.sh` prints, second run | 1.9 seconds, measured on 2026-10-20 on Apple silicon | at most 180 seconds |
| Time from container start to healthy | `docker compose up -d && time sh -c 'until curl -sf http://127.0.0.1:8787/healthz; do sleep 0.2; done'` | 0.02 seconds to the first 200 from `/healthz`, and 1.3 seconds to the `healthy` status of Docker, measured on 2026-10-20 on Apple silicon | at most 5 seconds |
| Added time to the first byte through the proxy, 95th percentile | `.agents/scripts/proxy.latency.sh --requests 200`, which starts the fake upstream, sends 200 server-sent events requests direct and through the proxy, and prints `p95_added_ms` | `p95_added_ms` 0.33 (0.74 direct, 1.07 through the proxy), measured on 2026-10-20 on Apple silicon; re-measured 0.36 (0.80 direct, 1.16 through the proxy) on 2026-10-20 | `p95_added_ms` at most 20 |
| Secrets on disk | `grep -a -c <test key value> <data dir>/loupe.db <data dir>/loupe.db-wal <data dir>/logs/loupe.log` after a request with a known `x-api-key` | 0 matches in each file, measured on 2026-10-20 | 0 matches in each file |
| Capture during abort | `cargo test --test proxy_abort` | pass, 2 of 2 tests, measured on 2026-10-20 | pass |
| Added time to the first byte of the token gate, 95th percentile | `.agents/scripts/proxy.latency.sh --requests 200`, then `.agents/scripts/proxy.latency.sh --requests 200 --with-token` with 100 tokens in the table | to be measured; the last `p95_added_ms` with no token was 0.36 | the difference of the two `p95_added_ms` at most 2 |
| Raw token leak | `loupe token create --name leak-test` into `T`, send 10 requests with `T`, then `grep -c "$T" -r <data dir>` | none | 0 |

The time to a full verdict is the time of the local loop. The first measured value at the end of the foundation change is the baseline. A later change that makes it longer must state the reason.

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
| `pipeline.web.sh` | Checks that `node --version` equals the pin in `.tool-versions`, runs `node --test` on the calculation modules of the page, and checks the size of `web/` with `web.size.sh`. `--check-toolchain` does only the check of the pin. |
| `pipeline.image.sh` | Builds the container image. `--dry-run` prints the build command and does not run it. |

Each entry point has `--help`, a machine output by default, and `--human` for the friendly view. It writes actionable errors to stderr and returns documented exit codes. The entry points read the toolchain version from `rust-toolchain.toml` and state no version of their own.

`pipeline.verdict.sh` runs the stages in the order format, lint, test, web, image. It only calls the stage entry points. It prints one line for each stage with `status: passed` or `status: failed`, and the time of the stage. It also prints the total time. It returns a non-zero exit code if a stage fails. The entry point `pipeline.web.sh` reads the Node version from `.tool-versions`. If `node` is missing or its version differs from the pin, it stops with a non-zero exit code and an actionable error. The thin caller installs Node from `.tool-versions` and states no Node version. The stages format, lint, test, and web are hermetic: they use no network after the dependencies are fetched, and no assertion reads the wall clock.

## Measurable targets

| Metric | Method | Baseline | Threshold |
|---|---|---|---|
| Verdict result | `.agents/scripts/pipeline.verdict.sh; echo $?` | exit code 0, every stage `status: passed`, measured on 2026-10-20, re-measured on 2026-10-20 (18.5 seconds total) | exit code 0, every stage `status: passed` |
| Time to a full verdict, warm cache | total time that `pipeline.verdict.sh` prints, second run | 1.9 seconds, measured on 2026-10-20 on Apple silicon | at most 180 seconds |
| Time from container start to healthy | `docker compose up -d && time sh -c 'until curl -sf http://127.0.0.1:8787/healthz; do sleep 0.2; done'` | 0.02 seconds to the first 200 from `/healthz`, and 1.3 seconds to the `healthy` status of Docker, measured on 2026-10-20 on Apple silicon | at most 5 seconds |
| Added time to the first byte through the proxy, 95th percentile | `.agents/scripts/proxy.latency.sh --requests 200`, which starts the fake upstream, sends 200 server-sent events requests direct and through the proxy, and prints `p95_added_ms` | `p95_added_ms` 0.33 (0.74 direct, 1.07 through the proxy), measured on 2026-10-20 on Apple silicon; re-measured 0.36 (0.80 direct, 1.16 through the proxy) on 2026-10-20 | `p95_added_ms` at most 20 |
| Secrets on disk | `grep -a -c <test key value> <data dir>/loupe.db <data dir>/loupe.db-wal <data dir>/logs/loupe.log` after a request with a known `x-api-key` | 0 matches in each file, measured on 2026-10-20 | 0 matches in each file |
| Capture during abort | `cargo test --test proxy_abort` | pass, 2 of 2 tests, measured on 2026-10-20 | pass |
| Added time to the first byte of the token gate, 95th percentile | `.agents/scripts/proxy.latency.sh --requests 200`, then `.agents/scripts/proxy.latency.sh --requests 200 --with-token` with 100 tokens in the table | `p95_added_ms` 0.30 with no token and 0.24 with a token, a difference of -0.06, measured on 2026-10-20 on Apple silicon; re-measured 0.26 with no token and 0.40 with a token, a difference of 0.14, on 2026-10-20; re-measured 0.28 with no token and 0.28 with a token, a difference of 0.00, on 2026-10-20 | the difference of the two `p95_added_ms` at most 2 |
| Raw token leak | `loupe token create --name leak-test` into `T`, send 10 requests with `T`, then `grep -c "$T" -r <data dir>` | 0 matches in the data dir and on stderr after 10 requests, measured on 2026-10-20; re-measured 0 on 2026-10-20 | 0 |
| Parse time, 1000 exchanges | `.agents/scripts/parser.bench.sh --exchanges 1000`, which makes mixed JSON and SSE exchanges through the proxy against the fake upstream, times `loupe parse --once`, and prints `parse_secs` | `status: not_implemented` before the parser, measured on 2026-10-20; `parse_secs` 0.20 for 1000 exchanges with the local release binary on Apple silicon, measured on 2026-10-20 | `parse_secs` at most 10 |
| Real call capture | the scenario `.agents/scenarios/parser.real-agents.md` with Claude Code and Pi | 0 rows in `calls`; measured 4 rows for Claude Code 2.1.304 and 1 row for Pi 1.3.0, each above 0, on 2026-10-20 | 1 or more rows in `calls` for each agent, each with `input` plus `cache_read` plus `output` above 0 |
| Schema validation of the price file | `cargo test price_file_matches_schema` | none; passed on 2026-10-20 | passes |
| Cost of the fixture calls | `cargo test --test price_fixtures` | none; 3 of 3 fixture costs equal on 2026-10-20 | every expected cost equal to the value computed by hand, as decimal text |
| Index load and price time | `.agents/scripts/cost.bench.sh` | none; load 19.2 milliseconds for 4218 entries and price 0.96 microseconds median, release build on Apple silicon, measured on 2026-10-20 | load at most 500 milliseconds, price of one call at most 50 microseconds median |
| Increase of the verdict time | total time that `pipeline.verdict.sh` prints, warm cache, compared to 18.5 seconds | 18.5 seconds; 21.9 seconds on 2026-10-20, an increase of 3.4 seconds because the test stage compiles the embedded price file, the jsonschema dev-dependency, and the new price tests; 42.8 seconds on 2026-10-20 after the summaries, an increase of 20.9 seconds because the new summary and retention integration tests wait 2.4 seconds per closed segment for the idle close, and the test stage compiles four more test binaries and chrono-tz | an increase above 10 seconds has a stated reason in this table |
| Scrubber leak rate | `cargo test --test summary_scrub_corpus`, over a corpus of at least 50 lines, prints `leaks` | none, a new measurement; `leaks: 0` over 52 lines on 2026-10-20, re-measured `leaks: 0` on 2026-10-20 | `leaks: 0` |
| Live summary of a real session | `.agents/scripts/summary.live.sh` | none; `status: done`, `failed: 0`, a line of 8 words, Claude Code 2.1.304, OAuth token, on 2026-10-20; re-measured `status: done`, `failed: 0`, a line of 9 words, on 2026-10-20 | `status: done`, and no `failed` |
| Bodies after retention | `LOUPE_RAW_RETENTION_HOURS=0`, then `loupe summarize --once`, then count the exchanges with a call and no `bodies_deleted_at` | the count of exchanges before the retention; `bodies_left: 0` after the live run, with the calls and sessions rows unchanged, on 2026-10-20; re-measured `bodies_left: 0`, calls and sessions unchanged, on 2026-10-20 | `0` |
| Proxy time to first byte with the summary and retention workers, 95th percentile | `.agents/scripts/proxy.latency.sh --requests 200 --with-token` | the value before the two workers: median 0.71 of 5 runs (-0.01 to 0.88) on 2026-10-20; with the two workers, median 0.73 of 9 runs (-0.09 to 2.75) on 2026-10-20, a difference of 0.02; the single runs vary by about 3 milliseconds on this machine; re-measured `p95_added_ms` 0.14 on 2026-10-20 | `p95_added_ms` at most 20, and at most 2 above the baseline |
| Build time of a day, 10000 calls | `.agents/scripts/days.bench.sh --calls 10000` prints `build_secs` | none, a new measurement, added before the builder; `build_secs` 0.03 with the release binary on Apple silicon on 2026-10-20; re-measured 0.03 on 2026-10-20 | `build_secs` at most 2 |
| API latency, 400 days | `.agents/scripts/api.bench.sh --days 400 --requests 200` prints `p95_ms` | none, a new measurement, added before the routes; `p95_ms` 1.53 with the release binary on Apple silicon on 2026-10-20; re-measured 1.17 on 2026-10-20 | `p95_ms` at most 50 |
| Same-day record of a real session | the scenario `.agents/scenarios/days.same-day.md`: a real session through the proxy, then `loupe aggregate --date today`, then `curl -s http://127.0.0.1:8787/v1/days/<today>` | `404` before the routes; `200` with `calls` 1 and `by_agent` key `claude-code` on 2026-10-20, also in the container on 2026-10-20 | `200` with `calls` 1 or more and `by_agent` with the key `claude-code` |
| Privacy of the API | `cargo test --test api_allowlist` | none; passed, 2 of 2 tests, on 2026-10-20, again on 2026-10-20 | passes |
| Proxy time to first byte with the queue writes and the publisher, 95th percentile | `.agents/scripts/proxy.latency.sh --requests 200 --with-token` | the value before the queue writes: `p95_added_ms` 0.14 on 2026-10-20; with the queue writes and the publisher, `p95_added_ms` 0.23 on 2026-10-20, a difference of 0.09; re-measured 0.21 on 2026-10-20, a difference of 0.07 | `p95_added_ms` at most 20, and at most 2 above the baseline |
| Verdict time after the day records and the API | total time that `pipeline.verdict.sh` prints, warm cache | 42.8 seconds after the summaries; 45.3 seconds on 2026-10-20, an increase of 2.5 seconds, because the test stage compiles and runs ten more test binaries, and the integration tests of `loupe day` wait 2.5 seconds for the publisher; re-measured 45.4 seconds, exit code 0, every stage `status: passed`, on 2026-10-20 | an increase above 10 seconds has a stated reason in this table |
| Node pin | `.agents/scripts/pipeline.web.sh --check-toolchain; echo $?` | exit code 0 with node v22.20.0 on 2026-10-20; re-measured exit code 0 on 2026-10-20 at verification | exit code 0 when `node --version` equals the entry in `.tool-versions`, non-zero otherwise |
| Size of the page files | `.agents/scripts/web.size.sh` prints `code_kib` and `total_kib` | `code_kib` 41.6 and `total_kib` 184.7 on 2026-10-20; re-measured `code_kib` 41.6 and `total_kib` 184.7 on 2026-10-20 at verification | `code_kib` at most 100 (without the fonts), `total_kib` at most 400 (with the fonts) |
| First full draw, 400 days | `.agents/scripts/web.render.sh --days 400` prints `draw_ms` | 81.7 on 2026-10-20; re-measured 53 on 2026-10-20 at verification | `draw_ms` at most 500 |
| Headline size | `.agents/scripts/web.render.sh --days 400` prints `headline_ratio`, and `headline_above_fold` at 360 px and at 1440 px | `headline_ratio` 2.12, `headline_above_fold: true` at 360 px and at 1440 px, on 2026-10-20; re-measured `headline_ratio` 2.12, `headline_above_fold: true` at 360 px and at 1440 px, on 2026-10-20 at verification | `headline_ratio` at least 2.0, `headline_above_fold: true` at both widths |
| Remote requests of the page | `.agents/scripts/web.render.sh --days 400` prints `remote_requests` | `remote_requests: 0` on 2026-10-20; re-measured `remote_requests: 0` on 2026-10-20 at verification | `remote_requests: 0` |
| Proxy time to first byte with the static routes, 95th percentile | `.agents/scripts/proxy.latency.sh --requests 200 --with-token` | the value after the day records and the API: `p95_added_ms` 0.21 on 2026-10-20; re-measured `p95_added_ms` 0.31 on 2026-10-20 at verification, a difference of 0.10 | `p95_added_ms` at most 20, and at most 2 above the baseline |
| Verdict time with the web stage | total time that `pipeline.verdict.sh` prints, warm cache, second run | 45.4 seconds after the day records and the API; 50.5 seconds and 49.3 seconds, exit code 0, every stage `status: passed`, on 2026-10-20; re-measured 49.5 seconds and 50.7 seconds (second run), exit code 0, every stage `status: passed`, on 2026-10-20 at verification; with the GitHub worker 112.5 seconds (first run, new image) and 59.1 seconds (second run), exit code 0, every stage `status: passed`, on 2026-10-20; re-measured 64.8 seconds, exit code 0, every stage `status: passed`, on 2026-10-20 at verification. The increase of about 9 seconds comes from the new GitHub test binaries and the 3 seconds of waits in the rate limit test | at most 180 seconds, every stage `status: passed`, with the stage `web` |
| GitHub fetch time, 50 merged pull requests | `.agents/scripts/github.bench.sh --merged 50` prints `fetch_secs`, against the local fake GitHub | `fetch_secs` 0.04 on 2026-10-20; re-measured `fetch_secs` 0.03 on 2026-10-20 at verification | `fetch_secs` at most 10 |
| Privacy of the GitHub tables | `cargo test --test github_privacy` | passed on 2026-10-20; passed again on 2026-10-20 at verification | passes |
| Proxy time to first byte with the GitHub worker, 95th percentile | `.agents/scripts/proxy.latency.sh --requests 200 --with-token` | the value after the dashboard: `p95_added_ms` 0.31; with the GitHub worker `p95_added_ms` 0.28 on 2026-10-20; re-measured 0.29 on 2026-10-20 at verification, a difference of -0.02 | at most 2 ms above the baseline |

The time to a full verdict is the time of the local loop. The first measured value at the end of the foundation change is the baseline. A later change that makes it longer must state the reason.

<!-- @agents-index How the loupe binary runs: subcommands, logging, storage, health, shutdown, container, and the local pipeline verdict. -->
# Operations

## Subcommands

| Subcommand | Behaviour |
|---|---|
| `loupe serve` | Loads the configuration, opens the database, applies the migrations, then listens on `LOUPE_LISTEN`. |
| `loupe gen-key` | Prints one new random 32-byte key in standard base64 with padding, then a newline. The key comes from the random source of the operating system. The subcommand stores nothing. |
| `loupe config show` | Prints the effective configuration. See `configuration.md`. |
| `loupe backfill` | Reads the session logs of Claude Code or Pi into the calls and the sessions. See `backfill.md`. |

Each subcommand has a complete `--help`.

## Logging

The binary writes each log event as one line to stderr and to `<LOUPE_DATA_DIR>/logs/loupe.log`. Each line in the file is one JSON object. Each line has the time in UTC, the level, the source location as `path/file.rs:LINE`, and the message with its key fields. No log line contains a secret value. A normal start with valid configuration logs no warning and no error.

## Storage

The binary stores its data in the SQLite file `<LOUPE_DATA_DIR>/loupe.db`. The journal mode is WAL.

If the capture queue is full, the binary writes each sealed exchange record as one file in `<LOUPE_DATA_DIR>/spill/`. See `proxy.md`.

At start, the binary applies the versioned migrations from `migrations/` in order. It records each applied version in a version table in the database. A second start applies no migration again.

## Health

`GET /healthz` returns status 200 and the body `{"status":"ok"}`. The route answers only after the database is open and migrated. The route carries `access-control-allow-origin: *`, as `api.md` states. The provider status never changes the status or the body of `GET /healthz`. A provider that is down does not make the container unhealthy, because the proxy works without the summarizer.

## Provider ping

The ping worker keeps a provider status for each known provider. It runs in its own task and does not delay the proxy, the parser, or the start of the HTTP listener.

1. The worker pings once at start, then every `LOUPE_PROVIDER_PING_SECS` seconds (default `300`, minimum `60`). It sends at most one request for each provider in each interval.
2. The ping is `GET {base_url}/v1/models` for `anthropic` and `GET {base_url}/models` for `openai`. It never calls a model and spends no tokens. `summaries.md` gives the ping headers.
3. The timeout of a ping is 5 seconds.
4. A `2xx` gives `ok`. A `401` or a `403` gives `unauthorized`. A network error, a timeout, or a different status gives `unreachable`. A provider with no credential gives `not_configured` and the worker sends no request.
5. Each status has the time `checked_at`, in UTC, RFC 3339.
6. The worker keeps the status in a cache. A read of the status uses the cache and never starts a ping. `GET /private/api/providers` (`private-reports.md`) shows the status.

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

## Connect the agents

The connect script `.agents/scripts/agent.connect.sh` points one agent at the proxy for one shell. It changes no global setting of the agent.

Inputs:

- The agent: Claude Code or Pi.
- The proxy token: a file that holds the token, or an environment variable. The script does not accept the token as an argument value, because the shell history and the process list keep arguments.
- The base of the proxy. The default is `http://127.0.0.1:8787`.

If no token is given, the script stops with a non-zero exit code. The error names the missing input, the command that makes a token, and the command that lists the tokens.

Output:

- For Claude Code, the script prints one export line that sets the base URL of Anthropic to the Claude Code route of the proxy with the token. It also prints the hint for the model `claude-haiku-5-5`.
- For Pi, the script makes the isolated agent directory under the state directory of the user, with mode 0700. It writes one file of model settings into it, with mode 0600. That file sets only the base URL of the Anthropic provider to the Pi route of the proxy with the token. The script prints one export line that points Pi at this directory.

The isolated agent directory holds no credential file. Pi gets its credential from the environment of the user. The plain route is a subscription token that the user makes one time with `claude setup-token` and exports in the shell. The alternative route is a Pi extension that supplies the credential, for example an extension that reads the Claude Code login from the keychain.

The script never writes to the global settings directory of Claude Code or of Pi, and never writes to a file in the repository. It never reads, copies, or prints a provider credential.

The script is idempotent. A second run with the same inputs writes the same bytes and prints the same output. With `--dry-run`, it prints the planned file path and content with the token masked as `lpt_…`, and writes nothing. With `--check`, it sends one request to the health route of the proxy, and stops with an actionable error if the container does not answer. The script runs with the bash 3.2 of macOS, the default tools of macOS, and curl. It does not need jq.

To see a day before the nightly publish, run the aggregate subcommand for that date in the container. A short idle time in an untracked compose override closes the sessions sooner than the default of 3600 seconds.

## Measurable targets

| Metric | Method | Baseline | Threshold |
|---|---|---|---|
| Verdict result | `.agents/scripts/pipeline.verdict.sh; echo $?` | exit code 0, every stage `status: passed`, measured on 2026-10-20, re-measured on 2026-10-20 (18.5 seconds total), re-measured on 2026-10-21 (exit code 0, every stage `status: passed`) | exit code 0, every stage `status: passed` |
| Provider ping requests to a model | `cargo test provider_ping_never_calls_a_model` | 0 requests to `/v1/messages` or `/chat/completions`, measured on 2026-10-21, re-measured 0 at verification on 2026-10-21 | 0 requests to `/v1/messages` or `/chat/completions` |
| Credential leak | `cargo test config_show_never_prints_a_credential`, `cargo test provider_log_never_holds_a_credential`, and `cargo test private_config_never_holds_a_credential` | 0 occurrences, measured on 2026-10-21, re-measured 0 at verification on 2026-10-21 | 0 occurrences of the test secret |
| Read time of the provider status and of the private configuration, 95th percentile | `cargo test read_is_cached -- --nocapture` (1000 reads each) | 0.33 ms for the provider status route, 0.79 ms for the private configuration route, and under 0.01 ms for the cache, measured on 2026-10-21 on Apple silicon; re-measured 0.38 ms, 0.79 ms, and 0.005 ms at verification on 2026-10-21 | under 5 ms |
| Ping timeout | `cargo test provider_ping_times_out_unreachable` | `unreachable` in 5.02 seconds, measured on 2026-10-21 on Apple silicon; re-measured 5.00 seconds at verification on 2026-10-21 | status `unreachable` in at most 5.5 seconds |
| Time to a full verdict, warm cache | total time that `pipeline.verdict.sh` prints, second run | 1.9 seconds, measured on 2026-10-20 on Apple silicon; re-measured 101.4 seconds on 2026-10-21 (183.4 seconds on the first run after a cold image build); re-measured 105.3 seconds at verification on 2026-10-21 | at most 180 seconds |
| Time from container start to healthy | `docker compose up -d && time sh -c 'until curl -sf http://127.0.0.1:8787/healthz; do sleep 0.2; done'` | 0.02 seconds to the first 200 from `/healthz`, and 1.3 seconds to the `healthy` status of Docker, measured on 2026-10-20 on Apple silicon; re-measured 0.24 seconds from `docker compose up -d` to the first 200 on 2026-10-21 | at most 5 seconds |
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
| Scrubber leak rate | `cargo test --test summary_scrub_corpus -- --nocapture`, over a corpus of at least 50 lines, prints `leaks` | none, a new measurement; `leaks: 0` over 52 lines on 2026-10-20, re-measured `leaks: 0` on 2026-10-20 | `leaks: 0` |
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

| Acceptance pass rate | Run the acceptance scenario `.agents/scenarios/acceptance.macos.end-to-end.md` three times, and count the runs that meet the success condition | 0 of 0, no run exists; 3 of 3 on 2026-10-21, Claude Code 2.1.306 and Pi 1.3.0; an independent fourth run passed on 2026-10-21, so 4 of 4 | 3 of 3 |
| Time from the last agent call to the day record | `date +%s` after the last agent call returns, then the aggregate subcommand, then poll `/v1/days/<today>` until the agent breakdown holds both agents; report the difference | not measured; 74, 73, and 73 seconds on 2026-10-21; 73 seconds in the independent fourth run | at most 300 seconds with an idle time of 60 seconds |
| Secret leak count | grep for `lpt_[A-Za-z0-9_-]{5,}` and `sk-ant-` over the scenario files and the container logs | not measured; 0 on 2026-10-21, over the scenario files and the container logs of three runs; 0 again after the independent fourth run | 0 |
| Private acceptance criteria | `.agents/scripts/pipeline.verdict.sh` and the private browser scenario under `.agents/scenarios/` | no private route exists; on 2026-10-21 the verdict passed in 110.8 seconds and the browser scenario passed 1 of 1 | every criterion passes |
| Private report time, 400 days | `.agents/scripts/private.bench.sh` seeds 100 000 calls over 400 days, then prints the `time_total` of the report request with the bearer header | no private route exists; 0.088 seconds on 2026-10-21 | below 1.0 seconds |
| Backfill time, 10000 entries | `.agents/scripts/backfill.bench.sh --entries 10000 --dry-run` prints `backfill_secs` | none, a new measurement, added before the backfill; 0.46 seconds with `--dry-run` (re-measured), earlier 0.48 seconds with `--dry-run` and 0.39 seconds for a real run on 2026-10-21, release binary on this machine | `backfill_secs` at most 15 |
| Backfill idempotence | run `loupe backfill --source claude-code` two times on the fixtures | none; `calls_added: 0` on the second run on 2026-10-21, also for `--source pi` | `calls_added: 0` on the second run |
| Backfill of real logs | `loupe backfill --source claude-code --dry-run` and `--source pi --dry-run` on this machine | 0 calls with origin `log`; with the backfill the full verdict passed in 85.9 seconds (warm) on 2026-10-21; on 2026-10-21 the dry run gave `calls_added` 38088 of 274673 entries (re-measured; earlier 38070 of 274434) for `claude-code` and 10450 of 23282 entries for `pi`, `entries_skipped` 0 for each | `calls_added` above 0 for each source, `entries_skipped` at most 1 percent of `entries_read` |
| Size of the private page files | the size of the files under `web/private/`, without the shared fonts | no private page exists; 12.9 KiB on 2026-10-21 | at most 200 KiB |
The time to a full verdict is the time of the local loop. The first measured value at the end of the foundation change is the baseline. A later change that makes it longer must state the reason.
| Time of the local loop with the live feed | `.agents/scripts/pipeline.verdict.sh` | 85.9 seconds (warm) after the backfill | the full verdict passed in 86.1 seconds (warm) on 2026-10-21; the live tests add a heartbeat test of 15 seconds that runs in parallel |
| Verdict time with the session texts | `.agents/scripts/pipeline.verdict.sh`, warm cache, second run | 86.1 seconds after the live feed | 87.6 seconds, exit code 0, every stage `status: passed`, on 2026-10-21; the six new test files add about 1.5 seconds |
| Live event latency, exchange end to client, 95th percentile | `cargo test --test live_latency -- --nocapture` (1000 exchanges, local WebSocket client) | none, a new measurement; 96.5 milliseconds on 2026-10-21; re-measured 96.2 milliseconds on 2026-10-21 | at most 250 milliseconds |
| Proxy time to first byte with 32 live clients, 95th percentile | `.agents/scripts/proxy.latency.sh --requests 200 --with-token --live-clients 0`, then `--live-clients 32` | the value with 0 clients; on 2026-10-21, p95_added_ms 0.33 with 0 clients and 0.70 with 32 clients (`--live-clients 32`), a difference of 0.37 milliseconds; re-measured 0.29 and 0.27, a difference of -0.02 milliseconds, on 2026-10-21 | the difference at most 1 millisecond |
| Added live page code | `wc -c web/assets/js/live.*.js web/assets/css/live.css` | 0 bytes; 7748 bytes on 2026-10-21; re-measured 7748 bytes on 2026-10-21 | at most 12288 bytes |
| Resident memory of the live hub and 32 clients | `cargo test --test live_memory -- --nocapture` reads the resident memory before and after 32 clients | the value before; on 2026-10-21, 38288 KiB before and 42576 KiB after, a difference of 4288 KiB; re-measured 38192 KiB and 42368 KiB, a difference of 4176 KiB, on 2026-10-21 | the difference at most 8 MiB |
| Skipped rate of session day summaries, Claude Code | on a copy of the database, `loupe backfill --source claude-code` over the two-day window 2026-10-20 to 2026-10-21, then count `skipped` rows of `session_day_summaries` against all rows | 171 of 248 segments `skipped` (69 percent) under the per-segment summaries | at most 25 percent On 2026-10-21, with `.agents/scripts/summary.threshold.sh --from 2026-10-20 --to 2026-10-21`: 44 of 281 `skipped` (15.7 percent) at 200 characters, 44 of 281 (15.7 percent) at 400, 59 of 281 (21.0 percent) at 800. The selection rule gives 200: the lowest value under 25 percent, and 200 and 400 are within 2 points with the lower median characters at 200. The default of `LOUPE_SUMMARY_MIN_CONVERSATION_CHARS` is 200. Pi: 0 calls in the window before the change, so no Pi rows. Re-measured on 2026-10-21 at 200 characters: 46 of 282 `skipped` (16.3 percent), passed. Re-measured on 2026-10-21 after the session lines stopped: 39 of 282 `skipped` (13.8 percent), 3 `pending`, passed. |
| Summary request count, two-day window | the count of model requests that the fake upstream records, `loupe backfill` over the two-day window on a copy of the database | 249 requests (248 Claude Code segments and 1 Pi segment) | at most 2 times the count of distinct pairs of session key and day of a call On 2026-10-21 at 200 characters: 794 requests, of which 278 session lines, 279 full session summaries, and 237 session day summaries, against 281 pairs (threshold 562). The two session texts alone use 516 requests; the session lines of each segment stay and add 278. Re-measured on 2026-10-21: 793 requests (278 lines, 279 full, 236 day) against 282 pairs (threshold 564), failed. Re-measured on 2026-10-21 after the session lines stopped: 523 requests (0 lines, 279 full, 244 day) against 282 pairs (threshold 564), passed. |
| Characters per summary request | the maximum and the median of the request characters that the fake upstream records, same run | measured before the change | maximum at most 24000 plus the rule text and the full session summary; median reported On 2026-10-21 at 200 characters, over the full and day requests: maximum 25767, median 2426 (400: 25838 and 2488; 800: 25823 and 2576). Re-measured on 2026-10-21: maximum 25708, median 2403. Re-measured on 2026-10-21 after the session lines stopped: maximum 25847, median 2436. |
| Row counts of the session texts | `sqlite3` counts of `session_day_summaries` and `session_full_summaries` rows against the count of distinct pairs of session key and day of a call in `calls`, over the two-day window | no table | equal counts On 2026-10-21: 281 rows in each table against 281 pairs; 2 pending rows belong to segments that were still open. Re-measured on 2026-10-21: 282 rows in each table, 3 pending rows of open segments. |

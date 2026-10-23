<!-- @agents-index How the loupe binary runs: subcommands, logging, storage, health, the provider ping, the start request, shutdown, container, the local pipeline verdict, and the hosted verdict reuse. -->
# Operations

## Subcommands

| Subcommand | Behaviour |
|---|---|
| `loupe serve` | Loads the configuration, opens the database, applies the migrations, then listens on `LOUPE_LISTEN`. After the listener is bound and the line `listening` is written, it decides the telemetry state, writes the line `telemetry`, and in the state `sent` starts the one start request in its own task. See "Start request". |
| `loupe gen-key` | Prints one new random 32-byte key in standard base64 with padding, then a newline. The key comes from the random source of the operating system. The subcommand stores nothing. |
| `loupe config show` | Prints the effective configuration. See `configuration.md`. |
| `loupe backfill` | Reads the session logs of Claude Code or Pi into the calls and the sessions. See `backfill.md`. |

Each subcommand has a complete `--help`.

`loupe --version` prints `loupe <version>`, then a newline, on stdout, and exits with code 0. The version comes from `Cargo.toml` at build time. `release.md` states the version rules.

## Logging

The binary writes each log event as one line to stderr and to `<LOUPE_DATA_DIR>/logs/loupe.log`. Each line in the file is one JSON object. Each line has the time in UTC, the level, the source location as `path/file.rs:LINE`, and the message with its key fields. No log line contains a secret value. A normal start with valid configuration logs no warning and no error.

The first log line of `loupe serve` holds the key field `version` with the version from `Cargo.toml`. Like each line, it goes to stderr and to the log file.

After the line `listening`, `loupe serve` writes one info line `telemetry` at each start. Its key field `state` is `sent`, `disabled`, or `built-out`. In the states `sent` and `disabled`, the key field `host` holds the host of the start request, and nothing else of the URL. The line never holds the scheme, the port, the path, the commit, the query, or the `user-agent` value. The line is at info level, so a normal start still logs no warning and no error. The result of the start request goes to one line at debug level only. `telemetry.md` gives the rules.

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

## Start request

A binary that holds a telemetry host sends one start request at each start of `loupe serve`. A release build holds the host. A build from source and a local image build do not. `telemetry.md` gives the full rules of the request and of its receiver.

1. The request is one `GET` of `https://<host>/<commit>`, with the full 40-character lowercase hex commit of the build as the path. The scheme is `http` only for the host `127.0.0.1` or `localhost`. The URL has no query. The URL is the schema.
2. The request has no body. The header `user-agent` has the form `agentloupe/<version> (<target triple>)`, with the version from `Cargo.toml` and the target triple of the build.
3. The request starts after the listener is bound and after the line `listening`. It runs in its own task. The bind, the line `listening`, and the first answer of `GET /healthz` do not wait for it.
4. The request has one attempt, a timeout of 5 seconds, and no retry. It follows no redirect. A status code, a network error, or a timeout gives one log line at debug level only, and does not change the start or the run.
5. The request uses its own HTTP client. It never goes through the proxy routes, and the capture, the database, the live feed, the dashboard, and the public API never see it.
6. A shutdown does not wait for an open start request.
7. The line `telemetry` names the state:

| State | Condition |
|---|---|
| `built-out` | The binary holds no host. The start path sends nothing. |
| `disabled` | The binary holds a host and a run-time opt-out is set. The start path sends nothing. |
| `sent` | The binary holds a host and the request started. |

The run-time environment cannot give a host to a `built-out` binary, and cannot change the host of a `built-in` binary.

## Shutdown

On SIGTERM or SIGINT, `loupe serve` stops new connections. It lets open requests finish within 10 seconds. Then it closes the database and exits with code 0.

## Container

The `Dockerfile` has two stages. The build stage compiles a release binary with the toolchain that `rust-toolchain.toml` pins. The runtime stage holds only the binary and its runtime files. It runs as a user that is not root. Its `HEALTHCHECK` calls `GET /healthz`.

The runtime image is `debian:bookworm-slim`. It has the same C library as the build stage, and it has `bash`. The `HEALTHCHECK` uses the `/dev/tcp` feature of `bash`, so the image needs no `curl`. A smaller distroless image has no shell, so it cannot run a `HEALTHCHECK` without an extra program. The runtime user is `loupe` with UID and GID `10001`.

Each `FROM` line of the `Dockerfile` names its base image by the SHA-256 digest of the multi-platform image index, not by a tag alone. A comment next to the line keeps the tag. The index digest resolves to the correct platform on each runner, so one `Dockerfile` builds `linux/amd64` and `linux/arm64`. Thus two builds of one commit get the same operating system layers. No `FROM` line is without a digest, and a test asserts this. At each minor release, a human refreshes the digests. `docker buildx imagetools inspect <image>:<tag>` prints the current digest of the index of a tag on its `Digest:` line. No tool opens update pull requests for the digests.

The runtime stage puts `LICENSE` into `/app` in each image. The release image also holds `/app/THIRD-PARTY-NOTICES`. A local build passes when no `THIRD-PARTY-NOTICES` file exists, and the image then holds `/app/LICENSE` only. `release.md` (Third-party notices) states the file.

`compose.yaml` has one service. It binds the port as `127.0.0.1:8787:8787`. It mounts a named volume at `/data`, reads the secrets from `.env`, and sets `LOUPE_ENV`. `.env.example` lists each variable name that `compose.yaml` passes, with no value.

### Registry image

A release publishes the image `ghcr.io/desek/agentloupe`. The release workflow builds it from the same `Dockerfile` with no change. The image is one multi-platform index for `linux/amd64` and `linux/arm64`, with the tags `<version>` (no `v`) and `latest`. The tag `latest` moves only when both platform builds pass, and only forward, through the promote step of the release image script. The image carries the OCI labels source, version, revision, licenses, title, description, and created. The label created is the commit time of the tag. `release.md` states the values. The image holds `/app/LICENSE` and `/app/THIRD-PARTY-NOTICES`. The release image script checks both files after the build and pushes nothing when one is absent. A pull of the image needs no login. `compose.yaml` stays a local build of the image.

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
| `pipeline.lint.sh` | `cargo clippy` with warnings as errors, then the dependency check `deps.deny.sh` |
| `pipeline.test.sh` | `cargo nextest run`, and `cargo test --doc` if nextest does not run the doc tests |
| `pipeline.web.sh` | Checks that `node --version` equals the pin in `.tool-versions`, runs `node --test` on the calculation modules of the page, and checks the size of `web/` with `web.size.sh`. `--check-toolchain` does only the check of the pin. |
| `pipeline.image.sh` | Builds the container image. `--dry-run` prints the build command and does not run it. |

Each entry point has `--help`, a machine output by default, and `--human` for the friendly view. It writes actionable errors to stderr and returns documented exit codes. The entry points read the toolchain version from `rust-toolchain.toml` and state no version of their own.

`pipeline.verdict.sh` runs the stages in the order format, lint, test, web, image. It only calls the stage entry points. It prints one line for each stage with `status: passed` or `status: failed`, and the time of the stage. It also prints the total time. It returns a non-zero exit code if a stage fails. The entry point `pipeline.web.sh` reads the Node version from `.tool-versions`. If `node` is missing or its version differs from the pin, it stops with a non-zero exit code and an actionable error. The thin caller installs Node from `.tool-versions` and states no Node version. The stages format, test, and web are hermetic: they use no network after the dependencies are fetched, and no assertion reads the wall clock. The lint stage is hermetic with one exception: the dependency check fetches the advisory database over the network. This fetch is the one network use of the lint stage.

### Dependency check

The lint stage calls `.agents/scripts/deps.deny.sh` after clippy. The script runs `cargo deny` with the four checks advisories, licenses, bans, and sources against `deny.toml` at the root, with the lock file locked. The dependency check is not a stage of its own, so the verdict keeps its five stages.

1. `pipeline.lint.sh` prints one stage line. The stage status is `failed` when clippy or the dependency check fails. The stage log holds the output of both.
2. When the advisory database cannot be fetched, the script stops with an actionable error that names the fetch, and the lint stage fails. The check fails closed.
3. The script fails when the accepted licenses of `about.toml` differ from the allowed licenses of `deny.toml`. The error names each license that is in one list only.
4. `.tool-versions` pins cargo-deny and cargo-about under their names in the registry of mise. The script reads no version of its own. When cargo-deny is missing, the script stops with exit code 3 and an actionable error that names the install from `.tool-versions`.
5. The script follows the same rules as the stage entry points: `--help`, a machine output by default, `--human`, actionable errors on stderr, and documented exit codes.

`deny.toml` states this policy:

| Check | Policy |
|---|---|
| Licenses | An allow list of permissive SPDX identifiers. No copyleft license is on the list. An exception for one crate names the crate and the reason in a comment. |
| Advisories | A vulnerability in any crate fails. An unmaintained or unsound advisory fails for a direct dependency. A yanked crate fails. An ignore entry names the advisory identifier, the reason, and a review date. |
| Bans | A wildcard version requirement fails. More than one version of a crate gives a warning, not a failure. |
| Sources | Only the crates.io registry. A git source or another registry fails. |

When a new advisory has no patched version, the owner adds an ignore entry with the identifier, the reason, and a review date.

### Test runner

The test stage runs the tests with the parallel runner `cargo nextest`. The minimum version of the runner is pinned in `.tool-versions`, on the line `aqua:nextest-rs/nextest/cargo-nextest`. That is the name of the tool in the registry of mise, so `mise install` can install it, and a bare `cargo-nextest` line is not used, because mise fails at the start of a shell for a tool name that its registry does not know (observed on 2026-10-22). The test stage matches the tool name `cargo-nextest` at the end of the first column, reads the pin from that line, and states no version of its own. If the runner is missing or its version is below the pin, the test stage stops with exit code 3 and an actionable error. The error names the install command `cargo install cargo-nextest --locked --version <pin>`, the pin file, and the verify command. The test stage does not fall back to `cargo test`, so the measured loop stays one runner. The image stage does not change, because the Dockerfile runs no tests.

### Timers in tests

A test that asserts a timer drives the timer through an injected clock, or sets a documented configuration variable to a short value. Such a test advances the clock, or polls for the end state with a cap, and then asserts. A test that drives a real socket or a real process keeps real time. Each wait in such a test is a poll for an end state, with a cap that is not longer than before and a poll interval of at most 100 milliseconds. No test sleeps for a fixed time before an assertion. No assertion reads the wall clock to decide pass or fail; a cap only ends a poll as a failure, and the failure prints the expected end state and the last observed state. An injected clock or a test variable does not change a default: with no variable set, each timer keeps its value.

### Tests of the dependency autofix

The test file `tests/deps.autofix.rs` tests `deps.autofix.sh`. It has its own `[[test]]` entry in `Cargo.toml` and runs in the test stage.

1. Each test runs the script in its fixture mode. A documented flag of the script, for tests only, names a fixture directory under `tests/fixtures/deps/`. The fixtures take the place of the GitHub API and of `claude`.
2. The fixtures are recorded responses: an alert list, an empty list, a list of Dependabot pull requests, a release pull request, a `claude -p` result, a diff, and a log. They hold no credential. A test that needs a token value uses a test value.
3. The test stage stays hermetic. A test of the script uses no network, starts no `claude` process, and writes nothing to GitHub.
4. The tests cover: nothing open, the dry run of an alert, an alert with a matching Dependabot pull request, an alert with no patched version, an absent verdict workflow, a missing credential that is named and not printed, an invalid result, a path outside the allowed paths, the form of the commit summary, the redaction of the log extract, a release pull request with a major version, and a pin update that replaces a SHA with a tag.

### Stage inputs and toolchain

Each stage entry point has the flag `--inputs`. It prints the paths that the stage reads, one path per line, and exits 0 with no run. The declared inputs are:

| Stage | Declared inputs |
|---|---|
| format, lint | `src`, `tests`, `Cargo.toml`, `Cargo.lock`, `build.rs`, `migrations`, `vendor`, `rust-toolchain.toml`, `examples`, `.cargo`, `rustfmt.toml`, `.rustfmt.toml`, `clippy.toml`, `.clippy.toml`, and the script of the stage |
| lint, in addition | `web`, because `build.rs` reads `web/` when clippy builds the crate, and `deny.toml`, `about.toml`, and `.agents/scripts/deps.deny.sh` of the dependency check |
| test | the inputs of format and lint with the script of the test stage, and also `.tool-versions` (the pin of the runner), any configuration file of the runner, `upstreams.toml`, `config`, `.agents/scripts`, each file under `docs/` that a test reads, today `docs/specification/accounts.md`, `.github/workflows` (a test reads the verdict and release workflow files), and also `web` (`build.rs` embeds `web/` into the crate, and tests read `web/`) and `Dockerfile` (a test reads its `COPY` lines) |
| web | `web`, `.tool-versions`, `.agents/scripts/pipeline.web.sh`, `.agents/scripts/web.size.sh` |
| image | `Dockerfile`, `.dockerignore`, each source path of a `COPY` instruction in the Dockerfile (`rust-toolchain.toml`, `Cargo.toml`, `Cargo.lock`, `build.rs`, `web`, `vendor/litellm`, `src`, `migrations`, `tests`, `LICENSE`, `THIRD-PARTY-NOTICES`), `.agents/scripts/pipeline.image.sh` |

Git ignores the generated `THIRD-PARTY-NOTICES`, so the temporary tree of the fingerprint never holds it, and the image stage fingerprint always sees it as absent.

Some declared paths do not exist today, for example the configuration files of the format and lint tools. A declared path that does not exist is part of the fingerprint as absent, so its later creation changes the fingerprint. A test finds each repository path that a test file reads, and fails if that path is not a declared input of the test stage.

Each stage entry point also has the flag `--toolchain`. It prints the versions of the tools outside the tree that the stage uses: the Rust compiler and cargo for format and lint, also cargo-deny for lint, also the runner for test, Node and the package manager for web, and the Docker server version and the local digest of each base image for image. The image stage reads the digests from the local image store with no network call.

### Fingerprints and the verdict cache

The tree fingerprint is the git tree hash of the working tree. The verdict makes it in a temporary index: it reads `HEAD`, adds every tracked change and every untracked file that `.gitignore` does not exclude, and writes the tree. Thus a dirty tree gets a fingerprint by its content, and the ignored files stay out. The verdict does not change the real index, the working tree, or the stash. Two trees with equal content give the same fingerprint, whatever their commits.

The stage fingerprint is the hash of the sorted path and object entries of the declared inputs of the stage in that temporary tree, plus the name of the stage, plus the `--toolchain` output of the stage. A change outside the inputs of a stage does not change its fingerprint.

The verdict cache is stored under `.agents/logs/`, which git does not track. The verdict writes the fingerprint of a stage only after the stage passed in a run of the verdict. A failed stage writes nothing. A write is atomic: the verdict writes a temporary file and then renames it. The cache keeps at most 200 tree entries and drops the oldest.

The verdict first compares the tree fingerprint plus the `--toolchain` output of every stage with the cache. If that key equals a key for which every stage passed, the verdict prints each stage with `status: cached` and exits 0. A match of the tree fingerprint alone does not skip a stage. Else the verdict compares each stage fingerprint with the last passed fingerprint of that stage. A stage with an equal fingerprint prints `status: cached` and does not run. Any other stage runs. A cached stage writes its stage fingerprint again, also on a match of the whole key, so the last passed fingerprint of each stage is always that of the last passed tree. A stage line has the form `stage: <name> status: passed|failed|cached seconds: <n>`, and the last line has the form `verdict: passed|failed seconds: <total>`. A cached stage counts as passed for the exit code.

The flag `--no-cache` runs every stage, and still writes the fingerprints of the stages that passed.

The verdict fails closed. If git is missing, the temporary index fails, a hash fails, the cache is absent or unreadable, a `--inputs` or a `--toolchain` fails, or a base image is absent from the local image store, the verdict runs the affected stages in full. It prints one diagnostic line on stderr that names the cause. Only a positive match skips a stage. The fingerprint and cache logic is in a separate script under `.agents/scripts/` that the verdict calls, and the verdict stays a caller of the stage entry points.

Only the stages format, lint, test, web, and image enter the verdict. The image stage is in the default verdict. The lint stage reads the advisory database over the network. The stage fingerprint does not cover the advisory database, so a cached lint pass can stay when a new advisory appears. The hosted verdict workflow has no verdict cache, so it checks the advisories again on each pull request and on each release tag. The live scenarios and the benches stay outside the verdict, because they use the network or the wall clock.

#### Hosted verdict reuse

The hosted verdict workflow reuses an earlier full green verdict of the identical tree. A squash merge makes a new commit over the same tree as the head of its pull request. Thus the push run on `main` reuses the proof of the pull request run and does not run the slow stages again. The key is the git tree hash of the checked-out commit. Only exact equality of that hash counts. The local verdict cache, `pipeline.verdict.sh`, and its fingerprint script do not change.

The entry point `.agents/scripts/verdict.reuse.sh` decides the reuse. It has two modes, `lookup` and `judge`. It follows the same rules as the stage entry points: `--help`, a machine output by default, `--human`, actionable errors on stderr, and documented exit codes.

The mode `lookup` takes the platform values as explicit inputs: the repository, the commit of the checkout, the event (`push` or `pull_request`), the base commit, the id of the current run, and a file for the decision. The base commit is the first parent of the pushed commit for a push, and the base commit of the pull request for a pull request. The token comes from the environment variable `GH_TOKEN`. Where git can give a value, the script uses git as the local default.

1. The lookup first checks that `HEAD` equals the commit input. Then it computes the tree hash of `HEAD` with git. Thus the tree that the decision names is the tree that the jobs check out.
2. The lookup reads the run records of the repository. Each run record holds the head commit, the event, the conclusion, the head repository, and the tree hash of the head commit.
3. The lookup accepts a source run only when each condition below is true:
   1. The run is a run of the verdict workflow of this repository, with the conclusion `success`.
   2. The event of the run is `push` or `pull_request`. A run of another event does not count.
   3. The head repository of the run is this repository. A run from a fork does not count.
   4. The run started within the lookback window of 72 hours before the current run.
   5. The full 40-character tree hash of the head commit of the run equals the tree hash of item 1, by string equality. No other match counts: not a commit SHA, a branch, an ancestor, a path filter, or a green check of a pull request.
   6. Each of the five stage jobs format, lint, test, web, and image of the run has the conclusion `success`. A run in which a reuse skipped a stage job does not count.
   7. For a run of the event `pull_request`, the head commit of that run contains the base commit: the compare of the base with the head gives `ahead` or `identical`.
   8. The run is not the current run.
4. The lookup is read-only. It makes only read calls to the GitHub API. It writes no cache, no artifact, no status, and no verdict store. The run records already hold the tree and the job results.
5. The lookup fails closed. A missing tool, a missing token, a failed or partial API call, an unknown field, a bad commit input, a `HEAD` that differs from the commit input, or a page limit that the lookup reaches gives the decision `miss` with the reason `lookup_failed` and the cause. Only a positive match gives `hit`.
6. The lookup prints one machine line with the decision (`hit` or `miss`), the reason (`match`, `novel`, or `lookup_failed`), the tree, and, for a hit, the id of the source run and the source commit.

| Exit code of `lookup` | Meaning |
|---|---|
| 0 | The decision is `hit`. With a decision file, also each `miss`: the script writes the decision and the reason to the file, so the platform step does not fail on a miss. |
| 1 | With no decision file: the decision is `miss` with the reason `novel`. |
| 2 | A usage error. |
| 3 | A tool is missing. |
| 4 | With no decision file: the decision is `miss` with the reason `lookup_failed`. |

The mode `judge` takes the results of the five stage jobs and the decision as explicit inputs. It passes only in one of two cases:

1. Each of the five stage jobs has the result `success`.
2. The decision is `hit`, the lint job has the result `success`, and the four jobs format, test, web, and image have the result `skipped`.

Each other combination fails. An absent decision counts as `miss`.

The soundness of the reuse rests on two rules:

1. A source run is a full run. Each of its five stage jobs ran and passed, and no reuse skipped one. Thus a reused run never becomes the source of a later reuse, and the age of a proof never passes the lookback window.
2. A pull request run is a source only when its head contains the base commit. GitHub runs the workflow of a pull request from the merge ref. When the head contains the base, the merge ref has the tree of the head, so the workflow file of that run equals the workflow file of the tree.

With these rules, the set of trees that can reach `main` with a green check `verdict`, and the set of content that a release can publish, are equal to or smaller than with a verdict that always runs in full.

The verdict workflow uses the reuse in this way:

1. The first job `reuse` runs on a Linux x86_64 runner. It checks out the head commit of the event, never the merge ref, with the history that the base commit needs. It makes exactly one call of `verdict.reuse.sh lookup`. Its decision goes to the other jobs as the output of the job.
2. The job `reuse` does not run on a call from another workflow. A call from the release workflow runs the five stage jobs in full, so the release keeps its full verdict on the tag commit.
3. The stage jobs format, test, web, and image run unless the decision is `hit`. When the job `reuse` fails, is skipped, or gives no decision, these jobs run in full.
4. The lint job runs in full on each event. The dependency check reads the advisory database over the network, so the lint stage is not deterministic and is never reused.
5. The job `verdict` depends on the job `reuse` and the five stage jobs. It makes exactly one call of `verdict.reuse.sh judge`. The check name `verdict` does not change, so the main ruleset still requires it.

The lookback window is 72 hours. The tree does not capture the runner image, which GitHub updates about each week. A window shorter than that cadence limits the drift. A squash merge usually follows the green pull request run within this window.

A test reads the verdict and release workflow files. It fails when a checkout of the verdict workflow does not use the head commit or uses the merge ref, when the job `reuse` can run on a call from another workflow, when the lint job has a condition on the decision, when the job `verdict` does not call `verdict.reuse.sh judge`, when a job of the verdict workflow has a write permission, or when the release workflow does not call the verdict workflow before its builds.

### Hosted workflows

Five GitHub Actions workflows are thin callers of the pipeline. A workflow does only platform work: the trigger, the runner, the checkout, the toolchain install from the pin files, the cache, one call for each stage or release step, the upload of artifacts and release assets, the login to the registry, and the gate on the visibility of the repository. A workflow file holds no project logic, no tool version, no target triple, no image name, no version of the project, no configuration value, and no secret value. The pin files, the release-please configuration files, and the scripts hold these values. The one limit that a workflow file holds is the timeout of each job. Each action from outside the repository is pinned by its full 40-character commit SHA, with the release tag in a comment.

| Workflow | Starts on | Does |
|---|---|---|
| verdict | each push to `main`, each pull request that targets `main`, and a call from another workflow of the repository with the ref to check out as an input | Runs the job `reuse` first, except on a call from another workflow (see "Hosted verdict reuse"). Then it runs the stages format, lint, test, web, and image as five parallel jobs. On a reuse hit, only the lint job runs. Then it runs one final job `verdict`. |
| release-please | each push to `main` | Keeps one release pull request open, and makes the tag and the GitHub release when the release pull request merges. `release.md` states the flow. |
| release | the push of a tag `v<major>.<minor>.<patch>`, and a manual start with a tag input | Runs the verdict workflow on the tag commit, then builds and publishes the release assets. `release.md` states the assets. |
| advisory-response | each day at 05:23 UTC, and a manual start | Makes one call of `advisory.respond.sh` on a checkout of the default branch with all tags. `release.md` (Advisory response) states the rules. It has the permissions `contents: write`, `issues: write`, and `packages: write`, and one concurrency group that never cancels a run. |
| deps-autofix | each day at 04:17 UTC, and a manual start | Makes one call of `deps.autofix.sh`. `dependencies.md` states the rules. It has the permissions `contents: write`, `pull-requests: write`, `issues: write`, `security-events: read`, `actions: read`, and `checks: read`, and one concurrency group that never cancels a run. Only the step of the script gets the secret `CLAUDE_CODE_OAUTH_TOKEN`. The checkout keeps no credential in the git configuration. |

The verdict workflow:

1. Each stage job makes exactly one call to its stage entry point, with no argument that changes the stage.
2. Each job checks out the head commit of the event: the head commit of the pull request, the pushed commit, or the commit of the ref input. A job never checks out the merge ref of a pull request.
3. Each job installs the Rust toolchain with rustup from `rust-toolchain.toml`, and Node, `cargo-nextest`, and cargo-deny with mise from `.tool-versions`. A job installs only the tools that its stage needs. The lint job installs cargo-deny.
4. The workflow fails closed. The job `verdict` depends on the job `reuse` and the five stage jobs. It passes only in the two cases of the mode `judge` (see "Hosted verdict reuse"): each of the five jobs has the result `success`, or the decision is `hit`, lint has the result `success`, and the four other stage jobs have the result `skipped`. Each other result makes it fail. No job continues on an error or retries automatically.
5. Each job of the workflow has the permission `contents: read`. The job `reuse` also has `actions: read` when the token needs it to read the run records. No job of the verdict workflow has a write permission.
6. The test stage runs on Linux in the verdict workflow. The local verdict of `pipeline.verdict.sh` runs on macOS.
7. A new push to a pull request cancels the older verdict run of that pull request. A run for a push to `main` and a release run are never cancelled.
8. The hosted runners do not use the verdict cache of `pipeline.verdict.sh`. The job `reuse` reads the run records of the repository instead (see "Hosted verdict reuse"). The jobs cache the cargo registry and the target directory with a key from `Cargo.lock` and `rust-toolchain.toml`.

The release-please workflow gives its `GITHUB_TOKEN` no permission. release-please itself uses the release token of `release.md`. The release workflow logs in to the registry with its `GITHUB_TOKEN`, with the permissions `contents: write` and `packages: write`. The build jobs and the image jobs of the release workflow install cargo-about with mise from `.tool-versions`, for the third-party notices.

Each job that runs on a runner has a timeout. A job with no timeout can run for 360 minutes.

| Workflow | Job | Timeout |
|---|---|---|
| verdict | the job `reuse` | 5 minutes |
| verdict | each of the five stage jobs | 30 minutes |
| verdict | the final job `verdict` | 5 minutes |
| release | `visibility` | 5 minutes |
| release | each build job and each image job | 60 minutes |
| release | `publish` | 15 minutes |
| release-please | the release-please job | 10 minutes |
| advisory-response | the job `respond` | 30 minutes |
| deps-autofix | the one job | 180 minutes |

The job `verdict` of the release workflow calls the verdict workflow, which is a reusable workflow. It has no timeout of its own. The jobs of the called workflow carry their own timeouts.

The release scripts `release.build.sh`, `release.image.sh`, `release.sbom.sh`, `release.notices.sh`, and `release.check.sh`, the advisory script `advisory.respond.sh`, and the dependency autofix script `deps.autofix.sh`, are in `.agents/scripts/`. They follow the same rules as the stage entry points: `--help`, a machine output by default, `--human`, actionable errors on stderr, and documented exit codes. They use the network, so they stay outside the verdict. `pipeline.verdict.sh` calls no release script and does not call `verdict.reuse.sh`. The hosted workflows do not change `pipeline.verdict.sh`, its cache, or its fingerprints. The repository settings script `repo.settings.sh` follows the same rules. Only a person runs it, and no workflow calls it. `release.md` (Repository settings) states it.

The project uses no paid service. The workflows use the GitHub-hosted runners and the GitHub Container Registry, which are free for a public repository. While the repository is private, only the verdict and release-please workflows run, on Linux x86_64 runners, and the release workflow stops in its first job. The account keeps a budget of 0 USD for GitHub Actions and for GitHub Packages, with usage stopped at the limit.

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
| Size of the page files | `.agents/scripts/web.size.sh` prints `code_kib` and `total_kib` | `code_kib` 41.6 and `total_kib` 184.7 on 2026-10-20; re-measured `code_kib` 41.6 and `total_kib` 184.7 on 2026-10-20 at verification; `code_kib` 62.3 and `total_kib` 218.2 on 2026-10-21 after the card gap, the token shares, and the cost footnote; re-measured `code_kib` 62.3 and `total_kib` 218.2 on 2026-10-21 at verification, passed; `code_kib` 64.8 and `total_kib` 220.8 on 2026-10-21 after the lines footer line in the pull request card and the asterisk sentence in the footer, passed; re-measured `code_kib` 64.8 and `total_kib` 220.8 on 2026-10-21 at verification, passed | `code_kib` at most 100 (without the fonts), `total_kib` at most 400 (with the fonts) |
| First full draw, 400 days | `.agents/scripts/web.render.sh --days 400` prints `draw_ms` | 81.7 on 2026-10-20; re-measured 53 on 2026-10-20 at verification; 93.4 on 2026-10-21 after the token shares and the cost footnote; re-measured 111 on 2026-10-21 at verification, passed; 99.3 on 2026-10-21 after the lines footer line in the pull request card and the asterisk sentence in the footer, passed; re-measured 130.6 on 2026-10-21 at verification, with `headline_ratio` 2.12 (threshold at least 2) and `no_hscroll_360` true, passed | `draw_ms` at most 500 |
| Headline size | `.agents/scripts/web.render.sh --days 400` prints `headline_ratio`, and `headline_above_fold` at 360 px and at 1440 px | `headline_ratio` 2.12, `headline_above_fold: true` at 360 px and at 1440 px, on 2026-10-20; re-measured `headline_ratio` 2.12, `headline_above_fold: true` at 360 px and at 1440 px, on 2026-10-20 at verification; `headline_ratio` 2.12 on 2026-10-21; re-measured `headline_ratio` 2.12, `headline_above_fold: true` at both widths, on 2026-10-21 at verification, passed | `headline_ratio` at least 2.0, `headline_above_fold: true` at both widths |
| Remote requests of the page | `.agents/scripts/web.render.sh --days 400` prints `remote_requests` | `remote_requests: 0` on 2026-10-20; re-measured `remote_requests: 0` on 2026-10-20 at verification | `remote_requests: 0` |
| Card gap | the smallest gap between two cards and between two rows in the render at 360 px and at 1440 px, read from the bounding boxes with the browser driver | 24 px at both widths on 2026-10-21; re-measured 24 px at both widths on 2026-10-21 at verification, passed | at least 24 px, from `--space-card-gap` |
| Token share sum | `node --test web/test/stats.shares.test.js` checks that the counts sum to the total and that the printed percentages sum to a value in the range | no check before 2026-10-21; 4 of 4 tests passed on 2026-10-21; the render with 400 seeded days prints 4.8, 91.7, 0.1, and 3.4 percent, sum 100.0 | counts sum to the total; printed percentages from 99.8 to 100.2 |
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
| Warm full verdict | `.agents/scripts/pipeline.verdict.sh --no-cache`, second run, total seconds, with the image stage | about 104 seconds on 2026-10-21 (format 0.5, lint 0.6, test 99, web 0.5, image 1, up to 70 when the Docker layer cache misses); after: 31.3 seconds on 2026-10-21 with `--no-cache` (format 0.6, lint 0.8, test 27.4, web 0.6, image 0.7); the first run after the change took 112.7 seconds, with a Docker layer cache miss of 77 seconds; re-measured 30.4 seconds on 2026-10-21 (test 26.7), passed; re-measured 27.9 and 27.8 seconds on 2026-10-21 at the second verification (test 25.2), passed | at most 40 seconds, every stage `status: passed` |
| Test stage, first run after a Rust edit | the seconds of the `test` line of the first verdict after the edit, on this Mac, where macOS scans each freshly linked test binary on its first start | 99 seconds on 2026-10-21 with `cargo test`; 166.6, 181.3, and 201.5 seconds on 2026-10-21 with `cargo nextest` and the scan; re-measured 206.7 seconds on 2026-10-21, passed; at the second verification on 2026-10-21 the first test runs took 225.4 seconds (first verdict of the session), 234.8 seconds (after a web edit), and 255.1 seconds (with an unreadable cache), above the threshold, failed; 206.7, 225.4, 234.8, 255.1, and 266.9 seconds on 2026-10-21 after the cache refresh fix; 378.1 seconds in a verdict and 679.9 seconds for `pipeline.test.sh` after `touch src/lib.rs` on 2026-10-21 at 23:06, with iTerm added to Developer Tools but not restarted, under a load average of 25 to 49 from other processes, `syspolicyd` at 235 percent CPU, every test passed, failed; 566.9 seconds on 2026-10-22 at 06:42 after an iTerm restart with the Developer Tools grant in place, load average 7 at the start and 36 at the end, every test passed, failed: the grant does not remove the scan; 798.6 seconds on 2026-10-23 in the verdict of the light mode verification, every test passed, failed | at most 300 seconds (decided by the human on 2026-10-21, after the re-measurement, because the scan is a machine cost outside the repository) |
| Test stage with the timer rule | the seconds of the `test` line of a second `.agents/scripts/pipeline.verdict.sh --no-cache` run, warm tree | 26.7 seconds for 394 tests on 2026-10-21; after: 14.5 and 14.0 seconds for 417 tests on 2026-10-21, every stage `status: passed`, passed; re-measured at the verification on 2026-10-21 with a load average of 19 to 40 on 10 cores from other processes: 16.7 and 18.1 seconds in the verdict, and 14.5, 19.6, 16.8, 16.4, and 17.4 seconds for `pipeline.test.sh`, all tests passed, failed; re-measured at the second verification on 2026-10-21 with a load average of 14 to 23 on 10 cores from other processes: 21.7 and 21.5 seconds for `pipeline.test.sh`, `status: passed`, failed, because the load from other processes blocks the measurement | under 15 seconds, all tests pass |
| Slowest test | the slowest test in the summary of `cargo nextest run` | about 10 seconds or more on 2026-10-21 (15.0 seconds for the heartbeat test); after: 3.8 seconds on 2026-10-21, the slowest of 20 runs, passed; re-measured 5.4 seconds on 2026-10-21 at the verification under the same load (`whole_tree_hit_refreshes_stage_fingerprints`, not a timer test), the slowest of 20 runs, failed; 15 of 20 runs passed, the 5 failures are the wall-clock p95 bound of `private_config_read_is_cached` (8.4 ms against 5 ms) | at most 5 seconds |
| Flake of the changed timer tests | each changed test, 20 runs in a row under `cargo nextest run` | not measured; after: 20 of 20 runs of the full suite with `--retries 0` on 2026-10-21 (12.9 to 13.7 seconds each), and 20 of 20 runs of the changed tests, passed | 20 of 20 pass |
| Test stage, warm run | the seconds of the `test` line of a second `--no-cache` run after the same edit | 24 seconds with `cargo nextest run --no-fail-fast` (394 tests) on 2026-10-21; 27.7 to 29.5 seconds in the verdict on 2026-10-21; re-measured 26.7 seconds on 2026-10-21, passed; re-measured 25.2 seconds twice on 2026-10-21 at the second verification, passed; supervisor measurement on 2026-10-21 at 19:35 under a load average of 40 to 58 on 10 cores: 15.2 and 14.4 seconds, and 13.4 seconds for the runner alone with 417 tests | at most 30 seconds |
| Docs-only change | change `docs/specification/operations.md`, then `.agents/scripts/pipeline.verdict.sh`, total seconds | about 104 seconds on 2026-10-21; after: 0.9 seconds on 2026-10-21, every stage `status: cached`, exit code 0; re-measured 0.8 seconds on 2026-10-21 (a line changed in `docs/specification/index.md`), every stage `status: cached`, passed; an unchanged tree, second run: 0.8 seconds, every stage `status: cached`, passed; re-measured 0.6 seconds (index.md edit) and 0.7 seconds (unchanged tree) on 2026-10-21 at the second verification, passed | at most 3 seconds, every stage `status: cached`, exit code 0 |
| Web-only change | change a file under `web/`, then `.agents/scripts/pipeline.verdict.sh` | about 104 seconds on 2026-10-21; after: 282.9 seconds on 2026-10-21; every stage ran: lint and test because `build.rs` embeds `web/`, format because its last passed fingerprint was of a Rust edit that was reverted before; re-measured 325.6 seconds on 2026-10-21 after a Rust edit, its revert, and one cached run: format `status: passed` (not cached), lint 45.5, test 225.7, web, and image `status: passed`, failed, because the cached run of the reverted tree matched the key of the whole tree and did not write the format fingerprint again; re-measured 332.4 seconds on 2026-10-21 at the second verification: format `status: cached`, lint 44.0, test 234.8, web 0.4, image 52.4, all `status: passed`, passed | format `status: cached`; lint, test, web, and image `status: passed`, because `build.rs` embeds `web/` and the Dockerfile copies `web`. Changed on 2026-10-21 from "format, lint, and test `status: cached`", because a test reads `web/` and the crate embeds it, so a cached test stage could pass content that a full run fails |
| Fingerprint cost | the timing line on stderr of the fingerprint script | none, a new measurement; 0.56 to 0.74 seconds on 2026-10-21 | at most 1 second |
| Size of the private page files | the size of the files under `web/private/`, without the shared fonts | no private page exists; 12.9 KiB on 2026-10-21 | at most 200 KiB |
| Code size with the color schemes | `.agents/scripts/web.size.sh` prints `code_kib` | 65.6 KiB at `446f065`, after the product header (64.8 KiB at `ac55e2a`); 75.3 KiB at `e88f5bd`, with the three scheme buttons, passed; 76.1 KiB on 2026-10-23 at verification, with the scheme switch, passed | at most 100 KiB |
| Added scheme code | the increase of `page_kib` of `.agents/scripts/web.size.sh`, the bytes of `web/` minus `web/assets/fonts/`, `web/private/`, and `web/test/`, from 47.7 KiB at `446f065` | 0; +3.5 KiB (51.2 KiB) at `e88f5bd`, with the three scheme buttons, passed; +4.3 KiB (52.0 KiB) on 2026-10-23 at verification, with the scheme switch, passed. Changed on 2026-10-23 from the increase of `code_kib`, which counts the node tests (+9.7 KiB, 65.6 to 75.3 KiB at `e88f5bd`, of which the node tests 6.2 KiB, 17.9 to 24.1 KiB), because the target is a budget of the page code | at most 6.0 KiB |
| Draw time, each scheme | `.agents/scripts/web.render.sh --days 400 --scheme light` and `--scheme dark` print `draw_ms` | not recorded at the start of implementation; 58.2 ms, dark only, at `446f065`, measured on 2026-10-23 in a worktree; 28.9 ms light and 27.2 ms dark on 2026-10-23 at verification, with the scheme switch, passed; 22.0 to 30.1 ms in the 8 first-frame runs, passed | at most 500 ms in each scheme |
| Remote requests, each scheme | `.agents/scripts/web.render.sh --days 400 --scheme light` and `--scheme dark` print `remote_requests` | 0, dark only; 0 light and 0 dark on 2026-10-23 at verification, with the scheme switch, passed | 0 in each scheme |
| Headline above the fold with the scheme control | `.agents/scripts/web.render.sh --days 400 --scheme light` prints `headline_above_fold_360` and `headline_above_fold_1440` | true at both widths, with no control; true at both widths in light and in dark on 2026-10-23 at verification, with the scheme switch, passed | true at both widths, with the control |
| Headline ratio with the scheme control | `.agents/scripts/web.render.sh` prints `headline_ratio` | 2.12; 2.12 in light and in dark on 2026-10-23 at verification, with the scheme switch, passed | at least 2.0 |
| Horizontal scroll, each scheme | `.agents/scripts/web.render.sh --scheme light` and `--scheme dark` print `no_hscroll_360` | true, dark only; true in light and in dark on 2026-10-23 at verification, with the scheme switch, and no horizontal scroll at 360 px in 2 of 2 scenario runs (`.agents/scenarios/dashboard.light-mode.md`), passed | true in each scheme |
| Token contrast, light | `node --test web/test/tokens.contrast.test.js` | no light scheme, no test; 4 of 4 tests passed on 2026-10-23 at verification, with the scheme switch, passed | every pair of the contrast rules 2 to 6 of `dashboard.md` (Design tokens) passes |
| Rendered text contrast | `.agents/scripts/web.render.sh --days 400 --scheme light` and `--scheme dark` print `min_text_contrast` | not measured; 5.38 light and 4.98 dark on 2026-10-23 at verification, with the scheme switch, over the page and the states empty, error, GitHub not connected, partial, and the paused message; `min_paused_figure_contrast` 5.72 light and 4.98 dark, `min_focus_contrast` 6.11 light and 8.92 dark (threshold 3.0), passed | at least 4.5 in each scheme |
| Rendered graphic contrast | `.agents/scripts/web.render.sh --days 400 --scheme light` prints `min_graphic_contrast` over the series colors and heatmap levels 1 to 4 | not measured; 3.25 light on 2026-10-23 at verification, with the scheme switch, passed (1.78 dark, the dark heatmap levels keep their values and have no threshold) | at least 3.0 |
| First frame scheme | `.agents/scripts/web.render.sh --scheme light` and `--scheme dark`, each with the stored values none, `light`, `dark`, and `invalid`, prints `first_frame_scheme_ok` | not measured; true in 8 of 8 runs on 2026-10-23 at verification, with the scheme switch, passed; false with the scheme script removed, the negative control | true in all 8 runs |
| Scheme resolve rule | `node --test web/test/scheme.resolve.test.js` | no test; 2 of 2 tests passed on 2026-10-23 at verification, with the scheme switch, the rule module unchanged, passed | every row of the rule table of `dashboard.md` (Color scheme) passes |
| Policy header with the color scheme | `cargo test --test web_serve` | the policy of `dashboard.md` (Headers); 15 of 15 tests passed with `cargo nextest run --test web_serve` on 2026-10-23 at verification, with the scheme switch, the asserted policy unchanged, passed | unchanged |
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

### Release targets

The targets of the release and the hosted workflows. `release.md` states the release. The column Measured is empty until the first release `0.1.0` is measured. "None" means that the metric does not exist before the release work. The rows from "Community profile" on are measured after the switch to public and at the first release; their baseline is the value on 2026-10-23. The rows of the SBOM files are measured at the first release with SBOM files. The rows of the verdict reuse are measured at the implementation and in the first 14 days after it. The rows of the advisory response are measured with the dry run of the script. The rows of the attestations, of the time from an advisory to the response, and of the token of the draft advisory follow the public repository.

| Metric | Method | Baseline | Threshold | Measured |
|---|---|---|---|---|
| Verdict workflow time on a pull request | the duration of the workflow run, from `createdAt` to `updatedAt` of `gh run view <run id> --json createdAt,updatedAt`, on a pull request that changes one Rust file | none, no workflow | at most 20 minutes with a cold cache, at most 12 minutes with a warm cache | |
| Time from the tag to the published assets | the time from `createdAt` of `gh release view v<version> --json createdAt` to the end of the release workflow run on the same tag, when the asset files and the tag `latest` exist | none, no release | at most 45 minutes | |
| Size of each binary | the binary size that `release.build.sh` prints for each asset, read from the log of the release run | 14 402 064 bytes for a local release build on macOS arm64 on 2026-10-21; not a release asset | at most 25 MiB for each of the three binaries | |
| Size of the image | `docker image inspect --format '{{.Size}}' ghcr.io/desek/agentloupe:<version>` after the pull, for each platform | 158 MB for `agent-loupe:local` as `docker image ls` reports on 2026-10-23 | at most 200 MB for each platform | |
| Secrets in the workflow files | `gitleaks dir .github` finding count, plus `grep -rEc 'ghp_\|github_pat_\|gho_\|lpt_\|sk-ant-\|sk-' .github` | none, no workflow | 0 findings and 0 matches. A reference of the form `secrets.NAME` is not a secret value. | |
| Release check result | `.agents/scripts/release.check.sh --version <version>; echo $?` | none, no release | exit code 0, every check `status: passed` or a stated `skipped` | |
| New repository secrets | count of the secrets in the repository settings | 0 before 2026-10-23; 1 (`RELEASE_PLEASE_TOKEN`) since 2026-10-23 | exactly 2, `RELEASE_PLEASE_TOKEN` and `CLAUDE_CODE_OAUTH_TOKEN` | |
| Paid use | the billing usage of the account for GitHub Actions and GitHub Packages, and its budgets | none, no workflow | 0 USD charged; a budget of 0 USD with usage stopped at the limit for each product | |
| Community profile | `gh api repos/desek/agentloupe/community/profile --jq .health_percentage`, after the switch to public | 42 | 100 | |
| Workflow jobs with no timeout | in each of the three workflow files, count the jobs that have the key `runs-on` and no key `timeout-minutes` | 11 (6 in `verdict.yml`, 4 in `release.yml`, 1 in `release-please.yml`) | 0 | |
| Unpinned base images | count the `FROM` lines of `Dockerfile` whose image reference has no `@sha256:` with 64 hex digits | 2 | 0 | |
| Dependency check | `.agents/scripts/deps.deny.sh; echo $?` on `main`, and the lint job of the verdict workflow on the tag `v0.1.0` | none, no `deny.toml` | exit code 0 | |
| Notices in the archives | `tar -tzf <archive>` lists `THIRD-PARTY-NOTICES` in the top directory, for each of the three archives of the release | 0 of 3 | 3 of 3 | |
| Notices in the image | `docker run --rm --entrypoint cat ghcr.io/desek/agentloupe:<version> /app/THIRD-PARTY-NOTICES` exits 0 with a file that is not empty, for `linux/amd64` and `linux/arm64` | 0 of 2 | 2 of 2 | |
| Notices check | the lines `notices <target>` and `notices image` of `release.check.sh --version <version>` | none | 4 lines, each `status: passed` | |
| Main branch rules | `gh api repos/desek/agentloupe/rules/branches/main --jq '[.[].type]'` | 403 | holds `pull_request`, `required_status_checks` with the context `verdict`, `required_linear_history`, `non_fast_forward`, and `deletion` | |
| Tag ruleset | `gh api repos/desek/agentloupe/rulesets`, then the ruleset with the target `tag` | 403 | active, includes `refs/tags/v*`, rules `creation`, `update`, and `deletion` | |
| Merge settings | `gh api repos/desek/agentloupe --jq '[.allow_squash_merge,.allow_merge_commit,.allow_rebase_merge,.delete_branch_on_merge,.allow_auto_merge]'` | true, true, true, false, false | true, false, false, true, true | |
| Actions policy | `gh api repos/desek/agentloupe/actions/permissions --jq .sha_pinning_required` | false | true | |
| Secret scanning | `gh api repos/desek/agentloupe --jq .security_and_analysis` | null | `secret_scanning` and `secret_scanning_push_protection` with status `enabled` | |
| Dependabot alerts | `gh api -i repos/desek/agentloupe/vulnerability-alerts`, the HTTP status | 404 | 204 | |
| Private vulnerability reporting | `gh api repos/desek/agentloupe/private-vulnerability-reporting --jq .enabled` | 404 | true | |
| Idempotence of the settings script | the count of writes on the last line of a second `repo.settings.sh` run, and the exit code of `repo.settings.sh --check` | none, no script | 0 writes, exit code 0 | |
| Length of the community files | `wc -l` of `CONTRIBUTING.md`, `SECURITY.md`, the pull request template, and each issue template | none, no file | at most 80, 60, 30, and 40 lines | |
| Verdict workflow time with the dependency check | the method of the row "Verdict workflow time on a pull request", on a pull request that changes one Rust file | 5 min 13 s and 6 min 39 s (runs 39997262577 and 39997205916) | at most 20 minutes with a cold cache, at most 12 minutes with a warm cache | |
| Owner paths and mockup | `git grep -nE "/Users/[d]esek"`, `git grep -n "Repo/[d]esek" -- .agents/scripts`, and `git ls-files brainstorm` | 5 matches, 1 match, 2 archives | 0, 0, only `brainstorm/agent-ledger.zip` | |
| Remote branches | `git ls-remote --heads origin` | `main`, the release-please branch, and one stale feature branch of the earlier release work | no stale feature branch | |
| Default time zone | `loupe config show` with no `LOUPE_TZ` set | `Europe/Stockholm` | `UTC` with the source default | |
| Archives with an SBOM | `gh release view v<version> --json assets` lists `loupe-v<version>-<target>.cdx.json` and its `.sha256` for each target | 0 of 3 | 3 of 3 || follows the first release with SBOM files; rehearsal on 2026-10-24 in a scratch clone: 3 of 3 archive SBOM files with their `.sha256` files |
| Image platforms with an SBOM | the same list holds `loupe-v<version>-image-<os>-<arch>.cdx.json` and its `.sha256` | 0 of 2 | 2 of 2 || follows the first release; rehearsal on 2026-10-24 from local images (no push): 2 of 2, 90 components each, with the file `/usr/local/bin/loupe` and its digest |
| SBOM coverage of each archive | the component count that `release.build.sh` prints, against the count of distinct crates of `cargo tree --locked --target <triple> -e normal,build --prefix none` minus the root package, at the tag | none | equal for each target || rehearsal at `2ea58a5`: `aarch64-apple-darwin` 184 of 184, `x86_64-unknown-linux-gnu` 183 of 183, `aarch64-unknown-linux-gnu` 183 of 183; equal |
| Verdict time on a squash merge of a green pull request | the duration of the verdict run, from `createdAt` to `updatedAt` of `gh run view <run id> --json createdAt,updatedAt`, on the push of a squash merge of a pull request that was up to date with `main` and had a green verdict; median of 3 merges | 6 min 43 s and 6 min 39 s (push runs 39998277995 and 39997205916 on 2026-10-23; test job 6 min 32 s) | at most 3 minutes, and at most 50 percent of the baseline median || 1 min 4 s on the reuse of a pull request run (run 40039265435 on an empty commit over the tree of run 40038342459, 2026-10-24), 16 percent of the baseline median; the median of 3 squash merges follows the next merges |
| Verdict time on novel content | the same method on a pull request that changes one Rust file | 5 to 7 minutes on 2026-10-23 | at most 1 minute above the baseline median, for the job `reuse`; within the thresholds of the row "Verdict workflow time on a pull request" || 7 min 9 s and 6 min 58 s (pull request runs 40037532419 and 40038342459, 2026-10-24), at most 28 s above the baseline median of 6 min 41 s |
| Lookup time | the duration of the job `reuse` in the run of the row "Verdict time on a squash merge of a green pull request" | none | at most 60 seconds || 9 s (the job `reuse` of run 40039265435); 7 s on a miss |
| Unsound reuse | over each run with the decision `hit` in the reuse scenario and in the first 14 days: the count of the runs where the tree on the machine line differs from the tree hash of the checked-out commit | none | 0 || 0 of 1 hit (run 40039265435: tree `df5162f8…` on the machine line equals `git rev-parse` of the checked-out commit) |
| Reuse in a release | the count of the release runs whose called verdict has a job `reuse` that ran | none | 0 || follows the first release run |
| Write calls of the lookup | the fake `gh` of the shell test records the method of each call of `verdict.reuse.sh lookup` | none | 0 calls with a method other than `GET` || 0 of 21 shell test cases (`tests/scripts/verdict.reuse.test.sh`) |
| Responses in the seeded dry run | the planned action lines of `advisory.respond.sh --dry-run` with a policy file that names a seeded advisory database | none | 1 draft, 1 note for each affected release, 1 issue, and 0 write calls || 2 of 2 runs on 2026-10-24 with a seeded database copy (`RUSTSEC-2099-0001` for `hex` 0.4.3) and a faked release `v0.1.0`: 1 draft (`blocked`, the repository is private), 1 note, 1 issue, 1 label, 0 write calls |
| Deleted assets | before and after each run of `advisory.respond.sh` and of the release workflow: the count of releases, of release assets, of git tags, and of image tags | none | 0 deleted, 0 replaced by the advisory script || 0 deleted, 0 replaced: the repository has no release; no shell test case of the advisory script makes a delete, an upload, or a docker removal |

### Dependency autofix targets

The targets of the dependency autofix. `dependencies.md` states the behaviour. The column Measured is empty until the scenario `.agents/scenarios/deps.autofix.md` and the first 14 scheduled runs are measured. "None" means that the metric does not exist before the flow.

| Metric | Method | Baseline | Threshold | Measured |
|---|---|---|---|---|
| Time of one run with nothing open | the duration of the run, from `createdAt` to `updatedAt` of `gh run view <run id> --json createdAt,updatedAt`, on a run whose summary line counts zero alerts and zero pull requests | none, no workflow | at most 5 minutes | |
| Time of one run with one alert | the same method, on a run with exactly one alert and no Dependabot pull request, from the start to the merge of the release pull request | none, no workflow | at most 120 minutes | |
| Credentials in logs | for each real run of the scenario: download the run log with `gh run view <run id> --log`, the issue text, and the pull request text, then run `gitleaks` on them and count the matches of the patterns `sk-ant-`, `ghp_`, `github_pat_`, `gho_`, `ghs_`, and the literal value of each secret on the machine of the owner | none, no workflow | 0 findings and 0 matches | |
| Share of runs with a defined end | over the first 14 scheduled runs: count the runs that end with exit 0 and a count line of zero, with a merged or auto-merge fix, or with exit 1 and an updated issue; divide by the count of runs | none, no workflow | 14 of 14 (every run) | |
| Tokens of one `claude -p` run | the fields `input_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens`, and `output_tokens` of the result of the run, printed by the script on its item line | none, no run | recorded for each run; no threshold for the first 14 runs | |
| API-equivalent cost of one `claude -p` run | the token counts of the row above and the model of the run, priced with the rules of `cost.md` and the vendored price file, as the dashboard shows a cost; compared with the field `total_cost_usd` of the result | none, no run | at most 5 US dollars for each run; the two values differ by at most 5 percent, or the difference has a stated reason in `dependencies.md` | |

### Start telemetry targets

The targets of the start request and of the telemetry endpoint. `telemetry.md` states the behaviour. The column Measured holds the values of the implementation, measured on 2026-10-23 on macOS arm64 (`.agents/scenarios/telemetry.start.md`). A "fake endpoint" is a local HTTP server on `127.0.0.1` that records each request with its method, its path, its headers, and its body.

| Metric | Method | Baseline | Threshold | Measured |
|---|---|---|---|---|
| Requests per start, state `sent` | count of the GET requests at the fake endpoint after one start of a binary built with the host of the fake endpoint | 0, no feature | exactly 1 || 1 in 2 of 2 scenario runs, and 1 in `telemetry_sends_one_get` |
| Requests per start, state `disabled` | the same, with the run-time opt-out set | 0 | 0 || 0 in 2 of 2 scenario runs |
| Requests per start, state `built-out` | the same, with a binary built with no telemetry host | 0 | 0 || 0 in 2 of 2 scenario runs and in `telemetry_built_out_state` |
| Retries after an error or a timeout | count of the requests at a fake endpoint that answers 500, and at a fake endpoint that holds the request open | 0 | no request after the first attempt || 500: 1 request on 1 connection; held open: 1 request on 1 connection; refused: 1 debug line; redirect target: 0 requests |
| Time to give up | `cargo test telemetry_gives_up_after_timeout` | none | the send ends in at most 5.5 seconds || 5.03 s (5.028 s in the verdict) |
| Start delay added | the time from the process start to the first 200 of `GET /healthz`, median of 5 starts with a fake endpoint that holds the request open, against the median of 5 starts with the run-time opt-out set | measured at the start of the implementation | a difference of at most 50 ms || baseline 146.5 ms; held open 142.5 and 142.9 ms against 144.1 and 145.0 ms with the opt-out: a difference of -1.6 and -2.1 ms |
| Request shape | the method, the path, the body size, and the query of the request at the fake endpoint | none | `GET`; `/` and 40 lowercase hex characters equal to the commit of the build; 0 bytes; no query || `GET`; `/` and the 40-character commit of `HEAD`; 0 bytes; no query (2 of 2 runs) |
| User agent | the `user-agent` header at the fake endpoint | none | exactly `agentloupe/<version of Cargo.toml> (<target triple of the build>)` || `agentloupe/0.1.0 (aarch64-apple-darwin)`, equal to the expected value |
| Host in a built-out binary | count of the matches of a random marker host name in a binary built with no telemetry host, after an earlier build with the marker as the host | none | 0 matches || 0 matches (2 of 2 runs), and 0 matches of the commit |
| Telemetry rows in the product data | count of the rows in the exchange table and of the live events after a start in the state `sent` | 0 | 0 || 0 exchanges; 0 live events (one `snapshot` with `calls` 0 and `in_flight` 0 while the request was open) |
| Worker response time | the total time of each of 10 GETs of the deployed host with a test commit, from the owner's machine | none, no endpoint | median at most 500 ms, each at most 2 s || median 82 ms, each at most 109 ms |
| Rows after N starts of one build | count of the rows for the commit of the build in the remote database `agentloupe-telemetry`, read with `wrangler`, after N starts | 0, no store | exactly 1 row, and `count` equals N || 1 row and `count` 2 after 2 starts, for 2 of 2 builds |
| Count increment per start | `count`, `firstseen`, and `lastseen` of the row before and after one more start, read with `wrangler` | none | `count` exactly 1 more, `firstseen` unchanged, `lastseen` the UTC date of the start || `count` +1, `firstseen` unchanged, `lastseen` `2026-10-23`, the UTC date |
| Writes by a refused request | the row count and each `count` before and after the refused requests of the Worker test | none | unchanged, and each answer 404 || unchanged; 8 of 8 refused requests of the Worker test and 3 of 3 on the deployed host answer 404 |
| Stored columns | the column list of the table `starts` | none | exactly 4: `sha`, `firstseen`, `lastseen`, `count` || 4: `sha`, `firstseen`, `lastseen`, `count` |
| New tests | count of the eight Rust tests of the start request that pass in the verdict, and of the Worker test cases that pass with `telemetry.deploy.sh --test` | 0 | 8 of 8 Rust tests, 3 of 3 Worker cases || 8 of 8 Rust tests, 3 of 3 Worker cases |
| Deploy idempotence | a second run of `telemetry.deploy.sh` directly after the first | none | exit code 0, 1 database of the name, 0 migrations applied, the same host || exit 0, 1 database of the name, 0 migrations applied, the same host |
| Dry-run changes | the database list of the account and the git status of the repository, before and after `telemetry.deploy.sh --dry-run` | none | no difference || no difference |
| Repository secrets | count of the secrets in the repository settings | 1 | 1 || 1 |
| Opt-out name in the user documentation | count of the matches of the name of the run-time opt-out variable in `README.md`, the files under `docs/specification/`, `.env.example`, `compose.yaml`, `.taxonomy`, the files under `telemetry/`, and the output of `loupe config show` | 0 | 0 || 0 |

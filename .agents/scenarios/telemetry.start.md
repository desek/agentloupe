---
date: 2026-10-23
source: cr (CR-0026)
surface: cli, http
outcome: intended
runs: "Part A 1 of 1 (consented), Part B 2 of 2, Part C 2 of 2 passed, last run 2026-10-23"
intent_code: "src/telemetry/mod.rs:start, build.rs:telemetry, telemetry/src/index.js:fetch"
intent_test: "tests/telemetry.send.rs:telemetry_sends_one_get, tests/telemetry.built_out.rs:telemetry_built_out_state, telemetry/test/worker.test.mjs:insert"
coverage_gap: false
---

# Scenario: start telemetry

- Goal: a release-like build of `loupe` sends one body-less GET of `https://<host>/<commit>` when `loupe serve` starts, the telemetry endpoint counts it per commit, the run-time opt-out stops it, and a build with no host sends nothing; the owner deploys the endpoint with one idempotent script.
- Surface: the `loupe` binary (`config show`, `serve`, its log), `.agents/scripts/telemetry.deploy.sh`, the deployed Worker over HTTPS, and the remote D1 database read with `wrangler d1 execute`.
- Success condition: each check below holds; Part A and Part B write only to the owner's store with the owner's consent; Part C passes in 2 of 2 runs.
- Source: the acceptance criteria and the section "Model-Based Testing" of `docs/cr/CR-0026-start-telemetry.md`; the rules of `docs/specification/telemetry.md`.

## Preconditions

- `wrangler` 4.100.0 and Node 22.20.0 through mise (`.tool-versions`), `wrangler` logged in to the owner's account.
- The owner's consent for the deploy of the Worker `agentloupe-telemetry` and the database `agentloupe-telemetry`, and for the writes of Part B.
- A clean, committed `HEAD`. The scenario builds into a scratch target directory (`CARGO_TARGET_DIR=/tmp/cr26/target`), so the verdict build in `target/` stays built-out.
- The fake endpoint and the start delay measurement are `.agents/scripts/telemetry.probe.py fake` and `startdelay`.
- A helper `start <bin> [NAME=VALUE ...]`: make a data directory and two keys with `<bin> gen-key`, run `env -i PATH=$PATH LOUPE_LISTEN=127.0.0.1:<free port> LOUPE_DATA_DIR=<dir> LOUPE_DATA_KEY=<k1> LOUPE_ACCOUNT_KEY=<k2> NAME=VALUE <bin> serve` from the repository root, poll `GET /healthz` until 200, wait 2 seconds (1 second in hold mode), check `GET /healthz` again, send SIGTERM, and print the exit code, the stop time, the lines `listening` and `telemetry`, each warning, and the row count of `exchanges`. The keys are never printed.

## Steps

### Part A, the endpoint (once, with the owner's consent)

1. `.agents/scripts/telemetry.deploy.sh --test`: 3 of 3 Worker cases pass, exit 0.
2. `git status --porcelain` and `wrangler d1 list --json` before and after `.agents/scripts/telemetry.deploy.sh --dry-run`: the same; the dry run lists the steps, `wrangler deploy --dry-run` passes, exit 0. A pin of `4.54.0x` in a scratch `.tool-versions` gives exit 3; an empty `XDG_CONFIG_HOME` (no login) gives exit 4.
3. `.agents/scripts/telemetry.deploy.sh`: exit 0, one database made, its ID written to `telemetry/wrangler.toml`, 1 migration applied, and the host printed.
4. Run it again: exit 0, `exists ... none created`, `0 applied`, the same host; `wrangler d1 list` holds 1 database of the name.
5. `curl` of `GET /`, `POST /<sha>`, and `HEAD /<sha>` on the host: 404 each. `pragma_table_info('starts')`: exactly `sha`, `firstseen`, `lastseen`, `count`.
6. `gh variable set LOUPE_TELEMETRY_HOST --body <host> --repo desek/agentloupe`; `gh secret list` still has 1 secret.

### Part B, a local build against the deployed host

7. `LOUPE_TELEMETRY_HOST=<host> cargo build --bin loupe` from the committed `HEAD`, with `LOUPE_BUILD_COMMIT` unset.
8. `loupe config show`: the line `telemetry` is `built-in host=<host>` with the source `build`; 0 matches of the opt-out name, with and without it set.
9. `start` once: `GET /healthz` 200, the line `telemetry` with `state` `sent` and the host after the line `listening`, 0 exchanges. Read the row of `git rev-parse HEAD` with `wrangler d1 execute agentloupe-telemetry --remote`: `count` 1, both dates the UTC date.
10. `start` again: `count` 2, the same `firstseen`, `lastseen` the UTC date, one row.
11. `start` with the run-time opt-out `=1`: `state` `disabled`, the row unchanged.
12. `LOUPE_TELEMETRY_HOST= cargo build`: `config show` gives `built-out`, 0 matches of the host in the binary, `start` gives `state` `built-out` and the row unchanged.
13. Ten `curl -w '%{time_total}'` GETs of a test `sha` (`feedface` × 5) on the host, then `DELETE FROM starts WHERE sha='<test sha>'`.

### Part C, the binary against a local fake endpoint (2 runs)

14. Fake A on `127.0.0.1:18899` and fake B on `127.0.0.1:18898`, answer mode. Build with `LOUPE_TELEMETRY_HOST=127.0.0.1:18899`. `start` with `LOUPE_TELEMETRY_HOST=127.0.0.1:18898` in the run-time environment: A gets exactly one `GET`, the path `/` and `git rev-parse HEAD`, no query, 0 body bytes, the user agent `agentloupe/<Cargo.toml version> (<rustc host>)`, only the headers `accept`, `host`, `user-agent`; B gets 0; 0 exchanges; `GET /healthz` 200.
15. `start` with the opt-out `=1`: `state` `disabled`, no new request at A. With the opt-out `=true`: `state` `sent`, one new request.
16. Fake A in hold mode: `start` gives `GET /healthz` 200 while the request is open, then SIGTERM exits 0 at once; A got one request. A live client on `/live` during an open request gets one `snapshot` with `calls` 0 and `in_flight` 0, and no other frame in 3 seconds.
17. Start delay: `telemetry.probe.py startdelay --bin <bin> --runs 5` with fake A in hold mode, and again with `--env <opt-out>=1`; compare the medians.
18. `LOUPE_BUILD_COMMIT=1111…` (40 characters) with the host: the path is that commit.
19. Build with a random `marker-<hex>.invalid` host, then with the variable unset: 1 match of the marker in the first binary, 0 in the second, 0 matches of the commit; `config show` gives `built-out`; `start` with the host of A in the run-time environment gives `state` `built-out` and no request.
20. Builds with `https://example.dev`, `example.dev/p`, `u:pw@example.dev`, `example.dev?q=1`: each stops (exit 101) with an error that names `LOUPE_TELEMETRY_HOST`, and no error holds the value. A `git archive` copy with no `.git` and a host stops with the error that names `LOUPE_BUILD_COMMIT`.
21. Rebuild rule: build with the host of A at a commit, commit, build again with the same environment: cargo reports the ref file of the branch as changed, and the binary holds the new commit and not the old one.

## Run log

**Part A, 2026-10-23 (once, consented).** `--test`: 3 Worker cases passed. `--dry-run`: exit 0, `git status` and `d1 list` (`[]`) unchanged; exit 3 with the bad pin; exit 4 with no login. First real run: `created, ID 4d739d08-6823-4a7f-845f-8fef1b103130`, `1 applied`, host `agentloupe-telemetry.daniel-95c.workers.dev`. Second run: `exists ... none created`, `0 applied`, the same host, 1 database of the name. `GET /`, `POST /<sha>`, `HEAD /<sha>`: 404 each. Columns: `sha`, `firstseen`, `lastseen`, `count`. Repository variable set; secrets 1 before and after.

**Part B, run 1, `HEAD` `eab56f0`.** `config show`: `built-in host=agentloupe-telemetry.daniel-95c.workers.dev`, source `build`, 0 opt-out matches. Start 1: `sent`, row `count` 1, `firstseen` = `lastseen` = `2026-10-23`, 0 exchanges. Start 2: `count` 2, one row. Opt-out `=1`: `disabled`, `count` 2. Worker response time over 10 GETs: median 0.082 s, max 0.109 s, each 204; the test row (count 10) deleted. (The built-out step of this run is Part C run 1, step 19.)

**Part B, run 2, `HEAD` `e487306`.** The same sequence with the scripted helper: row `[]` before; `sent` → `count` 1; `sent` → `count` 2; `disabled` → `count` 2; built-out build: `config show` `built-out`, 0 matches of the host in the binary, `state` `built-out`, `count` 2.

**Part C, runs 1 and 2 (`HEAD` `eab56f0` and `e487306`).** Each run: A 1 request, `GET`, path = `HEAD`, query empty, 0 body bytes, user agent `agentloupe/0.1.0 (aarch64-apple-darwin)`, headers `accept`, `host`, `user-agent`; B 0; 0 exchanges. Opt-out `=1`: 0 new; `=true`: 1 new. Hold: healthz 200 twice while open, exit 0 in 0.01 to 0.02 s, 1 request. Start delay medians, hold against opt-out: 142.5 against 144.1 ms (run 1), 142.9 against 145.0 ms (run 2); the baseline before the change was 146.5 ms. Override commit: path `/1111…`. Marker: 1 match in the marker build, 0 in the built-out build, 0 matches of the commit, `built-out`, 0 requests. Bad values: exit 101 and 1 error naming the variable each, 0 echoes. No `.git`: exit 101 with the commit error. No process left. (A first attempt of run 1 is not counted: the harness did not wait for the answer-mode fake to stop, so the hold-mode fake could not bind its port; the harness now waits.) Live check: 1 `snapshot` frame, `calls` 0, `in_flight` 0, while the request was held. Rebuild rule: `Dirty ... the file .git/refs/heads/feat/cr-0026-telemetry has changed`; new `HEAD` 1 match, old `8876e40` 0 matches.

**Outcome.** `intended`: the behaviour comes from `src/telemetry/mod.rs:start` (the state, the line, the task), `build.rs:telemetry` (the host, the commit, the target, the rerun rules), and `telemetry/src/index.js:fetch` (the upsert and the 404); the Rust tests of `tests/telemetry.send.rs`, `tests/telemetry.built_out.rs`, and the unit tests in `src/telemetry/`, and the Worker cases of `telemetry/test/worker.test.mjs`, assert it.

**Rows kept in the store.** The build commits `eab56f0…` and `e487306…` keep their rows (count 2 each); they are commits of the development branch, which the owner can delete with `wrangler d1 execute agentloupe-telemetry --remote --command "DELETE FROM starts WHERE sha IN (...)"`.

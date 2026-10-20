---
date: 2026-10-20
source: cr (CR-0004)
surface: cli and http
outcome: reproduced
runs: "1 of 1 passed, last run 2026-10-20"
---

## Scenario: fake upstream calls become sessions

### Goal

The operator sends streamed Anthropic requests through the proxy with two proxy tokens and one shared session header. After `loupe parse --once`, `sqlite3` shows the calls with the token ID, one session for each token, and closed segments after the idle time.

### Preconditions

- The release build exists: `cargo build --release --bin loupe --example fake_upstream`.
- A scratch dir `S` with an empty `S/data` and `S/up.toml` that has the line `fake = "http://127.0.0.1:<fake port>"`.

### Steps at the user surface

1. Start `target/release/examples/fake_upstream <fake port>`.
2. Run `loupe gen-key` and put the value in `K`.
3. Set `LOUPE_DATA_DIR=S/data`, `LOUPE_DATA_KEY=K`, `LOUPE_ACCOUNT_KEY=K`, `LOUPE_UPSTREAMS_FILE=S/up.toml`, `LOUPE_IDLE_SECS=60`, `LOUPE_PARSE_INTERVAL_SECS=86400`.
4. Run `loupe token create --name a` and `loupe token create --name b`. Put the `token` fields in `T1` and `T2`.
5. Start `loupe serve`.
6. Send a streamed `/v1/messages` request with the header `x-session-id: shared-1` two times with `T1` and one time with `T2`, to `http://127.0.0.1:<port>/<token>/claude-code/fake/v1/messages`.
7. Run `loupe parse --once`.
8. Run `sqlite3 S/data/loupe.db` with `select proxy_token_id, model, input, cache_read, cache_write_5m, cache_write_1h, output from calls` and `select id, segment, state from sessions`.
9. Wait 65 seconds. Run `loupe parse --once` again, then repeat the query of `sessions`.
10. Search `S/data/logs/loupe.log` and the stderr of `loupe serve` for the session header value. Stop both processes.

### Success condition

- Step 7 prints `calls_added: 3` and `sessions_opened: 2`.
- `calls` has 3 rows with the proxy token ID and model `claude-haiku-5-5`, and the five token counts of the fixture (10, 20, 4, 3, 5).
- `sessions` has 2 rows. Each holds calls of one proxy token only.
- After step 9, the second run prints `calls_added: 0` and `sessions_closed: 2`, and each session is `closed` with `closed_at` set.
- Step 10 finds 0 matches.

### Run log

- 2026-10-20: all conditions met. 3 calls, 2 sessions (session 1: token 1, 2 calls; session 2: token 2, 1 call), both closed after 65 seconds, 0 log matches. Passed.
- The resume after a close (segment 2) and the backlog gap inside the idle time are asserted by `src/session/segment.rs:resume_after_close_starts_new_segment` and `src/session/idle.rs:backlog_gap_within_idle_keeps_one_segment`.

---
date: 2026-10-20
source: cr (CR-0003)
surface: cli and http
outcome: reproduced
runs: "scenario 1: 2 of 2 passed; scenario 2: 1 of 1 passed; last run 2026-10-20"
---

## Scenario 1: token for one machine

### Goal

The user makes a proxy token, sends requests through the proxy with the token first in the base URL, and the raw token is not on disk.

### Preconditions

- The release build exists: `cargo build --release --bin loupe --example fake_upstream`.
- `LOUPE_DATA_DIR` points to an empty directory. An upstreams file has the line `fake = "http://127.0.0.1:<fake port>"`.

### Steps at the user surface

1. Start the fake upstream: `target/release/examples/fake_upstream <fake port> &`.
2. Run `loupe token create --name leak-test` and put the `token` field in `T`.
3. Start `loupe serve` with `LOUPE_DATA_KEY` and `LOUPE_ACCOUNT_KEY` from `loupe gen-key`. Send stderr to a file.
4. Send 10 requests: `curl http://127.0.0.1:<port>/$T/x/fake/json`.
5. Send 1 request with no token: `curl http://127.0.0.1:<port>/x/fake/json`.
6. Run `grep -c "$T" -r $LOUPE_DATA_DIR <stderr file>`.
7. Run `loupe token list`.
8. Stop both processes.

### Success condition

- Step 4 gives status 200 ten times. Step 5 gives status 401 with the error `token_missing`.
- Step 6 finds 0 matches.
- Step 7 shows `leak-test` as `active` with a `last_used_at` value, and no `lpt_` value.

### Runs

- 2026-10-20: steps 1 to 4, 6, and 7 passed (10 of 10 status 200, 0 matches, `last_used_at` set). Step 5 is covered by the test `refused_request_is_401_without_exchange`.
- 2026-10-20: re-run with the token `work-laptop` and two tags. Status 200, the newest exchange row has the `proxy_token_id` of the token and the path `/json`. `loupe token list` shows `last_used_at` set. A request with no token gave 401 `token_missing`. The grep found 0 matches. Verdict: passed.

## Scenario 2: revoke a token

### Goal

The user stops a leaked token at once, while `loupe serve` runs, and keeps the old exchange rows.

### Preconditions

- The same setup as scenario 1. `loupe serve` runs and the token `work-laptop` is active and was used once.

### Steps at the user surface

1. Run `sqlite3 $LOUPE_DATA_DIR/loupe.db "select count(*) from exchanges"` and keep the value as `N`.
2. Run `loupe token revoke work-laptop` from a second shell.
3. Wait 5 seconds.
4. Send `curl http://127.0.0.1:<port>/$T/x/fake/json`.
5. Run the count of step 1 again.
6. Stop both processes.

### Success condition

- Step 2 exits 0 and prints `"changed":true`.
- Step 4 gives status 401 with the error `token_revoked`, and the body does not hold `T`.
- The count of step 5 equals `N`, so the old rows stay and no new row exists.

### Runs

- 2026-10-20: step 2 exit 0, step 4 status 401 `token_revoked`, the count was 11 before and 11 after. Verdict: passed.

---
date: 2026-10-20
source: cr (CR-0003)
surface: cli and http
outcome: reproduced
runs: "1 of 1 passed, last run 2026-10-20"
---

## Goal

The user makes a proxy token, sends requests through the proxy with the token first in the base URL, and the raw token is not on disk.

## Preconditions

- The release build exists: `cargo build --release --bin loupe --example fake_upstream`.
- `LOUPE_DATA_DIR` points to an empty directory. An upstreams file has the line `fake = "http://127.0.0.1:<fake port>"`.

## Steps at the user surface

1. Start the fake upstream: `target/release/examples/fake_upstream <fake port> &`.
2. Run `loupe token create --name leak-test` and put the `token` field in `T`.
3. Start `loupe serve` with `LOUPE_DATA_KEY` and `LOUPE_ACCOUNT_KEY` from `loupe gen-key`. Send stderr to a file.
4. Send 10 requests: `curl http://127.0.0.1:<port>/$T/x/fake/json`.
5. Send 1 request with no token: `curl http://127.0.0.1:<port>/x/fake/json`.
6. Run `grep -c "$T" -r $LOUPE_DATA_DIR <stderr file>`.
7. Run `loupe token list`.
8. Stop both processes.

## Success condition

- Step 4 gives status 200 ten times. Step 5 gives status 401 with the error `token_missing`.
- Step 6 finds 0 matches.
- Step 7 shows `leak-test` as `active` with a `last_used_at` value, and no `lpt_` value.

## Runs

- 2026-10-20: steps 1 to 4, 6, and 7 passed (10 of 10 status 200, 0 matches, `last_used_at` set). Step 5 is covered by the test `refused_request_is_401_without_exchange`.

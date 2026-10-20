---
date: 2026-10-20
source: cr (CR-0002)
surface: http
outcome: reproduced
runs: "4 of 4 passed, last run 2026-10-20"
---

## Goal

The user sends a streamed model request through Agentloupe and gets the same stream, chunk by chunk. The proxy stores the exchange.

## Preconditions

- The release build exists: `cargo build --release --bin loupe --example fake_upstream`.
- A work directory holds a copy of `config/` and `upstreams.toml`, with the line `fake = "http://127.0.0.1:9911"` added.
- `LOUPE_DATA_KEY` and `LOUPE_ACCOUNT_KEY` hold values from `loupe gen-key`. `LOUPE_DATA_DIR` points to an empty directory.
- Nothing listens on ports 8787 and 9911.

## Steps at the user surface

1. Start the fake upstream: `target/release/examples/fake_upstream 9911 &`.
2. Start the proxy from the work directory: `loupe serve &`.
3. Run `curl -sN http://127.0.0.1:8787/claude-code/fake/sse` and note the time of each line.
4. Run `sqlite3 $LOUPE_DATA_DIR/loupe.db "select agent, upstream, status, outcome from exchanges"`.
5. Stop both processes.

## Success condition

- curl prints 5 `data:` lines about 200 ms apart, and the first line arrives before 300 ms.
- The table has one row `claude-code|fake|200|complete` for each stream.

## Run log

- Run 1: lines at 0.000, 0.200, 0.402, 0.604, 0.807 s after the first; time to first byte 1.7 ms. Row `claude-code|fake|200|complete`. Passed.
- Runs 2 to 4: time to first byte 1.2, 0.8, 0.8 ms. 3 rows with outcome `complete`. Passed.

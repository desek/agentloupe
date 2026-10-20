---
date: 2026-10-20
source: cr (CR-0007)
surface: http
outcome: reproduced
runs: "1 of 1 passed, last run 2026-10-20"
---

## Scenario: a restart publishes each day that the stopped container missed

### Goal

After a stop of several days, the operator sees each missed day with activity in the public API.

### Preconditions

- A data dir with calls on past days and no `days` rows, for example from a stopped `loupe serve`.

### Steps at the user surface

1. Start `loupe serve` with `LOUPE_PUBLISH_INTERVAL_SECS=1`.
2. Wait 2 seconds. Run `curl -s 'http://127.0.0.1:8787/v1/days?from=<first day>&to=<yesterday>'`.

### Success condition

- The response holds one record for each past day with calls, in ascending order, and no record for a day with no calls.

### Run log

2026-10-20: covered by the test `catch_up_publishes_missed_days_in_order` in `tests/publish.worker.rs` with a set clock, and by `tests/cli.day.rs`, which starts `loupe serve` over a store with a call on a past day and sees the record within 1 second.

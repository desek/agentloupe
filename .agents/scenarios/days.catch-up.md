---
date: 2026-10-20
source: cr (CR-0007)
surface: http
outcome: reproduced
runs: "2 of 2 passed, last run 2026-10-20"
---

## Scenario: a restart publishes each day that the stopped container missed

### Goal

After a stop of several days, the operator sees each missed day with activity in the public API.

### Preconditions

- A data dir with calls on past days and no `days` rows. The verification run made it with `loupe parse --once` on an empty data dir, then one `sessions` row and one `calls` row for each of 2026-10-14, 2026-10-16, and 2026-10-17 with `sqlite3`, as `tests/common/days.rs` does.
- The clock is the real wall clock. A past day is already due, so no injected clock or test variable is necessary.

### Steps at the user surface

1. Start `loupe serve` with `LOUPE_PUBLISH_INTERVAL_SECS=1`.
2. Wait 2 seconds. Run `curl -s 'http://127.0.0.1:8787/v1/days?from=<first day>&to=<yesterday>'`.

### Success condition

- The response holds one record for each past day with calls, in ascending order, and no record for a day with no calls.

### Run log

2026-10-20: covered by the test `catch_up_publishes_missed_days_in_order` in `tests/publish.worker.rs` with a set clock, and by `tests/cli.day.rs`, which starts `loupe serve` over a store with a call on a past day and sees the record within 1 second.

2026-10-20, release binary, `LOUPE_TZ=UTC`, `LOUPE_PUBLISH_INTERVAL_SECS=1`, real wall clock: before the start, `days` held 0 rows. After 3 seconds the log had 3 `day published` lines with `reason` `due` and `revision` 1. `GET /v1/days?from=2026-10-13&to=2026-10-19` returned `["2026-10-14","2026-10-16","2026-10-17"]` in ascending order, with no record for 2026-10-15. After a second start over the same data dir, `days` held 3 rows, 3 distinct days, maximum revision 1, so no day is there two times.

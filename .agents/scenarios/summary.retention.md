---
date: 2026-10-20
source: cr (CR-0006)
surface: cli
outcome: reproduced
runs: "2 of 2 passed; last run 2026-10-20"
---

## Scenario: bodies go away after retention

### Goal

The operator confirms that prompts do not stay on disk after the parse and the summary.

### Preconditions

- The same as `summary.live.md`.

### Steps at the user surface

1. Run `.agents/scripts/summary.live.sh`. Its last step runs `loupe summarize --once` with `LOUPE_RAW_RETENTION_HOURS=0`.

### Success condition

- The output has `bodies_left: 0`: no exchange with the parse outcome `call` has a null `bodies_deleted_at`.
- The output has `calls_unchanged: true` and `sessions_unchanged: true`.

### Result on 2026-10-20

- `bodies_left: 0`, `calls_unchanged: true`, `sessions_unchanged: true`.
- Second run (regression): `bodies_left: 0`, `calls_unchanged: true`, `sessions_unchanged: true`.

---
date: 2026-10-20
source: cr (CR-0006)
surface: cli, with a real agent and a real summary request
outcome: reproduced
runs: "1 of 2 passed; the first run failed with bad_request (temperature refused), the second passed after the change; last run 2026-10-20"
---

## Scenario: a real session gets a safe line

### Goal

The operator sees one non-identifying line for a finished `claude -p` session.

### Preconditions

- Claude Code 2.1.304 is logged in.
- `.env` at the repository root holds `LOUPE_SUMMARY_OAUTH_TOKEN`. Do not print it.

### Steps at the user surface

1. Run `.agents/scripts/summary.live.sh`.

### Success condition

- The output has `status: done`, `failed: 0`, and `line_safe: yes`.
- In the data dir, the newest segment has a `session_summaries` row in status `done`. Its line has at most 12 words, no digit, no `/`, and no `@`.

### Result on 2026-10-20

- First run: `status: failed`, error kind `bad_request`. The API refused `temperature` for `claude-haiku-5-5`, and without the billing block the OAuth path gave a `429` with no `retry-after`.
- Second run, with no `temperature` and with the billing block: `status: done`, `failed: 0`, a line of 8 words, `line_safe: yes`.

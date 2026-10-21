---
date: 2026-10-21
source: bug report (the day lines of 2026-10-17 and 2026-10-19 were empty after a 7-day re-summary)
surface: cli
outcome: reproduced
runs: "2 of 2 passed after the fix, last run 2026-10-21"
---

## Scenario: a backfilled multi-day session gets its summaries on every day

### Goal

An operator who backfills one Claude Code project over three days gets one done full session summary and one done session day summary for each day, so that no day line stays empty because the model answered without text.

### Preconditions

- The debug build exists: `cargo build --bin loupe`. Run every command from the repository root.
- `.env` holds `CLAUDE_CODE_OAUTH_TOKEN`, `LOUPE_DATA_KEY`, and `LOUPE_ACCOUNT_KEY`. The run sources it and prints no value.
- `R` is a scratch root that holds one symbolic link to one Claude Code project directory under `~/.claude/projects` with a session that spans 2026-10-16 to 2026-10-18.
- `D` is an empty scratch data dir.

### Steps at the user surface

1. Run, with `LOUPE_DATA_DIR=D`: `loupe backfill --source claude-code --path R --from 2026-10-16 --to 2026-10-18`. Read the `summaries_done` and `summaries_skipped` lines.
2. Run `loupe summarize --rebuild-days 2026-10-16..2026-10-18`, then step 1 again, for a second run.
3. Read the `session text` lines of `D/logs/loupe.log` and count the status values per table and day.
4. Read `grep -c "model response without text" D/logs/loupe.log`.

### Success condition

Each run prints `summaries_skipped: 0`, every `session text` line of both tables has the status `done`, and no line says `model response without text` with a final `skipped` row after it.

### Run log

- Before the fix, run 1 (22:44): `summaries_done: 0`, `summaries_skipped: 4`. Run 2: `summaries_done: 2`, `summaries_skipped: 2`. Every skipped row that followed a request had `raw_chars 0`: the response held one `thinking` block, `stop_reason max_tokens`, `output_tokens 600`. 6 of 14 requests. Failed.
- After the fix (thinking off in the request, a response without text retries with the error kind `no_text`), run 1 (22:48): `summaries_done: 4`, `summaries_skipped: 0`, 16 `session text` lines all `done`. Passed.
- After the fix, run 2 (22:49): the same. Passed.

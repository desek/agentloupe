---
date: 2026-10-21
source: cr (CR-0014)
surface: cli
outcome: reproduced
runs: "3 of 3 passed, last run 2026-10-21"
---
<!-- @agents-index User scenario: loupe backfill of a synthetic three-day session gets three full and three day texts, day lines count it, no transcript stored. -->
# Scenario: a three-day session gets one text for each day

Goal: the operator runs `loupe backfill` on a session with calls on three days. The operator gets a full session summary and a session day summary for each day, and a day line for each day that counts the session.

Preconditions: a release binary (`cargo build --release`) and Python 3. The script makes a scratch data dir, the test keys, `LOUPE_TZ=UTC`, and a fake summary upstream. The script uses no real credential.

Steps:

1. Run `python3 -I .agents/scripts/summary.session_days.scenario.py`.
2. The script runs `loupe backfill --source claude-code --path <synthetic logs>`. Then it runs `loupe summarize --day D` for 2026-03-02, 2026-03-03, and 2026-03-04.

Success condition: `session_full_summaries` and `session_day_summaries` each hold 3 `done` rows for the session. `session_lines` is 1 on each day, because one session has a call on each day. The marker word of the prompts is in no file of the data dir and not in the backfill output. The script prints `status=passed`.

Run log, 2026-10-21:

- Run 1: `full_done=3 day_done=3 day_lines=1,1,1 marker_in_db=0 status=passed`.
- Run 2: the same.
- Run 3: the same.

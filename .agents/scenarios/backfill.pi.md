---
date: 2026-10-21
source: cr (CR-0012)
surface: cli
outcome: reproduced
runs: "1 of 1 passed, last run 2026-10-21"
---
<!-- @agents-index User scenario: loupe backfill over the real Pi session logs, pi calls with origin log, skipped entries at most 1 percent. -->
# Scenario: backfill of the real Pi logs

Goal: the operator sees the Pi calls from before the proxy, through the `loupe backfill` command.

Preconditions: as in `backfill.claude-code.md`. The command reads only `~/.pi/agent/sessions`.

Steps:

1. Run `loupe backfill --source pi --dry-run`.
2. Run `loupe backfill --source pi --from 2026-10-17 --to 2026-10-21`.
3. Run the step 2 command again.
4. Query `calls` with `sqlite3` for `agent_label = 'pi'` and `origin = 'log'`.

Success condition: the dry run prints `calls_added` above 0 and `entries_skipped` at most 1 percent of `entries_read`. Calls with `agent_label = 'pi'` and `origin = 'log'` exist. The second run prints `calls_added: 0`.

Run log, 2026-10-21:

- Dry run: `entries_read` 23282, `calls_added` 10450, `entries_skipped` 0.
- Real run: `calls_added` 1, `summaries_done` 1. The call has agent label `pi`, origin `log`, and a null account.
- Second run: `calls_added` 0, `calls_existing` 1.

---
date: 2026-10-21
source: cr (CR-0012)
surface: cli
outcome: reproduced
runs: "1 of 1 passed, last run 2026-10-21"
---
<!-- @agents-index User scenario: loupe backfill over the real Claude Code logs, origin log calls, summaries, idempotent second run, no log content stored. -->
# Scenario: backfill of the real Claude Code logs

Goal: the operator sees the Claude Code calls from before the proxy, through the `loupe backfill` command.

Preconditions: a release binary, a scratch `LOUPE_DATA_DIR`, the keys and the summary credential from `.env` (`set -a; . ./.env; set +a`). Do not print the credential.

Steps:

1. Run `loupe backfill --source claude-code --dry-run`.
2. Run `loupe backfill --source claude-code --from 2026-10-20 --to 2026-10-21`.
3. Run the step 2 command again.
4. Query `calls`, `session_summaries`, and `agg_queue` with `sqlite3`.
5. Dump the database. Search the dump for the slug of this repository, `/Users/`, and a known word of a prompt.

Success condition: the dry run prints `calls_added` above 0 and `entries_skipped` at most 1 percent of `entries_read`. The real run adds calls with `origin = 'log'` and a null account, writes summary rows, and queues days. The second run prints `calls_added: 0`. The dump search finds 0 matches.

Run log, 2026-10-21:

- Dry run: `entries_read` 274673, `calls_added` 38088, `entries_skipped` 0, `sessions_already_captured` 0.
- Real run: `calls_added` 2462, `days_queued` 2, `summaries_done` 70. All 2462 calls have origin `log` and a null account.
- Second run: `calls_added` 0, `calls_existing` 2462.
- `session_summaries` rows: 260 (closed segments of the run). `agg_queue` rows: 3.
- Dump search: 0 matches for each search term.
- The scratch database had no proxied session, so the check for no proxied session key is proven by the tests `captured_session_is_skipped` and `only_new_session_is_added`.

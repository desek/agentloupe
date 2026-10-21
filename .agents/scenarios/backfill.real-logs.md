<!-- @agents-index User scenario: loupe backfill over the real Claude Code and Pi logs of this machine, dry run then a limited real run. -->
# Scenario: backfill of the real session logs

Goal: the operator adds the calls of the session logs on this machine to a scratch database, with no log content stored.

Success condition: each dry run prints `calls_added` above 0 and writes no schema; a real run limited with `--from` and `--to` adds calls with origin `log`, a null account, queued days, and summary rows; a second identical run prints `calls_existing` for each call of the first run; a dump of the database holds no cwd slug, no path, no raw session ID, and no prompt text.

Steps:

1. Run `loupe backfill --source claude-code --dry-run` and `--source pi --dry-run` with a scratch `LOUPE_DATA_DIR`.
2. Run `loupe backfill --source claude-code --from 2026-10-20 --to 2026-10-21` with the summary credential, two times.
3. Run `loupe backfill --source pi --from 2026-10-17 --to 2026-10-18`, two times.
4. Dump the database with `sqlite3 .dump`. Search the dump for the cwd slug of this repository, `/Users/`, a raw session ID, and a known word of a prompt.

Result on 2026-10-21, 1 of 1 attempt passed:

- Dry run: `claude-code` 38070 calls of 274434 entries, `pi` 10450 calls of 23282 entries, `entries_skipped` 0, no schema written.
- Real run `claude-code`: `calls_added` 2448, `days_queued` 2, `summaries_done` 77, `summaries_skipped` 171. Second run: `calls_existing` 2448, `calls_added` 3 (new calls of the live session of this run).
- Real run `pi`: `calls_added` 1, `summaries_done` 1. Second run: `calls_added` 0, `calls_existing` 1.
- All calls have origin `log` and a null account. The `claude-code` calls have a price; the `pi` call is unpriced, because its provider is not in `upstreams.toml`.
- The dump search found 0 matches for each search term.

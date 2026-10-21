# Scenario: a Rust edit gets the full proof

- Goal: a Rust change is never trusted from the cache.
- Surface: the command `.agents/scripts/pipeline.verdict.sh`.
- Success condition: format, lint, and test have `status: passed` with test seconds above 1, image has `status: passed`, total at most 40 seconds on a warm cache. A failing assertion gives `status: failed` and exit code 1 on two runs in a row.

## Steps

1. Run `.agents/scripts/pipeline.verdict.sh` until it prints `verdict: passed`.
2. Append a comment line to `src/lib.rs`.
3. Run `.agents/scripts/pipeline.verdict.sh` again.
4. Revert the line with `git checkout src/lib.rs`.

## Runs

- 2026-10-21: 1 attempt. Format 0.5, lint 28.9, test 166.6, image 65.6 seconds, all `status: passed`, web `status: cached`, total 262.5 seconds.
- The proof part passes: no Rust stage was cached. The time part fails: the total is above 40 seconds, because a Rust edit rebuilds 82 test binaries and macOS scans each new binary at its first start. The test of a failing stage is in `tests/pipeline.cache.rs` (`failed_stage_is_never_cached`), not run here against the real tree.

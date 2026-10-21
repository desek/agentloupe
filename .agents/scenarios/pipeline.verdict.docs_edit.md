# Scenario: a docs edit gets an instant verdict

- Goal: a change to documentation only gets a verdict fast.
- Surface: the command `.agents/scripts/pipeline.verdict.sh`.
- Success condition: exit code 0, every stage line has `status: cached`, total seconds at most 3.

## Steps

1. Run `.agents/scripts/pipeline.verdict.sh` until it prints `verdict: passed`.
2. Append one line to `docs/specification/operations.md`.
3. Run `.agents/scripts/pipeline.verdict.sh` again.
4. Revert the line with `git checkout docs/specification/operations.md`.

## Runs

- 2026-10-21: 3 attempts, 3 passed. Totals 1.0, 0.9, and 0.9 seconds, every stage `status: cached`, exit code 0.
- Outcome: reproduced (the behaviour is present).

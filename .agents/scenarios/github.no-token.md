---
date: 2026-10-20
source: cr (CR-0009)
surface: cli and http
outcome: reproduced
runs: "1 of 1 passed, last run 2026-10-20"
---

## Scenario: the product runs with no GitHub token

### Goal

The operator runs loupe with no GitHub token and gets day records with the four GitHub fields null.

### Preconditions

- The release build and the scratch dir `S` of `github.counts.md`. `S/data` holds the record of a day `D`.
- `LOUPE_DATA_KEY` and `LOUPE_ACCOUNT_KEY` are set. No `LOUPE_GITHUB_TOKEN_<LABEL>` variable is set: `env | grep -c LOUPE_GITHUB_TOKEN_` prints 0.

### Steps at the user surface

1. Run `loupe aggregate --date D`.
2. Start `loupe serve`. Run `curl -s http://127.0.0.1:8787/v1/days/D`. Stop `loupe serve`.

### Success condition

- The log of `loupe serve` has one line `github: disabled, no token`.
- Step 2 returns 200 with `prs_opened`, `prs_merged`, `lines_added`, and `lines_removed` null.

### Run log

2026-10-20, D 2026-10-14: step 1 printed `revision: 2`, `written: true`. Step 2 returned 200 with the four fields null. The log had `github: disabled, no token` at `src/serve/mod.rs:67`. Passed.

Note: when `loupe serve` starts with no token, the publisher builds again a day of the last 7 days with a failed label, so the record then has the four fields null. Start `loupe serve` with the same tokens as `loupe aggregate` if you want the counts.

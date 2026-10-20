---
date: 2026-10-20
source: cr (CR-0009)
surface: cli and http, with the real GitHub API
outcome: reproduced
runs: "1 of 1 passed, last run 2026-10-20"
---

## Scenario: the operator finds out that a token does not work

### Goal

The operator sees in the log which token label failed and how to fix it, and the public record shows only the number of failed tokens.

### Preconditions

- The release build and a new scratch dir `S` as in `github.counts.md`.
- `.env` is loaded, so `LOUPE_GITHUB_TOKEN_PERSONAL` works.
- `LOUPE_GITHUB_TOKEN_ACME` holds an invalid value, for example `ghp_invalidinvalidinvalidinvalid0000`. Never use a real token here.

### Steps at the user surface

1. Run `loupe aggregate --date D 2> S/agg.log`.
2. Run `grep '"WARN"' S/agg.log`.
3. Start `loupe serve` with the same variables. Run `curl -s http://127.0.0.1:8787/v1/days/D`. Stop `loupe serve`.
4. Run `grep -c ACME` on the record, and `grep -rl <invalid value> S`.

### Success condition

- Step 2 shows a warn line with `github_label` `ACME`, `cause` `unauthorized`, and the fix text.
- Step 3 shows `detail.github.failed_tokens` 1 and the counts of PERSONAL.
- Step 4 prints 0 and finds no file.

### Run log

2026-10-20, D 2026-10-14: the warn line at `src/github/fetch.rs:50` had `github_label` ACME, `cause` unauthorized, Fix 1, Fix 2, Reference, and Verify. The record had `failed_tokens` 1, `prs_opened` 8, `prs_merged` 6, `lines_added` 8676. ACME is not in the record. No file holds the invalid value. Passed.

---
date: 2026-10-20
source: cr (CR-0009)
surface: cli and http, with the real GitHub API
outcome: reproduced
runs: "2 of 2 passed, last run 2026-10-20"
---

## Scenario: the PR counts of a day match the counts of GitHub

### Goal

The operator sees the PR counts and the line counts of one day in the public API, and the counts match GitHub.

### Preconditions

- The release build exists: `cargo build --release --bin loupe`. Run every command from the repository root.
- `.env` holds `LOUPE_DATA_KEY`, `LOUPE_ACCOUNT_KEY`, and `LOUPE_GITHUB_TOKEN_PERSONAL`, a token of the user of the gh CLI.
- A scratch dir `S` with an empty `S/data` and `S/upstreams.toml` with the table `[anthropic]`, `base_url = "https://api.anthropic.com"`, `litellm_provider = "anthropic"`.
- A recent day `D` on which the user opened and merged pull requests. Port 8787 is free.

### Steps at the user surface

1. Run `set -a; . ./.env; set +a`. Set `LOUPE_DATA_DIR=S/data`, `LOUPE_UPSTREAMS_FILE=S/upstreams.toml`, `LOUPE_TZ=Europe/Stockholm`.
2. Run `loupe aggregate --date D`.
3. Start `loupe serve`. Run `curl -s http://127.0.0.1:8787/v1/days/D`. Stop `loupe serve`.
4. Run `gh api -X GET search/issues -f q="author:@me type:pr created:R" -q .total_count`, with `R` equal to `DT00:00:00+03:00..DT23:59:59+03:00`. Do it again with `merged:R`.
5. For each repository of the user in the last ten days, run `sqlite3 S/data/loupe.db .dump | grep -ci <owner/repo>`. Print only the sum.

### Success condition

- Step 3 returns `prs_opened` and `prs_merged` equal to the two counts of step 4, and `detail.github.failed_tokens` 0.
- The sum of step 5 is 0. No step prints a repository name, a number, a URL, or the token.

### Run log

2026-10-20, D 2026-10-18: record `prs_opened` 2, `prs_merged` 0, GitHub 2 and 0. Passed.

2026-10-20, D 2026-10-14: record `prs_opened` 8, `prs_merged` 6, `lines_added` 8676, `lines_removed` 2525, `failed_tokens` 0. GitHub 8 and 6. The sum of step 5 for 2 repositories is 0. A grep for the token value over `S` finds no file. Passed.

Note: `gh search prs --merged R` returns 0 when `R` has a time and an offset, so step 4 uses `gh api` with the same query that the worker sends. `gh search prs --merged 2026-10-14` returns 7, because it uses the UTC day.

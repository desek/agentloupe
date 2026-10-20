---
date: 2026-10-20
source: cr (CR-0008)
surface: browser
outcome: reproduced
runs: "2 of 2 passed, last run 2026-10-20"
---

## Scenario: a new install shows a clear empty state

### Goal

An operator with no published days understands from the page that no data exists yet and how to make it.

### Preconditions

- The release build exists: `cargo build --release --bin loupe`. Run every command from the repository root.
- `K` is the output of `target/release/loupe gen-key`. `S` is an empty scratch dir.
- agent-browser 0.27.1 and Google Chrome are installed.

### Steps at the user surface

1. Run `LOUPE_DATA_KEY=$K LOUPE_ACCOUNT_KEY=$K .agents/scripts/days.seed.sh --data-dir S/empty --days 0`.
2. Start `loupe serve` on `127.0.0.1:8790` with `LOUPE_DATA_DIR=S/empty`.
3. Run `agent-browser open http://127.0.0.1:8790/`. Read `document.body.innerText` and the count of `svg` elements.
4. For AC-8, seed S/nogh with `--github none`, serve it on port 8789, and read the text of the pull request and lines sections.

### Success condition

The page shows "No days published yet", the hint with `loupe aggregate --date today`, the header "No update yet", and zero `svg` elements. With null GitHub fields, both GitHub sections show "GitHub not connected" and no zero figure.

### Run log

- Run 1: text "No update yet", "No days published yet", "Days appear after the nightly publish, or after loupe aggregate --date today.", svg 0. Passed. Screenshot: `dashboard.empty.png`.
- Run 1, GitHub none: `["Pull requests\n\nGitHub not connected","Lines changed in the range\n\nGitHub not connected"]`. Passed.
- Run 2: the same. Passed.

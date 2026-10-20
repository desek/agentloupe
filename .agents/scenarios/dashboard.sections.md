---
date: 2026-10-20
source: cr (CR-0008)
surface: browser
outcome: reproduced
runs: "2 of 2 passed, last run 2026-10-20"
---

## Scenario: the sections show the seeded sums, and 6M changes the totals and the URL fragment

### Goal

The reader sees the agent work of a range in each section of the dashboard, and changes the range with the 6M button.

### Preconditions

- The release build exists: `cargo build --release --bin loupe`. Run every command from the repository root.
- `K` is the output of `target/release/loupe gen-key`. `S` is an empty scratch dir.
- agent-browser 0.27.1 and Google Chrome are installed.

### Steps at the user surface

1. Seed S/full as in `dashboard.headline.md`. Then make one record the test record: `sqlite3 S/full/loupe.db "UPDATE days SET record=json_set(record,'$.unpriced_calls',0); UPDATE days SET record=json_set(record,'$.unpriced_calls',2,'$.summary','<img src=x onerror=alert(1)>') WHERE day=(SELECT max(day) FROM days);"`.
2. Start `loupe serve` as in `dashboard.headline.md`, with `LOUPE_SUBSCRIPTION_MAX=100` also set.
3. Run `agent-browser open http://127.0.0.1:8788/`. Read `document.querySelector("main").innerText` with `agent-browser eval`.
4. Press 6M. Read `location.hash`, the main text, the class list of the heatmap `rect` elements, and the `/v1/days` entries of `performance.getEntriesByType("resource")`.
5. Press 6M again and read the resource entries again. Press 1Y and read the main text.
6. Compute the expected sums with `jq` from `curl -s 'http://127.0.0.1:8788/v1/days'`, and the subscription cost from `curl -s 'http://127.0.0.1:8788/v1/days?from=<first day>'`.

### Success condition

Each section shows the sums of step 6. After 6M, the hash is `#range=6M`, one new `/v1/days` request occurs, and the heatmap classes do not change. A second 6M press makes no request. The log shows the summary as literal text with no `img` element.

### Run log

- Expected (jq), 3M from 2026-07-22: tokens 52 530 660, input 49 715 595, output 2 815 065, cost 62.11, PRs 133 opened 90 merged, lines +41 085 -14 265, subscription 296.77. 6M from 2026-04-22: tokens 170 930 280, cost 152.03, PRs 270 and 180, lines +85 270 -28 780, subscription 594.5161. 1Y from 2025-10-21: tokens 362 891 112, cost 316.19, subscription 1200.
- Run 1: the page showed every value above, the header, the headline, pull requests, lines changed, tokens per day, cost with "List price of the tokens, not money paid." and "2 calls not priced", subscription cost, heatmap (365 cells: h0 1, h1 to h4 91 each), daily log, and the footer "Covers Claude Code and Pi. claude.ai chats are not covered." After 6M: hash `#range=6M`, one new request `?from=2026-04-22`, heatmap class string identical (cmp), no request on the second press. The log showed `<img src=x onerror=alert(1)>` as text, 0 img elements. No streak text. Passed. Screenshot: `dashboard.sections.6m.png`.
- Run 2 in a new browser session: the same values. Passed.
- Note: in run 1 the server was restarted with the subscription variable while the browser session stayed open, and the 3M card showed the cached old value "No subscription configured". The page caches one subscription value per range for the life of the page, so a reload after a server restart is part of step 3.

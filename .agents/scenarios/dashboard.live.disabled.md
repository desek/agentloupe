---
date: 2026-10-21
source: cr (CR-0013)
surface: browser and http
outcome: reproduced
runs: "1 of 1 passed, last run 2026-10-21"
---

## Scenario: run without a live feed

### Goal

The operator switches the live feed off. The dashboard then shows no live section.

### Preconditions

- The setup of `dashboard.live.follow.md`, with no `loupe serve` running.

### Steps at the user surface

1. Start `LOUPE_LIVE_ENABLED=false loupe serve`.
2. Run `curl -s -o /dev/null -w "%{http_code}" http://127.0.0.1:8787/live`.
3. Run `agent-browser open http://127.0.0.1:8787/`. Wait 20 s for 3 failed connects with backoff. Run `agent-browser snapshot`.

### Success condition

- Step 2 returns 404.
- The snapshot holds no region "Live", no "Live, today" heading, and no paused text.

### Run log

2026-10-21: `GET /live` returned 404. After 20 s the snapshot held 0 matches for the live region, the heading, and the paused text. Screenshot: `dashboard.live.disabled.png`.

---
date: 2026-10-21
source: cr (CR-0013)
surface: browser
outcome: reproduced
runs: "1 of 1 passed, last run 2026-10-21"
---

## Scenario: the view survives a restart of the binary

### Goal

The operator keeps the open dashboard when the binary restarts. The live section shows the paused state, then the figures return with no reload.

### Preconditions

- The setup and the state at the end of `dashboard.live.follow.md`: `loupe serve` runs and the page is open in agent-browser 0.27.1.

### Steps at the user surface

1. Stop `loupe serve` (`pkill -f "target/release/loupe serve"`). Wait 2 s. Run `agent-browser snapshot`.
2. Start `loupe serve` again with the same variables.
3. Run `agent-browser snapshot` once per second until the live figures show and the paused text is gone. Do not reload.

### Success condition

- After step 1 the live section shows "Live feed paused, reconnecting".
- The figures return within 35 s of the restart, with the values from the new snapshot.

### Run log

2026-10-21: after the stop, the section showed "Live feed paused, reconnecting" (`dashboard.live.reconnect.paused.png`). The figures returned 1 s after the restart, Calls 1 and Tokens "55.0k", with no reload (`dashboard.live.reconnect.png`).

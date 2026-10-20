---
date: 2026-10-20
source: cr (CR-0008)
surface: browser
outcome: reproduced
runs: "2 of 2 passed, last run 2026-10-20"
---

## Scenario: the page works with no internet

### Goal

A reader opens the dashboard with no network outside 127.0.0.1, and every font and script loads from the binary.

### Preconditions

- The release build exists: `cargo build --release --bin loupe`. Run every command from the repository root.
- `K` is the output of `target/release/loupe gen-key`. `S` is an empty scratch dir.
- agent-browser 0.27.1 and Google Chrome are installed.

### Steps at the user surface

1. Seed and serve S/full on port 8788 as in `dashboard.sections.md`.
2. Run `agent-browser open http://127.0.0.1:8788/`, then `agent-browser network requests`.
3. Run `agent-browser eval 'document.fonts.ready.then(()=>JSON.stringify([...document.fonts].map(f=>f.family+":"+f.status)))'`.
4. Run `.agents/scripts/web.render.sh --days 400` and read `remote_requests`.

### Success condition

Every request goes to `http://127.0.0.1:8788`, both fonts have the status `loaded`, and `remote_requests: 0`.

### Run log

- The network was not blocked at the operating system level. The request log of the browser is the evidence, as the CR permits.
- Run 1: 30 requests, all to `http://127.0.0.1:8788`. Fonts `Geist:loaded`, `Geist Mono:loaded`. Passed. Screenshot: `dashboard.offline.png`.
- Run 2: `web.render.sh --days 400` printed `remote_requests: 0 status: passed`. Passed.

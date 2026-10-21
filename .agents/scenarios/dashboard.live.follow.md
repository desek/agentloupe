---
date: 2026-10-21
source: cr (CR-0013)
surface: browser, with a real agent
outcome: reproduced
runs: "1 of 1 passed, last run 2026-10-21"
---

## Scenario: follow the calls and tokens live, under the CSP

### Goal

The operator sees the calls and tokens of today change on the open dashboard, with no reload, when an agent makes a call through the proxy. The socket opens under the CSP with no violation.

### Preconditions

- The release build exists: `cargo build --release --bin loupe`. Run from the repository root.
- A scratch dir `S` with an empty `S/data` and `S/upstreams.toml` as in `.agents/scenarios/cost.call.md`.
- `LOUPE_DATA_KEY` and `LOUPE_ACCOUNT_KEY` from `loupe gen-key`, a proxy token `T` from `loupe token create --name verify`. Do not print the token.
- `loupe serve` runs on `127.0.0.1:8787` with the default `LOUPE_PARSE_INTERVAL_SECS` of 5.
- agent-browser 0.27.1.

### Steps at the user surface

1. Run `agent-browser open http://127.0.0.1:8787/`. Run `agent-browser snapshot`, and read the region "Live".
2. Run `env -u CLAUDECODE ANTHROPIC_BASE_URL=http://127.0.0.1:8787/$T/claude-code/anthropic claude -p "say hi" --model claude-haiku-5-5`.
3. Wait 6.5 seconds (parse interval plus 1 s plus margin for the command exit). Run `agent-browser snapshot` again. Do not reload.
4. Run `agent-browser console` and `agent-browser errors`.
5. Run `sqlite3 S/data/loupe.db "select count(*), sum(input+output+cache_read+cache_write_5m+cache_write_1h) from calls"`.

### Success condition

- "Calls today" changes from 0 to 1, and "Tokens today" equals the stored sum (rounded display), with no reload.
- "in flight" shows 0.
- The console holds no CSP violation and no error.
- `curl -sI http://127.0.0.1:8787/` shows the CSP header of `docs/specification/dashboard.md` with `connect-src 'self'`.

### Run log

2026-10-21: before the call, Calls 0, Tokens 0. Within 6.5 s after the command exit and with no reload: "0 in flight", Calls 1, Tokens "55.0k", Open segments 1, cost "$0.01". The database holds 1 call with a token sum of 54974. The console and the error list were empty. The CSP header equals the specification value. Screenshot: `dashboard.live.follow.png`.

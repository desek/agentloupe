---
date: 2026-10-20
source: cr (CR-0007)
surface: cli and http, with a real agent
outcome: reproduced
runs: "1 of 1 passed, last run 2026-10-20"
---

## Scenario: a session of today is visible in the public API after loupe aggregate

### Goal

The operator sees the record of today in the public API after one real Claude Code call through the proxy.

### Preconditions

- The release build exists: `cargo build --release --bin loupe`. Run every command from the repository root.
- A scratch dir `S` with an empty `S/data` and `S/upstreams.toml` with the table `[anthropic]`, `base_url = "https://api.anthropic.com"`, `litellm_provider = "anthropic"`.
- Claude Code is logged in. Port 8787 is free.

### Steps at the user surface

1. Run `loupe gen-key` two times and put the values in `LOUPE_DATA_KEY` and `LOUPE_ACCOUNT_KEY`. Set `LOUPE_DATA_DIR=S/data` and `LOUPE_UPSTREAMS_FILE=S/upstreams.toml`.
2. Run `loupe token create --name verify` and put the `token` field in `T`.
3. Start `loupe serve` on `127.0.0.1:8787`.
4. Run `curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8787/v1/days/<today>`, with `<today>` the date in `Europe/Stockholm`.
5. Run `env -u CLAUDECODE ANTHROPIC_BASE_URL=http://127.0.0.1:8787/$T/claude-code/anthropic claude -p "say hi" --model claude-haiku-5-5`.
6. Run `loupe parse --once`, then `loupe aggregate --date today`.
7. Run `curl -s http://127.0.0.1:8787/v1/days/<today>`. Stop `loupe serve`.

### Success condition

- Step 4 returns `404`.
- Step 6 prints `written: true` and `calls: 1`.
- Step 7 returns `200` with `calls` 1 or more and `detail.by_agent` with the key `claude-code`.

### Run log

2026-10-20: step 4 returned 404. `loupe aggregate --date today` printed `calls: 1`, `sessions: 1`, `cost_usd: 0.0086`, `summary: absent`, `revision: 1`, `written: true`. Step 7 returned the record of 2026-10-20 with `calls` 1 and `by_agent` keys `["claude-code"]`.

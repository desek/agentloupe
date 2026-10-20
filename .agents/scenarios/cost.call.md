---
date: 2026-10-20
source: cr (CR-0005)
surface: cli and http, with a real agent
outcome: reproduced
runs: "1 of 1 passed, last run 2026-10-20"
---

## Scenario: a real Haiku call gets an API-equivalent cost

### Goal

The operator sees the API-equivalent cost of a call that Claude Code made through the proxy.

### Preconditions

- The release build exists: `cargo build --release --bin loupe`. Run every command from the repository root, because loupe reads `config/`.
- A scratch dir `S` with an empty `S/data` and `S/upstreams.toml` with the table `[anthropic]`, `base_url = "https://api.anthropic.com"`, `litellm_provider = "anthropic"`.
- Claude Code is logged in. Do not change `~/.claude/settings.json`.

### Steps at the user surface

1. Run `loupe gen-key` two times and put the values in `LOUPE_DATA_KEY` and `LOUPE_ACCOUNT_KEY`.
2. Run `LOUPE_DATA_DIR=S/data loupe token create --name verify` and put the `token` field in `T`.
3. Start `loupe serve` on `127.0.0.1:8787` with `LOUPE_DATA_DIR=S/data` and `LOUPE_UPSTREAMS_FILE=S/upstreams.toml`.
4. Run `env -u CLAUDECODE ANTHROPIC_BASE_URL=http://127.0.0.1:8787/$T/claude-code/anthropic claude -p "say hi" --model claude-haiku-5-5`.
5. Run `loupe parse --once` with the same variables. Stop `loupe serve`.
6. Run `sqlite3 -line S/data/loupe.db 'select model, service_tier, input, cache_read, cache_write_5m, cache_write_1h, output, cost_usd, price_commit, unpriced_reason from calls'`.
7. Run `loupe price show --model claude-haiku-5-5 --upstream anthropic`.

### Success condition

- One call row with model `claude-haiku-5-5`, a non-null `cost_usd` above 0, a null `unpriced_reason`, and the `price_commit` of `vendor/litellm/SOURCE.md`.
- `cost_usd` equals the sum of each token count times its price from step 7.
- No body, header value, or credential is in the output.

### Run log

2026-10-20: 1 call row. Service tier `standard`; input 2, cache_read 12778, cache_write_5m 0, cache_write_1h 41195, output 66. `cost_usd` 0.00839998, `price_commit` cfee0ffe4186e2ac36cfaee4c4e10f6c3daafd2f, `unpriced_reason` null. From `price show`: 2 x 0.0000001 + 12778 x 0.00000001 + 41195 x 0.0000002 + 66 x 0.0000005 = 0.00839998. The prompt is below 100k tokens, so the base prices apply.

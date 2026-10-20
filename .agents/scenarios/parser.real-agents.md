---
date: 2026-10-20
source: cr (CR-0004)
surface: cli and http, with real agents
outcome: reproduced
runs: "1 of 1 passed before the header list change, which showed the subagent join; 1 of 1 passed after it; last run 2026-10-20"
---

## Scenario: real agent calls become call rows and sessions

### Goal

The user sends real requests from Claude Code and Pi through the proxy with a token. The parser makes call rows with token counts, and a Claude Code subagent is its own session.

### Preconditions

- The release build exists: `cargo build --release --bin loupe --example capture_shape`.
- A scratch dir `S` with an empty `S/data` and `S/upstreams.toml` that has the line `anthropic = "https://api.anthropic.com"`.
- Claude Code 2.1.304 is logged in. Pi 1.3.0 and the extension `pi-cc-auth` are installed.
- Do not change `~/.claude/settings.json`. Do not read, copy, or link a file under `~/.pi/agent`.

### Steps at the user surface

1. Run `loupe gen-key` and put the value in `K`.
2. Run `LOUPE_DATA_DIR=S/data loupe token create --name verify` and put the `token` field in `T`.
3. Start `loupe serve` on `127.0.0.1:8787` with `LOUPE_DATA_DIR=S/data`, `LOUPE_DATA_KEY=K`, `LOUPE_ACCOUNT_KEY=K`, `LOUPE_UPSTREAMS_FILE=S/upstreams.toml`, `LOUPE_IDLE_SECS=60`.
4. Run `env -u CLAUDECODE ANTHROPIC_BASE_URL=http://127.0.0.1:8787/$T/claude-code/anthropic claude -p "say hi" --model claude-haiku-5-5`.
5. Run the same command with the prompt "Use the Agent tool to start one subagent. Ask the subagent to reply with the single word ready. Then tell me what it said."
6. Write `S/pi/models.json` as `{"providers":{"anthropic":{"baseUrl":"http://127.0.0.1:8787/<T>/pi/anthropic"}}}`.
7. Run `PI_CODING_AGENT_DIR=S/pi pi -p --no-extensions --no-mcp --no-skills --no-session --no-context-files -e <pi-cc-auth>/src/index.ts --provider anthropic --model claude-haiku-5-5 "say hi"`.
8. Run `loupe parse --once` with the same variables.
9. Run `LOUPE_DATA_KEY=K target/release/examples/capture_shape S/data`. It prints names, paths, and booleans only.
10. Wait until no row of `sessions` has the state `open`, then stop `loupe serve`.

### Success condition

- `calls` has 1 or more rows for `claude-code` and for `pi`, each with model `claude-haiku-5-5` and `input + cache_read + output > 0`.
- The subagent call of step 5 has `session_hint_source` `x-claude-code-agent-id` and a session row that differs from the main calls of step 5.
- Every session row has the state `closed` about 60 seconds after the last call.
- No header value, body, or credential is in the output or in a file of the repository.

### Result on 2026-10-20

- Claude Code: 4 call rows; the subagent call is in its own session; account source `metadata.user_id`.
- Pi: 1 call row; key source `opening`; account source `credential:authorization`.
- Every segment closed by the idle step after 60 seconds.
- The first run, with `x-claude-code-session-id` first in the list, put the subagent call in the session of its parent. The list now starts with `x-claude-code-agent-id`.

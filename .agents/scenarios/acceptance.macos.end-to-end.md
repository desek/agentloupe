---
date: 2026-10-21
source: cr (CR-0010)
surface: cli, http, and browser, with the real agents and the real Anthropic API
outcome: pending
runs: "0 of 0, no run yet"
---

## Scenario: the work of Claude Code and Pi of today is on the dashboard

### Goal

The human sees the work of today of Claude Code and Pi on the dashboard, after one real call of each agent through the proxy in the container on the Mac.

### Preconditions

- Docker Desktop runs. Port 8787 is free. Claude Code (record the version) and Pi 1.3.0 are installed, and Claude Code is logged in.
- A scratch copy `E` of `.env` outside the repository, without the `LOUPE_GITHUB_TOKEN_` lines. It holds `LOUPE_DATA_KEY`, `LOUPE_ACCOUNT_KEY`, and `LOUPE_SUMMARY_OAUTH_TOKEN`. It does not set `LOUPE_SUMMARY_MODEL`, `LOUPE_SUMMARY_API_KEY`, `LOUPE_SITE_TITLE`, or `LOUPE_RAW_RETENTION_HOURS`.
- An untracked `compose.override.yaml`: `services.loupe.env_file: !override ["E"]` and `services.loupe.environment.LOUPE_IDLE_SECS: "60"`.
- Each run uses the compose project name `loupe-acceptance` and starts with a new volume, so the volume of the human is not touched.
- `D` is `TZ=Europe/Stockholm date +%F`. `T` is a file in the scratchpad with mode 0600.
- The Pi credential route: the extension pi-cc-auth (`-e <path>/pi-cc-auth/src/index.ts`) reads the Claude Code login from the keychain. No `ANTHROPIC_OAUTH_TOKEN` exists on this Mac.

### Steps at the user surface

1. Record the checksum of `~/.claude/settings.json` and of every file under `~/.pi/agent`.
2. `docker compose -p loupe-acceptance up -d --build`. Wait for `healthy`. Run `docker compose -p loupe-acceptance exec loupe printenv LOUPE_IDLE_SECS` and `curl -s http://127.0.0.1:8787/healthz`.
3. `docker compose -p loupe-acceptance exec -T loupe loupe token create --name acceptance`, and save the `token` field in `T`. Run `loupe token list`.
4. `eval "$(.agents/scripts/agent.connect.sh --agent claude-code --token-file T --check)"`, then `env -u CLAUDECODE claude -p "Reply with the word ok." --model claude-haiku-5-5`.
5. `eval "$(.agents/scripts/agent.connect.sh --agent pi --token-file T)"`, then `pi --list-models claude-haiku-5-5` with the extension, then `pi -p --no-extensions --no-mcp --no-skills --no-session --no-context-files -e <pi-cc-auth> --provider anthropic --model claude-haiku-5-5 "Reply with the word ok."`. Record `date +%s` as `t0`.
6. Wait 70 seconds. Run `loupe parse --once`, `loupe summarize --once`, and `loupe aggregate --date today` in the container. Poll `curl -s http://127.0.0.1:8787/v1/days/D` until `detail.by_agent` holds both keys. Record `t1 - t0`.
7. Open `http://127.0.0.1:8787/` in a real browser with agent-browser. Read the tab title, the daily log row for D, and the footer. Take a screenshot.
8. Run the secret leak check over the scenario files and `docker compose -p loupe-acceptance logs`.
9. Compare the checksums of step 1. Run `docker compose -p loupe-acceptance down -v`.

### Success condition

- Step 2: `60`, `healthy`, and `{"status":"ok"}`.
- Step 3: the list shows `acceptance` and no token value.
- Steps 4 and 5: each command exits 0 and prints an answer. `--list-models` lists `claude-haiku-5-5` under `anthropic`.
- Step 6: `/v1/days/D` returns 200. `detail.by_agent` has `claude-code` and `pi`, each with `calls` 1 or more. `cost_usd` is more than 0, `unpriced_calls` is 0, the four PR fields are null, and `summary` is one paragraph of 3 to 8 sentences. `t1 - t0` is at most 300 seconds.
- Step 7: the title is `AI agent activity`. The daily log has a row for D with tokens more than 0. The footer names Claude Code and Pi. The page does not show `Agentloupe`. The page shows `GitHub not connected`.
- Step 8: the count is 0.
- Step 9: every checksum is the same.

### Run log


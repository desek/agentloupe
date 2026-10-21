---
date: 2026-10-21
source: cr (CR-0010)
surface: cli, http, and browser, with the real agents and the real Anthropic API
outcome: passed
runs: "4 of 4 passed (4 attempts), last run 2026-10-21"
---

## Scenario: the work of Claude Code and Pi of today is on the dashboard

### Goal

The human sees the work of today of Claude Code and Pi on the dashboard, after one real call of each agent through the proxy in the container on the Mac.

### Preconditions

- Docker Desktop runs. Port 8787 is free. Claude Code (record the version) and Pi 1.3.0 are installed, and Claude Code is logged in.
- A scratch copy `E` of `.env` outside the repository, without the `LOUPE_GITHUB_TOKEN_` lines. It holds `LOUPE_DATA_KEY`, `LOUPE_ACCOUNT_KEY`, and `CLAUDE_CODE_OAUTH_TOKEN`. It does not set `LOUPE_SUMMARY_MODEL`, `ANTHROPIC_API_KEY`, `LOUPE_SITE_TITLE`, or `LOUPE_RAW_RETENTION_HOURS`.
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

A first attempt on 2026-10-21 stopped at step 5: `pi -p` waited for stdin, and the extension route gave no model list. That was a defect of the run steps, not of the product, so the steps changed before the counted runs: each agent command gets `</dev/null`. The model check of step 5 runs with `ANTHROPIC_OAUTH_TOKEN=placeholder pi --no-extensions --list-models claude-haiku-5-5`, because Pi lists a model only when a credential variable exists, and the extension gives the credential only at session start. The placeholder sends no request. The route is the built-in model of Pi: the output lists `claude-haiku-5-5` under `anthropic`, so the fallback `models` entry was not necessary.

Versions: Claude Code 2.1.306, Pi 1.3.0. The Pi credential came from the extension pi-cc-auth.

| Run | Steps 2 and 3 | claude -p | pi -p | by_agent calls | cost_usd | unpriced | PR fields | summary sentences | seconds to record | browser | leaks | checksums |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 60, healthy, ok; list without a value | exit 0, ok | exit 0, ok | claude-code 1, pi 1 | 0.0006 | 0 | null | 5 | 74 | title AI agent activity, row 21 Oct 2026 with 57 754 tokens, footer names Claude Code and Pi, no Agentloupe, GitHub not connected | 0 | same |
| 2 | same | exit 0, ok | exit 0, ok | claude-code 1, pi 1 | 0.0006 | 0 | null | 5 | 73 | same checks passed | 0 | same |
| 3 | same | exit 0, ok | exit 0, ok | claude-code 1, pi 1 | 0.0006 | 0 | null | 5 | 73 | same checks passed | 0 | same |
| 4 (independent check) | 60, healthy, ok; list without a value | exit 0, ok | exit 0, ok | claude-code 1, pi 1 | 0.0089 | 0 | null (all four) | 4 | 73 | title AI agent activity, row 21 Oct 2026 with 57 767 tokens, footer names Claude Code and Pi, no Agentloupe, GitHub not connected | 0 | settings.json same; 0 files under ~/.pi/agent newer than the start marker |

Result: 3 of 3 runs passed. Evidence: `acceptance.macos.end-to-end.run1.log`, `.run2.log`, `.run3.log` (command output, tokens masked), `acceptance.macos.end-to-end.dashboard.png` (run 1), and `acceptance.macos.end-to-end.day.json` (`/v1/days/2026-10-21` of run 3). The secret leak grep over these files and over `docker compose logs` of each run counted 0. Each run used a new volume of the project `loupe-acceptance`, removed with `docker compose -p loupe-acceptance down -v`.

Observation: the daily log row shows `0 PRs` while the four PR fields are null. The success condition does not grade it.

Run 4, the independent check (2026-10-21, a separate verifier agent): the identical steps passed, so 4 of 4 runs passed. Two changes of method: step 1 and step 9 for `~/.pi/agent` use a start marker file and `find ~/.pi/agent -type f -newer <marker>`, because the directory holds 68 072 files and the checksum of each file took more than 10 minutes. The `agent-browser-specialization` skill is not installed, so the verifier used `agent-browser` 0.27.1 directly. Evidence: `acceptance.macos.end-to-end.run4.log` (tokens masked). The run also found that the README section `Connect the agents` did not name `ANTHROPIC_BASE_URL` or `PI_CODING_AGENT_DIR`, and the README was fixed.

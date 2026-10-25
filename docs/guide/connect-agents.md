<!-- @agents-index User guide: Connect Claude Code and Pi to the loupe proxy. -->

# Connect the agents

## Connect the agents

The connect script `.agents/scripts/agent.connect.sh` points one agent at the proxy for one
shell. It does not change `~/.claude`, `~/.pi/agent`, or a file in the repository. It takes the
proxy token from a file (`--token-file`) or from `LOUPE_PROXY_TOKEN`, never as an argument value.

1. Put `LOUPE_DATA_KEY`, `LOUPE_ACCOUNT_KEY`, and `CLAUDE_CODE_OAUTH_TOKEN` in `.env`. The
   summarizer uses the subscription token from `claude setup-token` and the default model
   `claude-haiku-5-5`. Do not set `LOUPE_SUMMARY_MODEL` or `ANTHROPIC_API_KEY`.
2. Start the container: `docker compose up -d --build`.
3. Make a proxy token into a file outside the repository:
   `(umask 077; docker compose exec -T loupe loupe token create --name laptop | sed -E 's/.*"token":"([^"]+)".*/\1/' > ~/.config/agentloupe-token)`.
   The file holds only the token value. `docker compose exec loupe loupe token list` shows the name.
4. Connect Claude Code in the shell that runs it:
   `eval "$(.agents/scripts/agent.connect.sh --agent claude-code --token-file ~/.config/agentloupe-token --check)"`.
   The script prints `export ANTHROPIC_BASE_URL=...`, which points Claude Code at the proxy.
5. Run Claude Code: `claude -p "Reply with the word ok." --model claude-haiku-5-5`.
6. Connect Pi: `eval "$(.agents/scripts/agent.connect.sh --agent pi --token-file ~/.config/agentloupe-token)"`.
   The script writes only `models.json` into the isolated agent directory
   `${XDG_STATE_HOME:-$HOME/.local/state}/agentloupe/pi-agent`. That directory holds no credential.
   The script prints `export PI_CODING_AGENT_DIR=...`, which points Pi at that directory.
7. Give Pi its credential from the environment. Use one of the two routes:
   - The plain route: run `claude setup-token` one time. Then export `ANTHROPIC_OAUTH_TOKEN`
     with that token in the shell that runs `pi -p`.
   - The alternative route: load a Pi extension that supplies the credential, for example an
     extension that reads the Claude Code login from the keychain (`pi -e <extension>`).
8. Check the model, then run Pi:
   `pi --list-models claude-haiku-5-5`, then
   `pi -p --provider anthropic --model claude-haiku-5-5 "Reply with the word ok."`.
9. Open the dashboard at `http://127.0.0.1:8787/`.

The nightly publish writes the day record. To see a day before the nightly publish, run
`docker compose exec loupe loupe aggregate --date YYYY-MM-DD`. A session closes after
`LOUPE_IDLE_SECS` with no calls, by default `3600` seconds. A short value in an untracked
`compose.override.yaml` (`services.loupe.environment.LOUPE_IDLE_SECS: "60"`) closes the sessions
sooner.

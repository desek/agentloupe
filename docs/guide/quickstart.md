<!-- @agents-index User guide: Full quickstart: configure, start the container, make a token, connect the agents, and open the dashboard. -->

# Quickstart

## Quickstart

Everything runs in one container on this machine. The proxy listens on `127.0.0.1:8787`, and the
agents reach it through a base URL that carries a proxy token, the agent label, and the upstream,
as `http://127.0.0.1:8787/<token>/<agent>/<upstream>`. A bare `ANTHROPIC_BASE_URL=http://localhost:8787`
does not work, because the root of the proxy serves the public API and the dashboard.

1. Make the configuration. Copy `.env.example` to `.env` and fill in:
   - `LOUPE_DATA_KEY` and `LOUPE_ACCOUNT_KEY`, required, each from `loupe gen-key` (or
     `docker compose run --rm loupe loupe gen-key` before the first start). They encrypt the
     captured bodies and hash the accounts; keep them, a new key makes the old data unreadable.
   - `CLAUDE_CODE_OAUTH_TOKEN` from `claude setup-token`, for the summaries on `claude-haiku-5-5`.
     Without it the days get no summary paragraph. Do not set `ANTHROPIC_API_KEY` for that.
   - `LOUPE_TZ`, the time zone that cuts the days, for example `Europe/Stockholm`. Default `UTC`.
   - `LOUPE_SUBSCRIPTION_<NAME>=<USD per month>` for each subscription you pay, optional.
   - `LOUPE_GITHUB_TOKEN_<NAME>` for the pull request counts, optional. See [GitHub](github.md#github).
2. Start the container: `docker compose up -d --build`. Check `curl -s http://127.0.0.1:8787/healthz`.
3. Make a proxy token into a file outside the repository, so it never lands in a shell history:
   `(umask 077; docker compose exec -T loupe loupe token create --name laptop | sed -E 's/.*"token":"([^"]+)".*/\1/' > ~/.config/agentloupe-token)`.
4. Connect Claude Code in the shell that runs it, then run it as usual:
   `eval "$(.agents/scripts/agent.connect.sh --agent claude-code --token-file ~/.config/agentloupe-token --check)"`.
   The script exports `ANTHROPIC_BASE_URL` with the full path and changes nothing under `~/.claude`.
5. Connect Pi the same way, with `--agent pi`. The script exports `PI_CODING_AGENT_DIR` with an
   isolated directory that holds only `models.json`; Pi takes its credential from
   `ANTHROPIC_OAUTH_TOKEN` or from a credential extension. See [Connect the agents](connect-agents.md#connect-the-agents).
6. Open the dashboard at `http://127.0.0.1:8787/`. The live card moves as the agents work, and the
   day records appear after the nightly publish, or at once with
   `docker compose exec loupe loupe aggregate --date today`.
7. To load the sessions that predate the proxy, mount the session logs read-only in an untracked
   `compose.override.yaml` and run `docker compose exec loupe loupe backfill --source claude-code`
   and `--source pi`. See [The `loupe` binary](loupe-binary.md#the-loupe-binary).

A release asset or the registry image replaces step 2 for an install without the source. See
[Install](install.md#install).

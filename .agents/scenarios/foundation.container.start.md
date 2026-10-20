---
date: 2026-10-20
source: cr (CR-0001)
surface: cli
outcome: reproduced
runs: "3 of 3 passed, last run 2026-10-20"
---

## Goal

The user runs Agentloupe locally in a container on macOS and sees that it is healthy.

## Preconditions

- Docker with Compose v2 or later runs on the host.
- `.env` holds `LOUPE_DATA_KEY` and `LOUPE_ACCOUNT_KEY`. Each value comes from `loupe gen-key`.
- No other process uses port 8787 on 127.0.0.1.

## Steps at the user surface

Driver: the shell, with the commands that the user types.

1. Run `docker compose up -d --build`.
2. Run `docker compose ps` until the status shows `healthy`.
3. Run `curl -s http://127.0.0.1:8787/healthz`.
4. Run `docker compose port loupe 8787`.
5. Run `docker compose restart`, then `docker compose logs loupe`.
6. Run `docker compose down`.

## Success condition

- `docker compose ps` shows `(healthy)` within 5 seconds of the start.
- `curl` returns `{"status":"ok"}`.
- `docker compose port loupe 8787` returns `127.0.0.1:8787`.
- After the restart, the log line `database open and migrated` shows `"applied":0`.

## Run log

- Run 1: healthy after about 5 seconds, on the threshold. The cause was the first HEALTHCHECK at `--interval=10s` after the start period. The Dockerfile got `--start-interval=1s`.
- Run 2: healthy after 1.3 seconds. `/healthz` returned `{"status":"ok"}`. The port was `127.0.0.1:8787`. The restart log showed `"applied":0`.
- Run 3: `/healthz` answered `{"status":"ok"}` 0.02 seconds after `docker compose up -d` returned.

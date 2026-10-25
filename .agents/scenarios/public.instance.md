---
date: 2026-10-25
source: cr (CR-0028)
surface: container (docker compose), http (curl), live socket (node WebSocket), browser (chrome-devtools-mcp), with a real agent
outcome: reproduced
runs: "2 of 2 product runs passed, last run 2026-10-25; one more scripted run failed on a bug of the script (a line break in the file check), fixed, then passed"
---

## Scenario: a public instance that has no access to non-public data

### Goal

A reader on the internet sees the dashboard, the public API, and the live card on a public port. The process behind that port has no access to the raw requests, the session data, or any secret. The surface is the container setup of `compose.yaml`, driven as an operator drives it.

### Preconditions

- Docker Desktop with Compose 2.24 or later (the `!override` tag), Node 22 or later, `sqlite3`, `openssl`, and `claude` on the path for the real call.
- The scenario runs as the separate compose project `loupe-scn`, on `127.0.0.1:18787` (role `full`) and `127.0.0.1:18788` (role `public`), with the image tag `agent-loupe:scn`, fresh volumes, and generated keys. The `loupe` service of `compose.yaml` and its data stay untouched.

### Steps at the user surface

Run `.agents/scripts/public.scenario.sh --dir <scratch dir> --agent-call`. The script does these steps and prints one line per check:

1. Build the image and `docker compose up -d` the two services. Wait until both are healthy.
2. In `loupe-public`, list `/data` and `/data/public`, and the names in `env`.
3. Touch a file under `/data/public` in `loupe-public`.
4. Copy the mirror out and list its tables.
5. Compare `GET /healthz` on the two ports. Read the first log line of `loupe-public`.
6. Create a proxy token in `loupe`. Open one live socket on each port. Run `claude -p "say hi" --model claude-haiku-5-5` through `http://127.0.0.1:18787/<token>/claude-code/anthropic`. Measure the time from `call.stored` on 18787 to `calls.batch` on 18788.
7. Run `loupe aggregate --date today` in `loupe`. Poll `GET /v1/days/<today>` on 18788 until the day is there. Compare the body with 18787.
8. Send the 404 list (15 GET and 6 POST paths, with a fake `lpt_` segment and the real token) to 18788.
9. Stop `loupe`. Read `/`, `/v1/days`, and `/healthz` on 18788, and compare the mirror bytes before and after.
10. Run `loupe-public` once with `LOUPE_DATA_KEY` set.

By hand, also open `http://127.0.0.1:18788/` in the browser driver and read the live card, the title, and the console (run 1).

### Success condition

- `/data` of `loupe-public` holds only `logs` and `public`; `/data/public` holds only `public.db`; `env` holds no secret name; a write under `/data/public` fails with a read-only file system error; the mirror holds only the tables `days` and `live_state`.
- The first log line of `loupe-public` holds `"role":"public"`, and `/healthz` gives the same body on both ports.
- The live delay is at most 2 100 ms, and the day reaches 18788 within 5 000 ms with the same body as on 18787.
- Each entry of the 404 list answers 404 with `not_found`, and no body holds `lpt_`.
- With `loupe` stopped, 18788 answers 200 on `/`, `/v1/days`, and `/healthz`, and the mirror bytes do not change.
- With `LOUPE_DATA_KEY` set, `loupe-public` exits with 2, the output names `LOUPE_DATA_KEY`, and the value is not in the output.

### Run log

- Run 1, 2026-10-25 19:52 CET, by hand. Both healthy 3.6 s after `up`. `/data`: `logs public`; `/data/public`: `public.db`, owner uid 10001 on a new volume; `env`: `HOME HOSTNAME LOUPE_ENV LOUPE_ROLE LOUPE_SITE_TITLE LOUPE_TZ PATH PWD SHLVL`. `touch` refused: "Read-only file system". Mirror: tables `days` and `live_state`, `user_version` 1, journal mode `delete`. `config show` in `loupe-public`: `LOUPE_ROLE` public from env, each secret `unset`. Live delay 481 ms; the in-flight count reached 18788 as `exchanges.batch` with `in_flight` 1, then 0. Day on 18788 4.38 s after `loupe aggregate`, the same body as 18787. 404 list: 21 entries, 0 failures; the real token path answered `{"error":"not_found","path":"/claude-code/anthropic/v1/messages"}`, and the full side held only the 2 exchanges of the real call. Browser: the page on 18788 shows the title Scenario, the live card with 1 call and 55.1k tokens, and the daily log; no console message (`public.instance.png`). With `loupe` stopped: 200 on each path, mirror bytes the same. Guard: exit 2, `LOUPE_DATA_KEY` named, the value 0 times in the output.
- Run 2, 2026-10-25 19:57 CET, scripted: 13 of 14 checks passed. The check `public_files` failed on a bug of the script: `echo "|"` put a line break into the compared text. The files were correct. The script now uses `printf`.
- Run 3, 2026-10-25 19:57 CET, scripted: 14 of 14 checks passed. Live delay 578 ms, mirror lag 3 390 ms.
- Extra run, 2026-10-25, scripted with `--keep`: 11 of 11 checks passed (no agent call). Then 100 API requests and 10 live sockets for 62 seconds on 18788 with `loupe` stopped: the mirror bytes were the same before and after.

### Measurements of the same runs

- Memory (`docker stats`, 60 seconds after start, the same data): full 17.66 MiB, public 14.93 MiB; public 21.03 MiB with 32 live sockets.
- Start time, process start to the line `listening`, median of 5: public 2.0 ms, full 7.0 ms. With `docker start` to the first 200 of `/healthz`: public 187.8 ms, full 168.3 ms; Docker dominates this figure.

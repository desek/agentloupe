---
date: 2026-10-20
source: cr (CR-0001)
surface: cli
outcome: reproduced
runs: "2 of 2 passed, last run 2026-10-20"
---

## Goal

The user starts the container without `LOUPE_DATA_KEY` and learns from the output what to fix.

## Preconditions

- The image `agent-loupe:local` exists. Build it with `docker compose build`.
- `LOUPE_DATA_KEY` is not set.

## Steps at the user surface

Driver: the shell.

1. Run `docker run --rm -e LOUPE_ACCOUNT_KEY=x agent-loupe:local serve`.
2. Run `echo $?`.

## Success condition

- The exit code is not 0.
- The output names `LOUPE_DATA_KEY` and `loupe gen-key`.

## Run log

- Run 1 (container): exit code 2. The output named `LOUPE_DATA_KEY`, `loupe gen-key`, `.env.example`, and `loupe config show`.
- Run 2 (host binary, `env -u LOUPE_DATA_KEY target/release/loupe serve`): exit code 2, with the same four names on stderr.

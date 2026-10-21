---
date: 2026-10-21
source: cr (CR-0015)
surface: cli (loupe config show, loupe serve) and http (GET /private/api/providers, GET /private/api/config)
outcome: reproduced
runs: "scenario 1: 2 of 2 passed; scenario 2: 3 of 3 passed; scenario 3: 3 of 3 passed; scenario 4: 3 of 3 passed against the real API; scenario 5: 3 of 3 passed; last run 2026-10-21"
---

## Scenario 1: detect the providers

### Goal

The operator sees which providers are available.

### Steps at the user surface

1. Run `env -i PATH=$PATH ANTHROPIC_API_KEY=testA-Zk2 OPENAI_API_KEY=testO-Wm7 loupe config show --human`.

### Success condition

- Two lines `provider.anthropic` and `provider.openai`, each with `credential=api_key`.
- The output holds neither test value.

## Scenario 2: a bad credential shows unauthorized

### Steps at the user surface

1. Start a fake provider that returns `401`.
2. Start `loupe serve` with `LOUPE_ADMIN_TOKEN`, `ANTHROPIC_API_KEY`, and `ANTHROPIC_BASE_URL` set to the fake.
3. Read `GET /private/api/providers` with the admin token.

### Success condition

- `anthropic` is `unauthorized` and `openai` is `not_configured`.
- `GET /healthz` returns `200` and `{"status":"ok"}`.

The test `healthz_status_independent_of_providers` (`tests/healthz.rs`) runs these steps.

## Scenario 3: a legacy name stops the start

### Steps at the user surface

1. Run `env -i PATH=$PATH LOUPE_SUMMARY_OAUTH_TOKEN=legacyV9 loupe serve`.

### Success condition

- The exit code is `2`.
- The error names `CLAUDE_CODE_OAUTH_TOKEN` and `.env`, and does not hold `legacyV9`.

## Scenario 4: the OAuth token on the models path

### Preconditions

- `.env` at the repository root holds a valid `CLAUDE_CODE_OAUTH_TOKEN`. Do not print it. Do not pass the other lines of `.env`, because `.env` can still hold a removed name.

### Steps at the user surface

1. Start `loupe serve` with only `CLAUDE_CODE_OAUTH_TOKEN` from `.env`, the default `ANTHROPIC_BASE_URL`, and an admin token.
2. Read `GET /private/api/providers` with the admin token after the first ping.

### Success condition

- `anthropic` is `ok`, so the real `GET https://api.anthropic.com/v1/models` returned a `2xx` with the OAuth ping headers.
- The log of the server does not hold the token.

### Result on 2026-10-21

- 3 of 3 runs: `anthropic` `ok`, `openai` `not_configured`, the start line `providers detected` with `detected` `anthropic` and `summary_provider` `anthropic`, no token in the log.

## Scenario 5: the private configuration

### Steps at the user surface

1. Start `loupe serve` with `LOUPE_ADMIN_TOKEN`, the three provider credentials, and a proxy token as `LOUPE_BOOTSTRAP_TOKEN`.
2. Read `GET /private/api/config` with the admin token, and run `loupe config show` with the same environment.

### Success condition

- The body has the same entries as `loupe config show`.
- `ANTHROPIC_API_KEY` is `set`, and the body holds no part of a test credential, the admin token, or the proxy token.

The tests `private_config_route_shape_and_auth` and `private_config_never_holds_a_credential` (`tests/private_config.rs`) run these steps.

---
date: 2026-10-20
source: cr (CR-0007)
surface: http
outcome: reproduced
runs: "2 of 2 passed, last run 2026-10-20"
---

## Scenario: the public API refuses bad input with an open CORS header

### Goal

A site on another origin reads `/v1/days` and gets a clear error for a bad query.

### Preconditions

- The release build exists. `loupe serve` runs on `127.0.0.1:8787` with a data dir.

### Steps at the user surface

1. Run `curl -si 'http://127.0.0.1:8787/v1/days?from=x'`.
2. Run `curl -si -X OPTIONS http://127.0.0.1:8787/v1/days`.
3. Run `curl -si -X POST http://127.0.0.1:8787/v1/days`.

### Success condition

- Step 1 returns `400`, `access-control-allow-origin: *`, `cache-control: no-store`, and the code `invalid_date` with a `fix` that names a valid query.
- Step 2 returns `204` with `access-control-allow-methods: GET, OPTIONS`.
- Step 3 returns `405` with `allow: GET, OPTIONS`.

### Run log

2026-10-20: step 1 returned 400 with `access-control-allow-origin: *` and `{"error":{"code":"invalid_date",...}}`. Step 2 returned 204 with the two headers. Step 3 returned 405 with `allow: GET, OPTIONS`.

2026-10-20, in the container: `curl -si -X OPTIONS http://127.0.0.1:8787/v1/days` returned `204` with `access-control-allow-origin: *` and `access-control-allow-methods: GET, OPTIONS`. `GET /v1/days?from=2026-10-18&to=2026-10-01` returned `400` with `error.code` `inverted_range` and an `error.fix` with the example `GET /v1/days?from=2026-10-01&to=2026-10-31`.

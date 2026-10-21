---
date: 2026-10-21
source: cr (CR-0011)
surface: browser
outcome: reproduced
runs: "1 of 1 passed, last run 2026-10-21"
---

## Scenario: see the tokens and the cost of one proxy token for 3M

### Goal

The operator sees the tokens and the API-equivalent cost of one proxy token for the range 3M on the private page.

### Preconditions

- The release build exists: `cargo build --release --bin loupe`.
- `K` is the output of `loupe gen-key`. `A` is the output of `loupe token generate`. `S` is an empty scratch dir.
- agent-browser 0.27.1 and Google Chrome are installed.

### Steps at the user surface

1. Migrate S with `loupe aggregate --date 2001-01-01`, with `LOUPE_TZ=UTC`.
2. Seed S with the SQL of `.agents/scripts/private.bench.sh`, with 3000 calls, 300 sessions, and 120 days.
3. Start `loupe serve` on `127.0.0.1:8791` with `LOUPE_ADMIN_TOKEN=A`.
4. Run `agent-browser open http://127.0.0.1:8791/private/`. The page goes to the login form.
5. Fill `#token` with A and click the submit button.
6. Read the text of the rows of `#groups`.
7. Get `/private/api/v1/report?group=token&from=<today minus 90>` with the bearer header.

### Success condition

The table row of `laptop` shows the same `total_tokens` and `api_equivalent_cost_usd` as the API returns.

### Run log

- Run 1: the first login failed with "the Origin of the request is not this origin", because `Referrer-Policy: no-referrer` makes the browser send `Origin: null`. The private policy changed to `same-origin`.
- Run 1 after the change: page row `laptop | 63 897 600 | ... | 59.25888`; API `laptop 63897600 59.25888`. Passed. The tag group `owner` showed `alice`, `(no token)`, `(none)`, `bob`. Log out went to `/private/login`. Screenshot: `private-reports-page.png`.

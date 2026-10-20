---
date: 2026-10-20
source: cr (CR-0008)
surface: browser
outcome: reproduced
runs: "2 of 2 passed, last run 2026-10-20"
---

## Scenario: a new reader reads the token velocity in five seconds

### Goal

A reader who has not seen the dashboard states the tokens per day figure and its unit within five seconds of the first draw.

### Preconditions

- The release build exists: `cargo build --release --bin loupe`. Run every command from the repository root.
- `K` is the output of `target/release/loupe gen-key`. `S` is an empty scratch dir.
- agent-browser 0.27.1 and Google Chrome are installed.

### Steps at the user surface

1. Run `LOUPE_DATA_KEY=$K LOUPE_ACCOUNT_KEY=$K .agents/scripts/days.seed.sh --data-dir S/full --days 400`.
2. Start `env -i PATH="$PATH" LOUPE_DATA_DIR=S/full LOUPE_DATA_KEY=$K LOUPE_ACCOUNT_KEY=$K LOUPE_TZ=UTC LOUPE_LISTEN=127.0.0.1:8788 target/release/loupe serve`.
3. Run `agent-browser set viewport 360 740`, then `agent-browser open http://127.0.0.1:8788/`, then `agent-browser screenshot`.
4. Give the screenshot to a reader for five seconds. Ask for the figure and its unit.
5. Do steps 3 and 4 again at 1440 px.
6. Compute the expected value: the sum of `input_total + output` from `curl -s http://127.0.0.1:8788/v1/days` over the days from `to` minus 90 days to `to`, divided by 91.

### Success condition

The reader states the figure and the unit "tokens per day". Both equal the page values, and the figure equals the value of step 6 with one decimal in thousands.

### Run log

- Run 1, 360 px: the reader (a model with no earlier view of the page) read "577.3k tokens per day" from `dashboard.headline.360.png`. Expected 52 530 660 / 91 = 577 260, shown as 577.3k. Passed.
- Run 2, 1440 px: the same figure and unit from `dashboard.headline.1440.png`. Passed.
- `.agents/scripts/web.render.sh --days 400` printed `headline_ratio: 2.12 headline_above_fold_360: true headline_above_fold_1440: true`.

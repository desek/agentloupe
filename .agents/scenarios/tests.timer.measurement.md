# Scenario: the test stage is fast after the timer rule

- Goal: the warm test stage gives a verdict in under 15 seconds, and every test passes.
- Surface: the commands `.agents/scripts/pipeline.verdict.sh --no-cache` and `cargo nextest run`.
- Success condition: the second `--no-cache` verdict prints `stage: test status: passed` with seconds under 15. The slowest test is at most 5 seconds. 20 of 20 `cargo nextest run` runs pass.

## Steps

1. Run `.agents/scripts/pipeline.verdict.sh --no-cache` on a warm tree.
2. Run it again, and read the seconds of the `test` line.
3. Run `cargo nextest run --no-fail-fast` 20 times. Count the passes and read the slowest test.

## Runs

- 2026-10-21 (implementation): test 14.5 and 14.0 seconds, slowest test 3.8 seconds, passed.
- 2026-10-21 (verification), load average 19 to 40 on 10 cores from other processes: verdict test 16.7 then 18.1 seconds, `verdict: passed`. `pipeline.test.sh` alone: 14.5, 19.6, 16.8, 16.4, 17.4 seconds. Outcome: failed, 0 of 2 second runs under 15 seconds.
- Flake: 15 of 20 `cargo nextest run` passed. All 5 failures are `private_config_read_is_cached` (`tests/private_config.rs:144`, p95 8.4 ms against a 5 ms bound), a latency test outside the timer change. Slowest test 5.4 seconds (`whole_tree_hit_refreshes_stage_fingerprints`).
- Next: re-run on an idle machine to separate the load from the code.

### Run 2026-10-21 19:29, second verification

- Two runs of `.agents/scripts/pipeline.test.sh`: 21.7 s and 21.5 s, `status: passed`.
- Load average 14 to 23 on 10 cores. Chrome and other processes used the cores.
- Outcome: blocked. The success condition (under 15 s) cannot be measured on a contended machine. Re-run on an idle machine.

<!-- @agents-index Recorded crates.io, GitHub, and claude -p answers for the shell tests of deps.freshness.sh; trimmed real answers of 2026-10-24 with the stated edits, and no credential. -->
# Fixtures of the dependency freshness flow

The shell tests of `.agents/scripts/deps.freshness.sh` (`tests/scripts/deps.freshness.test.sh`) serve these files through a fake `curl` and a fake `gh` on `PATH`, and a fake `claude` prints the `claude.*.json` results. The registry answers were recorded on 2026-10-24 from the real registries (crates.io with the `User-Agent` of the script, at most one request per second) and trimmed to the fields that the script reads and to a few versions. The tests use the scan time `2026-10-24T10:00:00Z`. No file holds a credential.

| File | Shape of | Edits after the recording |
|---|---|---|
| `crates/<name>.json` | `GET https://crates.io/api/v1/crates/<name>`: `crate.name`, `crate.repository`, `crate.max_stable_version`, and `versions[]` with `num`, `created_at`, and `yanked` | `rusqlite`: a pre-release `0.41.0-beta.1` first and `0.38.0` before the highest release `0.40.2` (an out-of-order list). `reqwest`: a yanked `0.13.6` above the latest `0.13.5`. `smallvec`: `1.16.3` yanked and dated `2026-10-14` (a yanked pinned version, 10 days old); its pre-release `2.0.0-beta.3` is as recorded. `dbl`: an aged crate of the GitHub organisation RustCrypto that is not in the name list. |
| `github/checkout.releases.json` | `GET /repos/actions/checkout/releases?per_page=100` | three real releases, plus a draft `v8.0.0`, a pre-release `v8.0.0-beta.1`, and a release with the tag `v9`, none of which may be the latest |
| `github/checkout.ref.v7.0.1.json` | `GET /repos/actions/checkout/git/ref/tags/v7.0.1` | none |
| `claude.update.json` | the `claude -p --output-format json` result of an update run with the structured result `updated` | written by hand from the shape of `tests/fixtures/deps/claude.result.json` |
| `claude.update.breaks.json` | the same with the structured result `breaks` | written by hand |
| `claude.update.invalid.json` | the same with no structured result | written by hand |
| `claude.assess.json` | the result of an assessment run with `replace` and 40 lines | written by hand |
| `claude.assess.large.json` | the same with 800 lines | written by hand |

The tests also read `workflow.verdict.json`, `checks.success.json`, `compare.current.json`, and `runs.release_please.json` of `tests/fixtures/deps/`.

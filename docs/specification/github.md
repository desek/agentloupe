<!-- @agents-index Specification of the GitHub worker: token labels, queries, day range, pull request hash, tables, failures, schedule, and retry. -->
# GitHub

The GitHub worker counts the pull requests that the token users open and merge on each day, and the lines of the merged pull requests. It fills the four GitHub fields of the day record in `days.md`.

## Tokens

Each variable `LOUPE_GITHUB_TOKEN_<LABEL>` gives one token. The GitHub token label is the part after `LOUPE_GITHUB_TOKEN_`, in uppercase, as the configuration loader parses it.

If no token is set, the worker does not start and does not call the network. It logs one line `github: disabled, no token` at start. All other workers and routes work as before, and the four GitHub fields of the record stay `null`.

The token values stay in memory only. No table, log line, error, or output of `loupe config show` holds a token value or a part of it.

The variable `LOUPE_GITHUB_API_URL` sets the base URL of the GitHub API. The default is `https://api.github.com`. The value must use `https`, except for the host `127.0.0.1` or `localhost`. This is the same rule as the upstreams of `proxy.md`. The tests use this variable to point the worker at a local fake GitHub, and never call `api.github.com`.

## Client

Each request sends the token as a bearer token, the GitHub JSON media type in `Accept`, the API version `2022-11-28`, and the user agent `agent-loupe/<crate version>`. Each request has a timeout of 30 seconds.

At the start of each fetch, the client gets the login of the token user from the user endpoint, one time.

## Day range

The day range is the start and the end of the day in `LOUPE_TZ`, written in ISO 8601 with the UTC offset of that moment. An example is `2026-10-17T00:00:00+03:00..2026-10-17T23:59:59+03:00`. The two ends come from the day boundaries of `days.md`. On a day with a change of the UTC offset, each end carries its own offset.

## Queries

The client does two searches for the token user and the day range:

1. The opened pull requests: the pull requests of the author that were created in the range.
2. The merged pull requests: the pull requests of the author that were merged in the range.

Each search asks for 100 items per page. The client follows the pages until a page has fewer than 100 items. The search API returns at most 1000 results. If the total count of a search is more than 1000, the fetch fails with the cause `search_limit`, because a part count is wrong.

For each merged pull request, the client gets the added lines and the removed lines from the pull request detail endpoint. It takes the owner, the repository, and the number from the repository URL and the number of the search item.

## Retry

On a `403` or `429` response with a `retry-after` header or with no remaining rate limit, the client waits the stated time, at most 120 seconds. It tries again at most 3 times. After that, the fetch fails with the cause `rate_limited`.

## Pull request hash

The worker identifies a pull request by the pull request hash. The hash is the HMAC-SHA256 of the text `owner/repo#number` under `LOUPE_ACCOUNT_KEY`, written as lowercase hex. The owner and the repository are in lowercase. Thus the same pull request seen by two tokens gets one hash.

The binary never stores, logs, or returns the repository name, the owner, the number, the title, the URL, or the login. These values stay in memory only for the fetch.

## Tables

One migration adds three tables.

| Table | Columns | Rule |
|---|---|---|
| `github_prs` | `pr_hash` (primary key), `opened_day`, `merged_day`, `additions`, `deletions`, `updated_at` | One row for each pull request. A day is `YYYY-MM-DD` or null. The line counts are integers or null. |
| `github_pr_labels` | `pr_hash`, `label` (the pair is the primary key) | One row for each label that saw the pull request. Analysis by label uses this private table only. |
| `github_fetches` | `day`, `label` (the pair is the primary key), `status`, `cause`, `fetched_at` | One row for each GitHub fetch. The status is `ok` or `failed`. The cause is null on `ok`. A new fetch replaces the row. |

A GitHub fetch is one run of the client for one day and one label. A fetch writes its pull request rows and its fetch row in one transaction. A failed fetch writes only its fetch row, so a part result never changes a count. A second fetch of the same day and label gives the same rows, so a fetch is idempotent.

## Failures

| Cause | When |
|---|---|
| `unauthorized` | The user endpoint returns `401` or `403`. |
| `network` | A network error. |
| `rate_limited` | The rate limit continues after 3 retries. |
| `search_limit` | A search has a total count of more than 1000. |
| `http_<status>` | Any other status. |

On each failure, the worker logs one line at level `warn` with the fields `github_label`, `day`, and `cause`. The line also gives the fix: renew or approve the token, then run `loupe aggregate --date <day>`, then verify that the fetch row has the status `ok`. The line names the README section "GitHub" as the canonical fix for the token permissions.

A failure of one label does not stop the fetches of the other labels or the build of the day.

## Day record

The day builder also reads the pull request table and the fetch table. If no token is set, the four fields are `null`. Otherwise:

| Field | Value |
|---|---|
| `prs_opened` | The number of distinct pull request hashes with an opened day equal to the day. |
| `prs_merged` | The number of distinct pull request hashes with a merged day equal to the day. |
| `lines_added` | The sum of the added lines of the pull requests merged on the day. |
| `lines_removed` | The sum of the removed lines of the pull requests merged on the day. |

The record holds `detail.github.failed_tokens`: the number of labels that have no fetch row with the status `ok` for the day. The value is `0` when no label is missing. The record never holds a label name or a count for each label. The label names stay in the private tables and in the log lines. The field allowlist of `days.md` contains `github` and `failed_tokens`.

## Activity source

The worker registers a GitHub source through the activity source interface of `days.md`. A day has GitHub activity when the pull request table has one or more rows with an opened day or a merged day equal to that day. The oldest such day is the first activity day of the source, or none. After a fetch that changes a row, the worker puts the day in the aggregation queue with the reason `github`. Thus a due day with pull requests and no call gets a record with `calls: 0`. A day with no call and no pull request gets no record.

## Schedule

On each wake, the worker scans the due days. For each label, it fetches each due day that has no day record and no fetch row with the status `ok` for that label, oldest first. The scan does not depend on the days that the calls source makes due. The scan starts at the start day and ends at the newest due day. The start day is the later of two days: the oldest first activity day of the calls source, and the current day minus 90 days. If the calls source has no activity day, the start day is the current day minus 90 days. Thus the lookback is at most 90 days. The worker finishes the scan before the publisher selects the due days to publish on the same wake.

On each wake, the publisher also fetches again each published day of the last 7 days that has a label with no fetch row with the status `ok`. After a fetch that changes a row, it puts the day in the aggregation queue with the reason `github`. A pull request that is merged after the publish of a day counts on its own merge day, so the 7-day window covers only failed tokens.

`loupe aggregate --date <day>` fetches the day for each label before it builds the day. With `--dry-run`, it does not call GitHub and builds from the stored rows.

The worker runs in its own task, so the proxy latency does not change.

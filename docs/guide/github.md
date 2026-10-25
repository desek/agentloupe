<!-- @agents-index User guide: How loupe reads the pull request counts from GitHub, and the token permissions. -->

# GitHub

## GitHub

A worker in the binary reads pull requests from the GitHub API for each day, at the same time
as the day is published. It catches up missed days in the same way as the rest of the day.

- **Tokens.** Set one or more variables `LOUPE_GITHUB_TOKEN_<LABEL>`, for example
  `LOUPE_GITHUB_TOKEN_PERSONAL` and `LOUPE_GITHUB_TOKEN_ACME`. The binary records the label with
  each pull request, so an analysis can group by it. The tokens stay in the environment. The
  database never stores them.
- **What it counts.** For each token, the worker gets the user of the token (`GET /user`). It
  then searches (`GET /search/issues`) for pull requests by that user that were opened on the
  day and that were merged on the day. The day is the date in `LOUPE_TZ`, sent as a time range
  with its UTC offset. For each merged pull request, it gets the additions and the deletions
  (`GET /repos/{owner}/{repo}/pulls/{pull_number}`).
- **No double counts.** Two tokens can see the same pull request. The worker identifies a pull
  request by the HMAC-SHA256 of `owner/repo#number` with `LOUPE_ACCOUNT_KEY`, and counts it one
  time. It never stores the repository name, the number, or the URL.
- **Day fields.** `prs_opened`, `prs_merged`, `lines_added`, and `lines_removed`. The lines
  count on the day of the merge.
- **Failures.** If a token fails (expired, revoked, not approved), the worker logs the label and
  the cause. The day records the number of failed tokens in `detail.github.failed_tokens`. The
  label names stay in the private database and in the logs. If a search finds more than 1000
  pull requests, the fetch fails with the cause `search_limit`, because the search API returns
  at most 1000 results and a part count is wrong.
- **Retry.** On each wake, the worker fetches again each published day of the last 7 days that
  has a failed token. A day that changes gets a new revision. After you fix a token, you can
  also run `loupe aggregate --date <day>`.

Required permissions for a fine-grained personal access token (recommended):

| Setting | Value |
|---|---|
| Resource owner | Your user account, or one organization. A fine-grained token reaches one owner only, so make one token for each owner. |
| Repository access | All repositories, or only the repositories to count. |
| Repository permissions: Pull requests | Read-only |
| Repository permissions: Metadata | Read-only. GitHub adds this permission automatically. |
| Account permissions | None |
| Organization permissions | None |

An organization can require approval for fine-grained tokens. Such a token works only after the
organization approves it. The search endpoint needs no extra permission. It returns only the
pull requests in repositories that the token can see, plus pull requests in public repositories.

A classic token also works, but it is not recommended. For private repositories it needs the
`repo` scope, which also gives write access to all of them. For public repositories only, a
classic token with no scope is sufficient.

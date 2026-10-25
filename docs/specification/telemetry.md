<!-- @agents-index Specification of the start telemetry: the build state, the one start request of loupe serve, the receiving Worker and its store, the deploy script, and the free plan bound. -->
# Telemetry

A release build of `loupe` sends one HTTP GET when `loupe serve` starts. The request tells the owner which commit of the project runs. It holds no data of the user, the host, or the dashboard. A build from source and a local image build send nothing. A Cloudflare Worker in this repository receives the request and keeps one count for each commit.

## Build state

The build of the binary decides if the binary holds a telemetry host. `configuration.md` (Build variables) gives the rules of the build variables.

| Build state | Condition at build time | The binary holds |
|---|---|---|
| `built-out` | `LOUPE_TELEMETRY_HOST` is absent, or empty after the removal of white space at the start and the end | no telemetry host and no telemetry URL |
| `built-in` | `LOUPE_TELEMETRY_HOST` holds a valid host | the host, the full commit of the build, and the target triple of the build |

A `built-out` binary still holds the send code, so the verdict can test it with a local fake endpoint. The start path of a `built-out` binary has no target, so it sends nothing.

The run-time environment does not change the host. A value of `LOUPE_TELEMETRY_HOST` in the environment of `loupe serve` has no effect.

## Start request

When the binary holds a host, `loupe serve` sends exactly one start request at each start, unless the run-time opt-out is set. This holds in the role `full` and in the role `public`, with the same rules, and each role writes the line `telemetry`. Two services send two requests per start.

1. The method is `GET`. The URL is `https://<host>/<commit>`. The scheme is `http` only when the host is `127.0.0.1` or `localhost`, for a local fake endpoint.
2. The path is `/` and the full commit of the build: exactly 40 characters, each a digit or a lowercase letter from `a` to `f`. The URL has no query and no fragment. The URL is the schema. The request has no schema field. A later change of the shape of the path is a new schema.
3. The request has no body. Its size is 0 bytes.
4. The header `user-agent` has the form `agentloupe/<version> (<target triple>)`. The version comes from `Cargo.toml` at build time. The target triple is the target of the build. For example: `agentloupe/0.2.0 (aarch64-apple-darwin)`.
5. The request has no `authorization` header, no cookie, and no `content-type` header. The binary sets no other header. The headers that the HTTP client adds for the transport stay.
6. The URL and the headers hold no host name of the user's machine, no user name, no file path, no working directory, no account, no key, no token, no credential, no configuration value, no time, no IP address of the host, and no data of the dashboard, the calls, or the sessions.
7. The request has one attempt and no retry. The timeout of the attempt is 5 seconds, from the start of the connect to the end of the response.
8. The client follows no redirect. Thus a redirect cannot send the commit to a host that the build did not name.
9. The request uses its own HTTP client with rustls. It never goes through the proxy routes of `loupe`. The capture never records it. The database never stores it. The live feed sends no event for it. The dashboard and the public API never show it.

### Timing

1. The request starts after the listener is bound and after the log line `listening`.
2. The request runs in its own task. The bind, the line `listening`, and the first answer of `GET /healthz` do not wait for it.
3. A shutdown signal does not wait for an open start request. The shutdown rules of `operations.md` (Shutdown) stay the same.

### Result

The result of the request does not change the start or the run of `loupe serve`. A 2xx status is not necessary. Each status code, a network error, and a timeout give one log line at debug level, and nothing more. The debug line holds the host and the status code or the error kind. It does not hold the scheme, the path, the commit, or the query of the URL.

## Telemetry state and the log line

At each start, after the line `listening`, `loupe serve` writes one info line `telemetry` with the key field `state`:

| State | Condition | Other key field |
|---|---|---|
| `built-out` | The binary holds no host. | none |
| `disabled` | The binary holds a host and the run-time opt-out is set. | `host`, the host |
| `sent` | The binary holds a host and the request started. | `host`, the host |

1. The state `sent` means that the request started. The line does not wait for the network. The outcome goes to the debug line.
2. The line holds the host only. It does not hold the scheme, the port, the path, the commit, the query, or the `user-agent` value.
3. The line is at info level. Thus a normal start still logs no warning and no error.
4. In the state `built-out`, the start path makes no request and reads no run-time opt-out.

`loupe config show` prints the build state only. `configuration.md` (`loupe config show`) gives the line.

## Telemetry endpoint

The receiver is the Cloudflare Worker `agentloupe-telemetry`. Its source is under `telemetry/` in this repository: one `wrangler` configuration file, the Worker source, one D1 migration under `telemetry/migrations/`, and the Worker test under `telemetry/test/`.

1. The Worker is served on the Cloudflare custom domain `telemetry.agentloupe.dev`, on the zone `agentloupe.dev` of the owner's account. `agentloupe.dev` is the one product domain. The `wrangler` configuration names the custom domain as its one route, and the deploy makes the DNS record and the certificate of the host.
2. The `wrangler` configuration sets `workers_dev` to false and `preview_urls` to false. Thus the `workers.dev` host of the Worker does not reach the Worker: Cloudflare answers it with 404 and the error code 1042, and the Worker writes nothing.
3. The Worker has one D1 database with the name `agentloupe-telemetry`.

### Store

The D1 database has one table `starts`. Each row is one telemetry record. The table has exactly four columns:

| Column | Holds |
|---|---|
| `sha` | the commit, as text; the primary key |
| `firstseen` | the UTC date of the first request for the commit, as text in the form `YYYY-MM-DD` |
| `lastseen` | the UTC date of the last request for the commit, as text in the form `YYYY-MM-DD` |
| `count` | the number of accepted requests for the commit, as an integer |

The Worker stores nothing else: no IP address, no `user-agent`, no header, no query, no time of day, and no country. It writes no request data to a log. The `wrangler` configuration turns Workers Logs (observability) off, and adds no Logpush and no tail consumer.

### Route

The Worker accepts one request shape: the method `GET` and the path `/` and a `sha` of exactly 40 characters, each a digit or a lowercase letter from `a` to `f`.

| Request | Store | Answer |
|---|---|---|
| An accepted request for a new `sha` | One atomic insert-or-update adds the row: `firstseen` and `lastseen` are the current UTC date, and `count` is 1. | 204, no body |
| An accepted request for a known `sha` | The same atomic insert-or-update sets `lastseen` to the current UTC date and adds 1 to `count`. `firstseen` does not change. | 204, no body |
| An accepted request when the store fails | no change | 503, no body |
| Each other path or method | no write and no read | 404 |

1. The current date is the UTC date at the Worker.
2. The Worker runs at most one D1 statement for each request, and none for a refused request.
3. The 404 cases include the root path, a `sha` of another length, a `sha` with uppercase letters, a path with more segments, and the methods `HEAD` and `POST`.
4. The Worker ignores the query of a request and does not store it. A query on an accepted path does not make the request a refused request.
5. The Worker has no read route. No request reads a row.
6. The route is public, so anyone can send a valid request. Thus the count is a signal and not an exact number. A row whose `sha` is not a commit of this repository is noise. The store holds no personal data, so a fake request leaks nothing.

### Worker test

The Worker test under `telemetry/test/` runs the Worker locally with `wrangler dev` and a local D1 database. It needs no Cloudflare account, no login, and no network to the account. It has three cases:

| Case | Checks |
|---|---|
| insert | A request for a new `sha` answers 204 with no body. The table holds one row with the `sha`, `firstseen` and `lastseen` equal to the UTC date, and `count` 1. |
| increment | A second request for the same `sha` answers 204. The row has `count` 2, the same `firstseen`, and `lastseen` equal to the UTC date. The table still holds one row. |
| rejection | Each 404 case of "Route" answers 404. The row count and each `count` stay the same. A query on an accepted path is not stored. |

The verdict does not run the Worker test. The deploy script runs it.

## Deploy script

`.agents/scripts/telemetry.deploy.sh` deploys the Worker through `wrangler`. `.tool-versions` pins `npm:wrangler 4.100.0`. The hosted workflows install only the tools that each job names, so the pin adds no install to a workflow.

1. The script follows the rules of the other scripts: the `@agents-index` line, a docstring with the purpose, the usage, and the parameters, a complete `--help`, a machine output on stdout by default, `--human` for the friendly view, and actionable errors on stderr. The machine output has one `key: value` line for each step and one summary line with the host.
2. The script calls `wrangler` through the pin of `.tool-versions`. It checks that the version of `wrangler` equals the pin.
3. A real run does these steps in this order:
   1. It checks the pin.
   2. It runs the Worker test.
   3. It checks the custom domain: the `wrangler` configuration holds exactly one route with `custom_domain = true`, its host is under the zone `agentloupe.dev`, and `workers_dev` is false.
   4. It checks the `wrangler` login.
   5. It makes the D1 database `agentloupe-telemetry` only when the account has no database of that name.
   6. It applies the migration to the remote database.
   7. It deploys the Worker. The deploy output must list the custom domain.
   8. It prints the host, which is the custom domain `telemetry.agentloupe.dev`, and the command that sets the repository variable `LOUPE_TELEMETRY_HOST` to that host.
4. `wrangler` binds a D1 database by its ID. On the first real run, the script writes the ID of the new database into the `wrangler` configuration. The owner commits that change. The ID is not a secret.
5. `--dry-run` runs the pin check, the Worker test, the custom domain check, the login check, and the dry run of the `wrangler` deploy. It prints each step that a real run does. It makes no change on the Cloudflare account and no change in the repository.
6. `--test` runs the Worker test alone.
7. The script is idempotent. A second run with no change in `telemetry/` makes no second database, applies no migration again, prints the same host, and exits 0.
8. The script stores no Cloudflare credential. It does not set the repository variable. The owner sets it one time. No workflow and no pipeline script runs the script. Only the owner runs it, on the owner's machine, with the owner's consent.

| Exit code | Meaning |
|---|---|
| 0 | Done. |
| 1 | A step failed. |
| 2 | A usage error. |
| 3 | `wrangler` is missing or is not the pinned version. The error names the pin and the fix. |
| 4 | `wrangler` is not logged in. The error names the `wrangler` login. |
| 5 | The `wrangler` configuration has no single custom domain under `agentloupe.dev`, or `workers_dev` is not false. The error names the custom domain and the fix. |

## Release builds

The release workflow passes the repository variable `LOUPE_TELEMETRY_HOST` and the full commit of the tag to the build. Thus the release assets and the release image are `built-in`, and send the start request. A local build and a local image build get no host, so they are `built-out` and send nothing. When the repository variable is not set, the release does not fail and its builds are `built-out`. `release.md` gives the release rules.

## Free plan bound

The Worker and its database run on the free plan of the owner's Cloudflare account. The free plan caps the Worker requests per day and the D1 rows written per day, at about 100 000 of each per day at the time of writing. One insert-or-update can count as more than one row written, because the index of the primary key counts.

1. The binary sends exactly one request for each start, with no retry.
2. The Worker runs at most one statement for each request, and none for a refused request.
3. Above a cap, the Worker or the store fails until the next day, and the counts of that day are lost. The Worker answers 503 or the platform refuses the request. The binary ignores the answer, so the user sees no effect.
4. The owner reads the usage in the Cloudflare dashboard. A paid plan is not part of the project.

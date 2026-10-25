<!-- @agents-index User guide: Run the public instance that serves only the public page and the public API. -->

# Public hosting

## Public hosting

`compose.yaml` runs two services from one image:

| Service | Role | Port |
|---|---|---|
| `loupe` | `full`: the proxy, the workers, the private page, the public routes, and the mirror writer | `127.0.0.1:8787` |
| `loupe-public` | `public`: the dashboard, the public API, and the live feed only | `127.0.0.1:8788` |

Expose only the port 8788. Never expose the port 8787, because it holds the proxy and the
private page.

The public container reaches only the public mirror, read-only, with the published days and the
live counters. It cannot reach `loupe.db`, the raw requests, the session data, the spill files,
the logs of the role `full`, or the secrets. It never mounts the volume `loupe-data` and never
reads `.env`. A local `compose.override.yaml` that mounts the session logs for `loupe backfill`
must name only the service `loupe`, never `loupe-public`.

A Cloudflare Tunnel is later work. Put only the public port, or the service `loupe-public`,
behind the tunnel. A tunnel on the compose network can reach the service `loupe`, so the tunnel
configuration must name `loupe-public` only.

A rate limit and a cache in front of the public port are the job of the host, for example the
rules of Cloudflare. The binary has no rate limit on the public routes beyond the live connection
limit.

Put the settings of the public service in `.env.public`, from `.env.public.example`, with no
secret. `LOUPE_LIVE_ALLOWED_ORIGIN` covers a front proxy that changes the `Host` header, and
`LOUPE_LIVE_MAX_CONNECTIONS` limits the live sockets. Copy `LOUPE_TZ`, `LOUPE_SITE_TITLE`, and
the `LOUPE_SUBSCRIPTION_` lines from `.env`, so the two pages match.

1. Run `cp .env.public.example .env.public` and fill in the values.
2. Run `docker compose up -d --build`.
3. Check with `curl -s http://127.0.0.1:8788/healthz`. The body is `{"status":"ok"}`.

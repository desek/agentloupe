<!-- @agents-index Specification of the provider account hash, the order of its sources, and the accounts table. -->
# Provider accounts

A provider account is the identity of the account at the model provider. The binary stores only an HMAC-SHA256 of a stable account ID or of the credential, under `LOUPE_ACCOUNT_KEY`. It never stores the raw account ID or the raw credential.

## Source order

The parser gives each call the account of the first present source:

| Order | Source | Value |
|---|---|---|
| 1 | `header:<name>` | The account ID hash that the capture wrote from an account ID header. See `proxy.md`, section "Account ID hash". |
| 2 | `metadata.user_id` | The account UUID in `metadata.user_id` of the request body. The parser hashes it with HMAC-SHA256 under `LOUPE_ACCOUNT_KEY`. |
| 3 | `credential:<header name>` | The credential hash that the capture wrote. See `proxy.md`, section "Credential hash". |

The parser never computes a credential hash again. It uses only the hashes that the capture wrote. The credential hash is always the last source. The section "Verified sources per agent" of `parser.md` records the source that applies to each agent version.

## Accounts table

The table `accounts` holds one row for each (hash, source):

| Column | Value |
|---|---|
| `id` | A unique id. |
| `hash` | The HMAC as lowercase hex. |
| `source` | The name of the source. |
| `first_seen` | The time of the first call with this account. |
| `last_seen` | The time of the last call with this account. |

Each call stores the account id. A call with no source stores a null account id. A call with origin `log` has the account `none`: its account id is null, because the logs hold no account ID and no credential. See `backfill.md`. The account id and the proxy token ID are independent.

## Privacy

The count of provider accounts is private. A route under `/v1/` does not read `accounts`, a key hash, or a proxy token ID. A public route and a day record do not show the count.

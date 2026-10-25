<!-- @agents-index User guide: How to make, list, and revoke the proxy tokens that the agents use. -->

# Proxy tokens

## Proxy tokens

A proxy token lets a client use the proxy. You can make many proxy tokens, for example one for
each machine, each person, or each CI pipeline. The binary records which token sent each
request, so you can later analyse the usage by token and by the metadata of the token.

- **Format.** A random value with the prefix `lpt_`. The prefix lets secret scanners find a
  leaked token.
- **Storage.** The binary stores only the SHA-256 hash of the token. It shows the raw token one
  time, when you make it. A plain hash is sufficient here, because the token is made from
  random bytes. The HMAC of the "Accounts" section is necessary for values that the binary does
  not make.
- **Metadata.** Each token has a unique name and free `key=value` tags, for example
  `machine=work-laptop`, `owner=daniel`, or `purpose=ci`. The binary also records when the
  token was made, when it was last used, and when it was revoked.
- **Tracking.** Each exchange, call, and session stores the ID of its proxy token. A session
  never mixes two tokens. The provider account hash of the "Accounts" section is recorded
  independently, so an analysis can group by token, by account, or by both.
- **Privacy.** Token names and tags stay in the database. The public API does not show them.
- **Use.** The client puts the token first in the route: `/<token>/<agent>/<upstream>/<path>`.
  Before the first token exists, the routes have no token part, and the binary logs a warning at
  start, because anyone who can reach the port can use the proxy. After the first token exists,
  every proxied request needs a valid active token. If no token is active, for example after you
  revoke the last one, the proxy refuses all proxied requests until you make a new token.
- **Bootstrap.** A deployment without a shell can set `LOUPE_BOOTSTRAP_TOKEN` from a secret. At
  start, the binary records it as the token named `bootstrap`, as a hash only. If the value
  changes, the binary replaces the hash. If the variable is removed, the binary revokes the
  `bootstrap` token.
- **Commands.**
  - `loupe token generate`: print a new random token and store nothing. Use it to make the value
    of `LOUPE_BOOTSTRAP_TOKEN`.
  - `loupe token create --name NAME [--tag KEY=VALUE]...`: make a token and print it one time.
  - `loupe token list`: show the name, the tags, and the dates of each token, never the token.
  - `loupe token revoke NAME`: stop the token. Its past records stay.

<!-- @agents-index Security policy: supported versions, private vulnerability reporting as the one channel, response times, and disclosure after the fix release. -->
# Security policy

## Supported versions

Agentloupe is in the 0.x series. Only the newest minor release gets security fixes.

| Version | Supported |
|---|---|
| newest 0.x minor release | yes |
| older releases | no |

## Report a vulnerability

Please do not open a public issue, a discussion, or a pull request for a vulnerability.

Use the private vulnerability reporting of GitHub. It is the one channel for a report:

1. Open the [Security tab](https://github.com/desek/agentloupe/security) of the repository.
2. Select "Report a vulnerability".
3. Describe the issue, the affected version, the steps to reproduce it, and the impact.

Only the maintainer sees the report.

## Response times

| Step | Time |
|---|---|
| Acknowledgement of the report | within 7 days |
| Assessment, with the severity and a plan | within 30 days |

The maintainer keeps you informed in the private advisory while a fix is made.

## Disclosure

The fix ships in a new release. After the release, the maintainer publishes the security advisory
on GitHub and credits the reporter, unless the reporter asks not to be named. Please keep the
details private until the advisory is public.

## Scope

In scope: the `loupe` binary, the container image `ghcr.io/desek/agentloupe`, and the release
assets of this repository. The prototype under `prototype/` and the early prototype archive (removed) are
not shipped and are out of scope.

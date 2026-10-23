<!-- @agents-index How to contribute: issue first, change requests, Conventional Commits, the local verdict, the required check, squash merge, and the MIT license of contributions. -->
# Contributing

Thank you for your interest in Agentloupe. This guide is short on purpose. The
[README](README.md) describes the product, and [`docs/specification/`](docs/specification/) describes
the behaviour of the code.

## Propose a change

1. Open an issue first. Use the bug report or the feature request template. Describe the problem
   before the solution, so we can agree on the scope before you write code.
2. For a vulnerability, do not open an issue. Follow [SECURITY.md](SECURITY.md).
3. When the change is agreed, open a pull request against `main` from a branch of your fork.

## Change requests

A change of behaviour starts as a change request under [`docs/cr/`](docs/cr/). A change request
states the motivation, the requirements, and the acceptance criteria. The specification under
`docs/specification/` changes first, then the code. A small fix of a behaviour that the
specification already states needs no change request.

## Commits

Each commit follows [Conventional Commits](https://www.conventionalcommits.org/): `feat`, `fix`,
`perf`, `revert`, and the hidden types `docs`, `chore`, `test`, `ci`, `build`, `refactor`, `style`,
and `checkpoint`. The commit types decide the version bump and the release notes.

The work of a change request is a series of `checkpoint(<id>)` commits. One implemented commit
carries the change request into the release notes: a `feat`, `fix`, or `feat!` commit whose summary
is the title of the change request with its id in parentheses, for example
`feat(release): Release governance (<id>)`, where `<id>` is the id of the change request.
[`docs/specification/release.md`](docs/specification/release.md), section "Commit rule for a change
request", states the full rule.

## Check your change

Run the local verdict before you push:

```sh
.agents/scripts/pipeline.verdict.sh
```

It runs the stages format, lint (with the dependency check), test, web, and image, and exits 0 only
when each stage passes. The tools come from `rust-toolchain.toml` and `.tool-versions`;
`mise install` installs the tools of `.tool-versions`.

## Pull requests

* The required check `verdict` must pass before a pull request can merge. It runs the same stages
  on GitHub Actions.
* Each pull request merges as one squash commit. The title of the pull request becomes the commit
  summary, so the title follows Conventional Commits.
* Update the specification or the README in the same pull request when the behaviour changes.
* Never commit a secret. `.env` stays local; `.env.example` lists the names only.

## License

Agentloupe is under the [MIT license](LICENSE). By contributing, you agree that your contribution
is licensed under the same license.

## Conduct

This project follows the [Code of Conduct](CODE_OF_CONDUCT.md).

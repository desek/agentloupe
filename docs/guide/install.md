<!-- @agents-index User guide: Install loupe from a release asset, the registry image, or the source, and check a release. -->

# Install

## Install

There are three ways to get `loupe`. Each release has a tag `v<version>`, for example `v0.1.0`, and
`<version>` below is the version with no `v`. [`docs/specification/release.md`](../specification/release.md)
states the versions, the release flow, and the assets.

### From a release asset

Each GitHub release of [desek/agentloupe](https://github.com/desek/agentloupe/releases) has one
archive per platform and one `.sha256` checksum file per archive:

| Platform | Archive |
|---|---|
| macOS arm64 | `loupe-v<version>-aarch64-apple-darwin.tar.gz` |
| Linux x86_64 (glibc 2.35 or newer) | `loupe-v<version>-x86_64-unknown-linux-gnu.tar.gz` |
| Linux arm64 (glibc 2.35 or newer) | `loupe-v<version>-aarch64-unknown-linux-gnu.tar.gz` |

1. Download the archive of your platform and its checksum file:
   ```sh
   V=<version>; A=loupe-v$V-aarch64-apple-darwin.tar.gz
   curl -fLO https://github.com/desek/agentloupe/releases/download/v$V/$A
   curl -fLO https://github.com/desek/agentloupe/releases/download/v$V/$A.sha256
   ```
2. Check the checksum: `shasum -a 256 -c $A.sha256` on macOS, `sha256sum -c $A.sha256` on Linux.
   Each prints `OK`.
3. Unpack: `tar -xzf $A`. The directory `loupe-v$V-<target>/` holds `loupe`, `upstreams.toml`,
   `config/`, `LICENSE`, `README.md`, the user guides under `docs/guide/`, the screenshot
   `docs/images/dashboard.png`, and `THIRD-PARTY-NOTICES` (the licenses of the third-party
   parts of the binary). The binary reads `upstreams.toml` and `config/` from the
   working directory, so run it from that directory.
4. Run: `cd loupe-v$V-*/ && ./loupe --version` prints `loupe <version>`. Then
   `LOUPE_DATA_KEY=... LOUPE_ACCOUNT_KEY=... ./loupe serve`, with two keys from `./loupe gen-key`.
5. The macOS binary is not signed. A download through a browser can get the quarantine attribute;
   remove it with `xattr -d com.apple.quarantine loupe`. A download with `curl` does not get it.

### From the registry image

The image `ghcr.io/desek/agentloupe` has the tags `<version>` (no `v`) and `latest`, for
`linux/amd64` and `linux/arm64`. A pull needs no login. The command below uses the same port, volume,
and env file as `compose.yaml`:

```sh
docker pull ghcr.io/desek/agentloupe:<version>
docker run -d --name loupe --restart unless-stopped \
  -p 127.0.0.1:8787:8787 -v loupe-data:/data --env-file .env -e LOUPE_ENV=container \
  ghcr.io/desek/agentloupe:<version>
```

`docker run --rm ghcr.io/desek/agentloupe:<version> gen-key` prints a key for `.env`.

### From source

Install Rust with rustup. In the repository, rustup reads `rust-toolchain.toml` and uses that
toolchain. Then build and run:

```sh
cargo build --release --locked --bin loupe
./target/release/loupe --version
```

The local image build of [Connect the agents](connect-agents.md#connect-the-agents) (`docker compose up -d --build`)
also builds from source.

A build from source sends no start request: the build variable `LOUPE_TELEMETRY_HOST` is unset, so
the binary holds no telemetry host and `loupe config show` prints `telemetry` as `built-out`. Only
the release workflow sets it (see [Privacy model](privacy.md#privacy-model)).

The receiver of the start request lives under [`telemetry/`](../../telemetry/). Only the owner deploys it,
with `.agents/scripts/telemetry.deploy.sh` and the `wrangler` of `.tool-versions`: `--test` runs the
Worker test on a local database, and `--dry-run` shows the steps and changes nothing. The Worker
answers on the custom domain `telemetry.agentloupe.dev` only; its `workers.dev` host is off.

### Check a release

`.agents/scripts/release.check.sh --version <version>` checks a published release on a macOS or Linux
machine with `curl` and Docker: it downloads the six assets with no credential, verifies the
checksums, runs `loupe --version` of each binary, pulls the image with no login, checks
`GET /healthz`, the version in the first log line, and the line `telemetry`, and checks
`THIRD-PARTY-NOTICES` in each archive and in the image. Its starts send no start request. It exits 0 when each check passes or is
skipped with a stated reason.

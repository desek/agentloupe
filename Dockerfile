# @agents-index Two-stage image: build loupe with the toolchain that rust-toolchain.toml pins, run it as a user that is not root.

# The build image carries rustup; rustup reads rust-toolchain.toml and installs that exact toolchain,
# so this file states no Rust version of its own.
# Each base image is pinned by the digest of its multi-platform index, so two builds of one commit get the
# same layers on linux/amd64 and linux/arm64. A human refreshes the digests at each minor release with
# docker buildx imagetools inspect <image>:<tag> (its Digest: line); docs/specification/operations.md (Container).
# Tag: rust:slim-bookworm
FROM rust:slim-bookworm@sha256:86234e857a413004348b471d11a4ebe1aa92d2eae2b6b143cac68fd1ac3c48be AS build
WORKDIR /src
COPY rust-toolchain.toml ./
RUN rustup show active-toolchain || rustup toolchain install
COPY Cargo.toml Cargo.lock ./
COPY build.rs ./
COPY web ./web
COPY vendor/litellm ./vendor/litellm
COPY src ./src
COPY migrations ./migrations
COPY tests ./tests
COPY examples ./examples
# The host and the commit of the start request, in the build stage only; each is empty by default, so a
# local image build is built-out. release.image.sh passes both for a release; the context holds no .git.
ARG LOUPE_TELEMETRY_HOST=""
ARG LOUPE_BUILD_COMMIT=""
RUN cargo build --release --locked --bin loupe

# debian slim has bash, so the HEALTHCHECK can use /dev/tcp and the image needs no curl.
# Tag: debian:bookworm-slim
FROM debian:bookworm-slim@sha256:7ff6ba31527b9e336162fa061746c366c4b2032f4d2cff2078bea43f78efe3f8
ARG UID=10001
ARG GID=10001
RUN groupadd --gid "$GID" loupe \
 && useradd --uid "$UID" --gid "$GID" --no-create-home --shell /usr/sbin/nologin loupe \
 && mkdir -p /data/public && chown loupe:loupe /data /data/public
WORKDIR /app
COPY --from=build /src/target/release/loupe /usr/local/bin/loupe
COPY config ./config
COPY upstreams.toml ./
# Each image holds the license of the project. The release image also holds the third-party notices that
# release.notices.sh writes before the release build; the bracket makes the notices optional, so a local
# build with no notices file still passes.
COPY LICENSE THIRD-PARTY-NOTICE[S] ./
ENV LOUPE_ENV=container
USER loupe
EXPOSE 8787
VOLUME ["/data"]
HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --start-interval=1s --retries=3 \
  CMD ["bash", "-c", "exec 3<>/dev/tcp/127.0.0.1/8787 && printf 'GET /healthz HTTP/1.0\\r\\n\\r\\n' >&3 && grep -q '\"status\":\"ok\"' <&3"]
ENTRYPOINT ["loupe"]
CMD ["serve"]

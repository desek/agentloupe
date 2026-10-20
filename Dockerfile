# @agents-index Two-stage image: build loupe with the toolchain that rust-toolchain.toml pins, run it as a user that is not root.

# The build image carries rustup; rustup reads rust-toolchain.toml and installs that exact toolchain,
# so this file states no Rust version of its own.
FROM rust:slim-bookworm AS build
WORKDIR /src
COPY rust-toolchain.toml ./
RUN rustup show active-toolchain || rustup toolchain install
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations
COPY tests ./tests
RUN cargo build --release --locked --bin loupe

# debian slim has bash, so the HEALTHCHECK can use /dev/tcp and the image needs no curl.
FROM debian:bookworm-slim
ARG UID=10001
ARG GID=10001
RUN groupadd --gid "$GID" loupe \
 && useradd --uid "$UID" --gid "$GID" --no-create-home --shell /usr/sbin/nologin loupe \
 && mkdir /data && chown loupe:loupe /data
WORKDIR /app
COPY --from=build /src/target/release/loupe /usr/local/bin/loupe
COPY config ./config
ENV LOUPE_ENV=container
USER loupe
EXPOSE 8787
VOLUME ["/data"]
HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=3 \
  CMD ["bash", "-c", "exec 3<>/dev/tcp/127.0.0.1/8787 && printf 'GET /healthz HTTP/1.0\\r\\n\\r\\n' >&3 && grep -q '\"status\":\"ok\"' <&3"]
ENTRYPOINT ["loupe"]
CMD ["serve"]

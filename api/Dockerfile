# syntax=docker/dockerfile:1
# Multi-stage build for the Underground API. CI builds it natively on an arm64 runner for the arm64 box.

FROM rust:1.96-slim-bookworm AS builder
WORKDIR /build
# Dependencies first, in their own layer: a code-only change reuses this layer from the CI cache
# instead of recompiling ~300 crates.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs && touch src/lib.rs \
    && cargo build --release --locked && rm -rf src
COPY src ./src
COPY migrations ./migrations
# The commit this image is built from; reported by GET /api/version as `build`.
ARG UG_BUILD_SHA=local
ENV UG_BUILD_SHA=$UG_BUILD_SHA
# touch: COPY keeps the source mtimes, which can be older than the placeholder build above.
RUN touch src/main.rs src/lib.rs && cargo build --release --locked \
    && cp target/release/underground-api /underground-api

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -u 10001 appuser
WORKDIR /app
COPY --from=builder /underground-api /app/underground-api
USER appuser
ENV PORT=8080
EXPOSE 8080
HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=5 \
    CMD curl -fsS "http://localhost:${PORT}/health" || exit 1
ENTRYPOINT ["/app/underground-api"]

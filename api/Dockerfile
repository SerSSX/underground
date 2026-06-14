# syntax=docker/dockerfile:1
# Multi-stage build for the Underground API.

FROM rust:1.96-slim-bookworm AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations
RUN cargo build --release && cp target/release/underground-api /underground-api

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

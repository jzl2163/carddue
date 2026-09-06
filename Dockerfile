# syntax=docker/dockerfile:1
FROM rust:1-bookworm AS backend
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY Cargo.toml rust-toolchain.toml ./
COPY backend ./backend
# The lockfile is copied separately so a missing release lock fails loudly.
COPY Cargo.lock ./Cargo.lock
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/usr/local/cargo/git cargo build --release --locked --bin carddue
RUN /build/target/release/carddue openapi > /build/openapi.json

FROM node:22-bookworm-slim AS frontend
WORKDIR /build/frontend
COPY frontend/package*.json ./
COPY frontend/svelte.config.js frontend/tsconfig.json frontend/vite.config.ts ./
COPY frontend/src ./src
COPY frontend/static ./static
RUN npm ci --no-audit --no-fund
COPY --from=backend /build/openapi.json ./openapi.json
COPY scripts/check-bundle.mjs /build/scripts/check-bundle.mjs
RUN npm run api:generate && npm run check && npm run build && npm run budget

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 curl tzdata && rm -rf /var/lib/apt/lists/* && useradd --system --uid 10001 --home-dir /app --create-home carddue
WORKDIR /app
COPY --from=backend --chown=10001:10001 /build/target/release/carddue /usr/local/bin/carddue
COPY --from=frontend --chown=10001:10001 /build/frontend/build /app/frontend
ENV APP_BIND=0.0.0.0:8080 FRONTEND_DIR=/app/frontend
USER 10001:10001
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 CMD curl --fail --silent http://127.0.0.1:8080/health/ready || exit 1
ENTRYPOINT ["carddue"]

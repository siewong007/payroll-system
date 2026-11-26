# ─── Stage 0: cargo-chef base ───
# cargo-chef caches the dependency build as its own layer keyed on the
# dependency graph, so application code changes don't trigger a full rebuild.
FROM rust:1-bookworm@sha256:75d26c23c24cc6ea1cdabf0714a6bff8fd7bf3248716043fd7ae86dac7f41b94 AS chef
RUN cargo install cargo-chef --version 0.1.78 --locked
WORKDIR /app

# ─── Stage 1: Plan the dependency recipe ───
FROM chef AS planner
COPY backend/Cargo.toml backend/Cargo.lock ./
COPY backend/src ./src
RUN cargo chef prepare --recipe-path recipe.json

# ─── Stage 2: Build ───
FROM chef AS builder
# Build (and cache) dependencies only — reused unless Cargo.lock changes.
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Build the real application. sqlx query!/query_as! macros are verified against
# the committed .sqlx offline cache (no database available during the build).
ENV SQLX_OFFLINE=true
COPY backend/Cargo.toml backend/Cargo.lock ./
COPY backend/src ./src
COPY backend/migrations ./migrations
COPY backend/.sqlx ./.sqlx
ARG GIT_SHA=dev
ENV APP_BUILD_SHA=${GIT_SHA}
RUN cargo build --release --bin payroll-system

# ─── Stage 3: Runtime ───
FROM debian:bookworm-slim@sha256:e67eeead736ff7bbe258ef088bff683635c561b1fcfefad0806c1df4ab49ce0e

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/payroll-system ./payroll-system

# Run as a non-root user with no login shell. The app writes uploads under /app/uploads at runtime.
RUN useradd --system --uid 10001 --home-dir /app --no-create-home --shell /usr/sbin/nologin appuser \
    && mkdir -p /app/uploads \
    && chown -R 10001:10001 /app \
    && chmod 0750 /app/uploads
USER 10001

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 \
    # Readiness, not liveness: the container must not receive traffic until
# migrations have run and the DB is reachable (plan item 35).
CMD curl -f http://localhost:8080/api/health/ready || exit 1

CMD ["./payroll-system"]

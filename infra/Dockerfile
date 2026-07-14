# ─── Stage 0: cargo-chef base ───
# cargo-chef caches the dependency build as its own layer keyed on the
# dependency graph, so application code changes don't trigger a full rebuild.
FROM rust:1-bookworm AS chef
RUN cargo install cargo-chef --locked
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
RUN cargo build --release

# ─── Stage 3: Runtime ───
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/payroll-system ./payroll-system

# Run as a non-root user. The app writes uploads under /app/uploads at runtime,
# so that directory must be owned by the runtime user.
RUN useradd --system --uid 10001 --home-dir /app appuser \
    && mkdir -p /app/uploads \
    && chown -R 10001:10001 /app
USER 10001

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 \
    CMD curl -f http://localhost:8080/api/health || exit 1

CMD ["./payroll-system"]

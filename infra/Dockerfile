# ─── Stage 1: Build ───
FROM rust:latest AS builder

WORKDIR /app

# Cache dependencies by building with dummy main
COPY backend/Cargo.toml backend/Cargo.lock* ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release 2>/dev/null || true
RUN rm -rf src

# Build the real application
COPY backend/src ./src
COPY backend/migrations ./migrations
RUN cargo build --release

# ─── Stage 2: Runtime ───
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/payroll-system ./payroll-system
COPY --from=builder /app/migrations ./migrations

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 \
    CMD curl -f http://localhost:8080/api/health || exit 1

CMD ["./payroll-system"]

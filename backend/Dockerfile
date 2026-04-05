FROM rust:1.88-slim AS builder
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations
ENV SQLX_OFFLINE=true
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/payroll-system /usr/local/bin/payroll-system
COPY --from=builder /app/migrations /app/migrations
WORKDIR /app
EXPOSE 8080
CMD ["payroll-system"]

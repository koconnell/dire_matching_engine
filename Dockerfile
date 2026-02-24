# Multi-stage: build the Rust binary, then run in a slim image.
FROM rust:1-bookworm AS builder
WORKDIR /app

# Cache dependencies (only rebuild when Cargo.toml/Cargo.lock change)
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
# Bench manifest entries require the bench file to exist; copy so cargo doesn't fail (we only build the bin)
COPY benches ./benches

# Build release binary (no separate lib copy needed for single crate)
RUN cargo build --release --bin dire_matching_engine

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/dire_matching_engine /usr/local/bin/

# Non-root user
RUN useradd -r -u 1000 app
USER app

EXPOSE 8080
ENV PORT=8080
ENTRYPOINT ["/usr/local/bin/dire_matching_engine"]

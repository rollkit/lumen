# syntax=docker/dockerfile:1

# Build stage
FROM rust:1.81-slim AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./
COPY bin/ bin/
COPY crates/ crates/

# Build the application
RUN cargo build --release --bin lumen

# Runtime stage
FROM gcr.io/distroless/cc-debian12

# Copy the binary from builder
COPY --from=builder /app/target/release/lumen /usr/local/bin/lumen

# Expose default ports
EXPOSE 8545 8546 30303 6060 9001

# Set the entrypoint
ENTRYPOINT ["/usr/local/bin/lumen"]
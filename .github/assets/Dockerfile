# syntax=docker/dockerfile:1
# CI-specific Dockerfile for E2E tests

FROM ubuntu:24.04

# Copy the pre-built binary
COPY dist/ev-reth /usr/local/bin/ev-reth

# Make it executable
RUN chmod +x /usr/local/bin/ev-reth

# Create a non-root user
RUN useradd -m -s /bin/bash ev-reth

# Switch to non-root user
USER ev-reth
WORKDIR /home/ev-reth

# Expose default ports
EXPOSE 8545 8546 30303 6060 9001

# Set the entrypoint
ENTRYPOINT ["/usr/local/bin/ev-reth"]

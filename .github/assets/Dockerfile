# syntax=docker/dockerfile:1
# CI-specific Dockerfile for E2E tests

FROM ubuntu:24.04

# Copy the pre-built binary
COPY dist/lumen /usr/local/bin/lumen

# Make it executable
RUN chmod +x /usr/local/bin/lumen

# Create a non-root user
RUN useradd -m -s /bin/bash lumen

# Switch to non-root user
USER lumen
WORKDIR /home/lumen

# Expose default ports
EXPOSE 8545 8546 30303 6060 9001

# Set the entrypoint
ENTRYPOINT ["/usr/local/bin/lumen"]
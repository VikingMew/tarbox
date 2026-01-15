# Multi-stage Dockerfile for Tarbox
# Produces a minimal runtime image with the compiled binary

# Build stage
FROM rust:1.92-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libfuse3-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /usr/src/tarbox

# Copy manifests
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./

# Create a dummy main.rs to build dependencies first (caching layer)
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    echo "pub fn dummy() {}" > src/lib.rs

# Build dependencies only (this layer will be cached)
RUN cargo build --release && \
    rm -rf src target/release/tarbox* target/release/deps/tarbox*

# Copy the actual source code
COPY . .

# Build the actual application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libfuse3-3 \
    fuse3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 -s /bin/bash tarbox

# Copy the binary from builder
COPY --from=builder /usr/src/tarbox/target/release/tarbox /usr/local/bin/tarbox

# Create mount point
RUN mkdir -p /mnt/tarbox && chown tarbox:tarbox /mnt/tarbox

# Copy example config
COPY config.toml.example /etc/tarbox/config.toml

# Switch to non-root user (can be overridden for FUSE mounting)
USER tarbox

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD tarbox --version || exit 1

# Default command
ENTRYPOINT ["/usr/local/bin/tarbox"]
CMD ["--help"]

# Labels
LABEL org.opencontainers.image.title="Tarbox" \
    org.opencontainers.image.description="PostgreSQL-based distributed filesystem for AI agents" \
    org.opencontainers.image.url="https://github.com/vikingmew/tarbox" \
    org.opencontainers.image.source="https://github.com/vikingmew/tarbox" \
    org.opencontainers.image.licenses="MPL-2.0"

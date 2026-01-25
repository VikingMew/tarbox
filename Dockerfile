# Multi-stage Dockerfile for Tarbox with cargo-chef optimization
# Produces a minimal runtime image with the compiled binary

# Chef stage - prepare recipe
FROM rust:1.92-bookworm AS chef
RUN cargo install cargo-chef
WORKDIR /usr/src/tarbox

# Planner stage - generate recipe.json
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Builder stage - build dependencies and application
FROM chef AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libfuse3-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Build dependencies only (this layer will be cached)
COPY --from=planner /usr/src/tarbox/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Copy source and build application
COPY . .
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

# Docker Build Optimization with cargo-chef

This document explains how Tarbox uses `cargo-chef` to optimize Docker build times.

## Overview

The Dockerfile uses a multi-stage build with `cargo-chef` to separate dependency compilation from application code compilation. This dramatically improves build times when only application code changes.

## Build Stages

### 1. Chef Stage (Base)
```dockerfile
FROM rust:1.92-bookworm AS chef
RUN cargo install cargo-chef
WORKDIR /usr/src/tarbox
```

Installs cargo-chef tool. This layer is rarely rebuilt.

### 2. Planner Stage
```dockerfile
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json
```

Analyzes `Cargo.toml` and `Cargo.lock` to generate `recipe.json` containing dependency information.

### 3. Builder Stage
```dockerfile
FROM chef AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y pkg-config libfuse3-dev

# Build dependencies only (cached layer)
COPY --from=planner /usr/src/tarbox/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Build application (only this layer rebuilds when code changes)
COPY . .
RUN cargo build --release
```

Two-phase build:
- **Phase 1** (`cargo chef cook`): Builds dependencies only. This layer is cached unless `Cargo.toml`/`Cargo.lock` changes.
- **Phase 2** (`cargo build`): Builds application code. Only rebuilds when source code changes.

### 4. Runtime Stage
```dockerfile
FROM debian:bookworm-slim
# ... minimal runtime image with only the binary
```

Produces a minimal production image (~100MB vs ~2GB builder image).

## Build Time Comparison

### Without cargo-chef
```bash
# First build: ~15 minutes
# Code change: ~15 minutes (rebuilds everything)
```

### With cargo-chef
```bash
# First build: ~15 minutes
# Code change: ~2 minutes (only rebuilds app, reuses cached dependencies)
# Dependency change: ~15 minutes (rebuilds dependencies + app)
```

## Usage

### Build Image
```bash
# Standard build
docker build -t tarbox:latest .

# Build with BuildKit for better caching
DOCKER_BUILDKIT=1 docker build -t tarbox:latest .

# Build with progress output
docker build --progress=plain -t tarbox:latest .
```

### Build with Docker Compose
```bash
docker-compose build tarbox-cli
```

### Verify Cache Effectiveness

Build twice and check timing:
```bash
# First build (cold cache)
time docker build -t tarbox:v1 .

# Modify source file
echo "// comment" >> src/main.rs

# Second build (warm cache)
time docker build -t tarbox:v2 .
```

The second build should be significantly faster (~2 minutes vs ~15 minutes).

## Cache Layers

The build generates these key cache layers:

1. **Rust base image** (rarely changes)
2. **cargo-chef installation** (rarely changes)
3. **System dependencies** (libfuse3-dev, etc.) (rarely changes)
4. **recipe.json** (changes when Cargo.toml/Cargo.lock changes)
5. **Compiled dependencies** (changes when dependencies change)
6. **Application code** (changes frequently)

Only layers 5 and 6 rebuild when code changes, making incremental builds very fast.

## Best Practices

### Do's
- ✅ Use BuildKit for better caching: `DOCKER_BUILDKIT=1 docker build`
- ✅ Keep Cargo.lock in version control
- ✅ Use multi-stage builds
- ✅ Minimize changes to Cargo.toml

### Don'ts
- ❌ Don't run `cargo update` unnecessarily
- ❌ Don't copy unnecessary files (use .dockerignore)
- ❌ Don't disable BuildKit cache

## Troubleshooting

### Cache not working
```bash
# Clear Docker build cache
docker builder prune

# Rebuild without cache
docker build --no-cache -t tarbox:latest .
```

### cargo-chef errors
```bash
# Verify cargo-chef version
docker run --rm rust:1.92-bookworm cargo install cargo-chef --version

# Check recipe.json generation
docker build --target planner -t tarbox-planner .
docker run --rm tarbox-planner cat recipe.json
```

### Build fails at dependency stage
```bash
# Build only dependency layer for debugging
docker build --target builder -t tarbox-builder .
```

## References

- [cargo-chef GitHub](https://github.com/LukeMathWalker/cargo-chef)
- [Docker BuildKit Documentation](https://docs.docker.com/build/buildkit/)
- [Multi-stage Builds Best Practices](https://docs.docker.com/build/building/multi-stage/)

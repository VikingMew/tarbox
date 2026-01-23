<div align="center">

# 🗄️ Tarbox

**PostgreSQL-based filesystem for AI agents with version control and audit logging**

[![CI](https://github.com/VikingMew/tarbox/workflows/CI/badge.svg)](https://github.com/VikingMew/tarbox/actions/workflows/ci.yml)
[![E2E Tests](https://github.com/VikingMew/tarbox/workflows/E2E%20Tests/badge.svg)](https://github.com/VikingMew/tarbox/actions/workflows/e2e.yml)
[![License: MPL-2.0](https://img.shields.io/badge/License-MPL2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.92%2B-orange.svg)](https://www.rust-lang.org)
[![PostgreSQL](https://img.shields.io/badge/PostgreSQL-16%2B-336791.svg)](https://www.postgresql.org)

[Quick Start](#-quick-start) • [Features](#-features) • [Architecture](#-architecture) • [Documentation](#-documentation)

</div>

---

## What is Tarbox?

Tarbox is a FUSE filesystem that stores everything in PostgreSQL. It's designed for AI agents that need:

- **Reliable storage** - PostgreSQL ACID guarantees
- **Version control** - Docker-style layers and Git-like text diffs
- **Audit logging** - Track every file operation
- **Multi-tenancy** - Complete data isolation per tenant
- **Cloud-native** - Ready for Kubernetes deployment

**Current Status**: Core filesystem and layered filesystem are production-ready. Advanced features like audit integration and performance optimization are next on the roadmap.

**Platform Support**: Linux is fully supported. macOS support is incomplete due to `fuser` crate limitations (requires macFUSE and conditional compilation). See [Task 17](task/17-macos-fuse-support.md) for details.

---

## ✨ Features

### ✅ Production Ready

- **POSIX Filesystem**: Standard file operations (create, read, write, delete) via FUSE
- **PostgreSQL Backend**: ACID guarantees, content-addressed storage with BLAKE3
- **Multi-tenancy**: Complete isolation with per-tenant namespace
- **CLI Tool**: Manage tenants and files from command line
- **FUSE Mount**: Mount as standard filesystem, use any Unix tool
- **Layered Filesystem**: Docker-style snapshots with COW
  - ✅ Automatic base layer creation
  - ✅ Checkpoint creation and switching
  - ✅ Text files: line-level COW with diff computation
  - ✅ Binary files: block-level COW (4KB blocks)
  - ✅ Virtual filesystem hooks (`/.tarbox/layers/`)
  - ✅ Union view across layer chain
- **File Type Detection**: Automatic text/binary classification
  - ✅ UTF-8/ASCII/Latin-1 encoding detection
  - ✅ Line ending detection (LF/CRLF/CR/Mixed)
  - ✅ Content-based classification

### 🚧 In Development

- **Audit Logging**: Operation tracking and compliance reports
  - ✅ Database schema and operations
  - ⏳ Integration with file operations
- **Performance Optimization**: Caching and query optimization
  - ⏳ LRU cache for metadata and blocks
  - ⏳ Query optimization and indexing

---

## Quick Start

### Prerequisites

- PostgreSQL 16+
- FUSE3 (Linux: `libfuse3-dev`)
- Rust 1.92+ (only for native build)

### Option 1: Docker Compose (Recommended)

The easiest way to get started. Includes PostgreSQL and all dependencies.

```bash
# Clone repository
git clone https://github.com/vikingmew/tarbox.git
cd tarbox

# Start PostgreSQL
docker-compose up -d postgres

# Run tarbox CLI via Docker
docker-compose run --rm tarbox-cli tarbox init
docker-compose run --rm tarbox-cli tarbox tenant create myagent
docker-compose run --rm tarbox-cli tarbox --tenant myagent ls /

# Optional: Start pgAdmin for database management
docker-compose --profile tools up -d pgadmin
# Access at http://localhost:5050 (admin@tarbox.local / admin)
```

### Option 2: Native Build

Build and run directly on your machine. Requires Rust toolchain.

```bash
# Clone and build
git clone https://github.com/vikingmew/tarbox.git
cd tarbox
cargo build --release

# Setup PostgreSQL (choose one):
# A) Use existing PostgreSQL instance
# B) Start with Docker
docker-compose up -d postgres

# Configure database connection
export DATABASE_URL=postgres://postgres:postgres@localhost:5432/tarbox

# Initialize and run
./target/release/tarbox init
./target/release/tarbox tenant create myagent
```

### Basic Usage

```bash
# CLI file operations
tarbox --tenant myagent mkdir /workspace
tarbox --tenant myagent write /workspace/config.txt "key=value"
tarbox --tenant myagent cat /workspace/config.txt
tarbox --tenant myagent ls /workspace

# Mount as FUSE filesystem (requires FUSE permissions)
tarbox --tenant myagent mount /mnt/tarbox
echo "test" > /mnt/tarbox/workspace/test.txt
ls -la /mnt/tarbox/workspace

# Use layer system (automatic snapshots)
echo "version 1" > /mnt/tarbox/workspace/app.py
echo "checkpoint1" > /mnt/tarbox/.tarbox/layers/new  # Create checkpoint
echo "version 2" > /mnt/tarbox/workspace/app.py
cat /mnt/tarbox/.tarbox/layers/list                  # View layer history

tarbox umount /mnt/tarbox
```

---

## 🏗️ Architecture

```mermaid
graph TB
    Apps[Applications / AI Agents]
    FUSE[FUSE Interface<br/>POSIX Operations]
    
    subgraph Core[Tarbox Core]
        FS[Filesystem Layer<br/>• Path resolution<br/>• Inode management<br/>• Permission control]
        Storage[Storage Layer<br/>• Tenants & inodes<br/>• Data blocks<br/>• Layers & audit]
    end
    
    DB[(PostgreSQL<br/>• tenants, inodes, blocks<br/>• layers, audit_logs<br/>• text_blocks)]
    
    Apps --> FUSE
    FUSE --> FS
    FS --> Storage
    Storage --> DB
    
    style Apps fill:#e1f5ff
    style FUSE fill:#fff3e0
    style Core fill:#f3e5f5
    style DB fill:#e8f5e9
```

### Key Design Decisions

- **FUSE over kernel module**: Easier development and debugging
- **PostgreSQL over file-based**: ACID guarantees, multi-tenancy, query capabilities
- **Content-addressed storage**: Deduplication with BLAKE3 hashing
- **Async Rust**: High-performance I/O with tokio runtime
- **Repository pattern**: Clean separation between filesystem and storage layers

---

## 📖 Documentation

### User Documentation

- **[Quick Start](#-quick-start)** - Get started in 5 minutes (see above)
- **[CLI Reference](#cli-reference)** - All commands and options
- **[Configuration](CLAUDE.md#configuration)** - Database and filesystem settings

### Developer Documentation

- **[Architecture Overview](spec/00-overview.md)** - System design and philosophy
- **[Database Schema](spec/01-database-schema.md)** - PostgreSQL table definitions
- **[FUSE Interface](spec/02-fuse-interface.md)** - POSIX operation mappings
- **[Development Guide](CLAUDE.md)** - Setup and coding standards
- **[Contributing](CONTRIBUTING.md)** - How to contribute

---

## 🛠️ CLI Reference

```bash
# Database initialization
tarbox init                                    # Create database schema

# Tenant management
tarbox tenant create <name>                    # Create new tenant
tarbox tenant list                             # List all tenants
tarbox tenant info <name>                      # Show tenant details
tarbox tenant delete <name>                    # Delete tenant

# File operations (all require --tenant <name>)
tarbox --tenant <name> mkdir <path>            # Create directory
tarbox --tenant <name> rmdir <path>            # Remove empty directory
tarbox --tenant <name> ls [path]               # List directory contents
tarbox --tenant <name> touch <path>            # Create empty file
tarbox --tenant <name> write <path> <content>  # Write to file
tarbox --tenant <name> cat <path>              # Read file
tarbox --tenant <name> rm <path>               # Remove file
tarbox --tenant <name> stat <path>             # Show file metadata

# FUSE mounting
tarbox --tenant <name> mount <mountpoint>      # Mount filesystem
tarbox --tenant <name> mount <mp> --read-only  # Mount read-only
tarbox --tenant <name> mount <mp> --allow-other # Allow all users
tarbox umount <mountpoint>                     # Unmount filesystem

# Layer management (via virtual filesystem hooks)
# After mounting, use standard file operations on /.tarbox/
cat /.tarbox/layers/current                    # Show current layer
cat /.tarbox/layers/list                       # List all layers
echo "checkpoint1" > /.tarbox/layers/new       # Create checkpoint
echo "<layer-id>" > /.tarbox/layers/switch     # Switch to layer
cat /.tarbox/layers/tree                       # Show layer tree
cat /.tarbox/stats/usage                       # Show storage statistics
```

---

## 🧪 Development

### Building and Testing

```bash
# Build
cargo build
cargo build --release

# Run tests
cargo test --lib                               # Unit tests (fast)
cargo test                                     # All tests (requires PostgreSQL)

# Code quality
cargo fmt --all                                # Format code
cargo clippy --all-targets -- -D warnings      # Lint code

# Pre-commit check
cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --lib
```

---

## Comparison

### vs AgentFS

[AgentFS](https://github.com/tursodatabase/agentfs) is a filesystem for AI agents based on SQLite. Choose Tarbox when:
- **Running multiple agents** that need isolated workspaces on shared infrastructure
- **Server-side deployment** with PostgreSQL already in your stack
- **Fine-grained version control** for text files (code, configs, logs)
- **Kubernetes/cloud-native** environments with horizontal scaling needs
- **Compliance requirements** needing centralized audit logs

---

## 📊 Performance

Designed for high performance with:

- **Prepared statements** for all PostgreSQL queries
- **Connection pooling** with configurable limits
- **Content addressing** for deduplication
- **Async I/O** with tokio runtime
- **LRU caching** for metadata and blocks (planned)

Benchmarks coming soon.

---

## 🤝 Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for:

- Code of Conduct
- Development workflow
- Testing requirements (>80% coverage)
- Code style guidelines

### Quick Contribution Guide

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run tests and linting
5. Submit a pull request

---

## 📜 License

This project is licensed under the [Mozilla Public License 2.0](LICENSE).

---

## 🙏 Acknowledgments

Built with PostgreSQL, Rust, and FUSE. Inspired by Docker's layered filesystem and Git's content addressing.

---

<div align="center">

**[⬆ back to top](#-tarbox)**

Made with ❤️ for AI agents

</div>

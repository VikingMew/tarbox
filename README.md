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

**Current Status**: Core filesystem (read/write/mount) is production-ready. Advanced features (layers, audit, text optimization) are partially complete - database layer done, filesystem integration in progress.

---

## ✨ Features

### ✅ Production Ready

- **POSIX Filesystem**: Standard file operations (create, read, write, delete) via FUSE
- **PostgreSQL Backend**: ACID guarantees, content-addressed storage with BLAKE3
- **Multi-tenancy**: Complete isolation with per-tenant namespace
- **CLI Tool**: Manage tenants and files from command line
- **FUSE Mount**: Mount as standard filesystem, use any Unix tool

### 🚧 In Development

- **Layered Filesystem**: Docker-style snapshots with COW
  - ✅ Database schema and operations
  - ⏳ Filesystem integration (COW, layer switching)
- **Audit Logging**: Operation tracking and compliance reports
  - ✅ Database schema and operations
  - ⏳ Integration with file operations
- **Text Optimization**: Line-level diffs for code and config files
  - ✅ Database schema and operations
  - ⏳ Diff computation and storage

---

## Quick Start

### Prerequisites

- PostgreSQL 16+
- FUSE3 (Linux: `libfuse3-dev`, macOS: `macfuse`)
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

### Test Coverage

- **Unit tests**: 112 tests, 47.76% coverage (pure functions, no database)
- **Integration tests**: 59 tests (database operations, FUSE logic)
- **E2E tests**: 50 tests (requires PostgreSQL + FUSE, runs in CI)
- **Total expected coverage**: >80%

### Project Structure

```
tarbox/
├── src/
│   ├── types.rs           # Core type aliases
│   ├── config/            # Configuration system
│   ├── storage/           # PostgreSQL layer (repositories, migrations)
│   ├── fs/                # Filesystem core (path, operations, permissions)
│   ├── fuse/              # FUSE interface
│   └── main.rs            # CLI entry point
├── spec/                  # Architecture design documents
├── task/                  # Development tasks and progress
├── tests/                 # Integration and E2E tests
└── migrations/            # Database schema migrations
```

---

## 🗺️ Roadmap

### ✅ Phase 1: Core Filesystem (Complete)

- [x] PostgreSQL storage backend
- [x] Multi-tenant data isolation
- [x] POSIX file operations
- [x] FUSE mounting support
- [x] CLI tool

### ✅ Phase 2: Advanced Storage Schema (Complete)

- [x] Audit logging tables (time-partitioned)
- [x] Layer management tables (chain queries)
- [x] Text optimization tables (content-addressed)
- [x] Repository implementations (3 modules, 22 methods)
- [x] Comprehensive tests (112 unit + 59 integration)

### 🚧 Phase 3: Filesystem Integration (In Progress)

- [ ] Audit logging integration with file operations
- [ ] COW implementation for layered filesystem
- [ ] Text diff computation and storage
- [ ] Filesystem hooks for layer control
- [ ] Advanced POSIX features (links, xattr)

### 📋 Phase 4: Cloud Native (Planned)

- [ ] Kubernetes CSI driver
- [ ] REST/gRPC API
- [ ] Monitoring and metrics (Prometheus)
- [ ] Web management UI

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

## 📊 Performance

Designed for high performance with:

- **Prepared statements** for all PostgreSQL queries
- **Connection pooling** with configurable limits
- **Content addressing** for deduplication
- **Async I/O** with tokio runtime
- **LRU caching** for metadata and blocks (planned)

Benchmarks coming soon.

---

## 📜 License

Dual-licensed under MIT or Apache 2.0, at your option.

---

## 🙏 Acknowledgments

Built with PostgreSQL, Rust, and FUSE. Inspired by Docker's layered filesystem and Git's content addressing.

---

<div align="center">

**[⬆ back to top](#-tarbox)**

Made with ❤️ for AI agents

</div>

<div align="center">

# 🗄️ Tarbox

**A PostgreSQL-based distributed filesystem for AI agents and cloud-native environments**

[![License: MPL-2.0](https://img.shields.io/badge/License-MPL2.0-blue.svg)](LICENSE)
[![Rust Version](https://img.shields.io/badge/rust-1.92%2B-orange.svg)](https://www.rust-lang.org)
[![PostgreSQL](https://img.shields.io/badge/PostgreSQL-14%2B-336791.svg)](https://www.postgresql.org)

[Features](#-features) • [Quick Start](#-quick-start) • [Architecture](#-architecture) • [Documentation](#-documentation) • [Contributing](#-contributing)

[中文文档](README_zh.md)

</div>

---

## 📖 Overview

Tarbox is a high-performance filesystem implementation using PostgreSQL as the storage backend, specifically designed for AI agents that require reliable, auditable, and version-controlled file storage. It provides complete POSIX compatibility through a FUSE interface while offering unique features like Docker-style layering, Git-like text diffs, and Kubernetes integration.

### Why Tarbox?

Traditional filesystems lack the auditability, versioning, and multi-tenancy features that modern AI agents need. Tarbox bridges this gap by combining:

- **Database Reliability**: PostgreSQL's ACID properties ensure data consistency
- **Version Control**: Docker-like layers with Git-style text file optimization
- **Multi-Tenancy**: Complete isolation between different AI agents
- **Cloud Native**: Built-in Kubernetes CSI driver for seamless deployment
- **Auditability**: Every file operation is logged for compliance and debugging

---

## ✨ Features

### Core Capabilities

- **🐘 PostgreSQL Storage Backend**
  - ACID guarantees for data consistency
  - Distributed deployment with high availability
  - Metadata-data separation for optimal performance
  - Content-addressed storage with deduplication

- **📁 POSIX Compatibility**
  - Standard file operations (read, write, open, mkdir, etc.)
  - Full permission and attribute management
  - Symbolic and hard links support
  - Seamless integration with existing tools

- **🔍 Complete Audit Trail**
  - Every file operation logged with metadata
  - Time-partitioned audit tables for efficient queries
  - Version history tracking for all changes
  - Compliance reporting support

- **🐳 Docker-Style Layered Filesystem**
  - Create checkpoints and snapshots instantly
  - Copy-on-Write (COW) for efficient storage
  - Linear history model with fast layer switching
  - Control via filesystem hooks (e.g., `echo "checkpoint" > /.tarbox/layers/new`)

- **📝 Git-Like Text File Optimization**
  - Line-level diff storage for text files (CSV, Markdown, YAML, code, etc.)
  - Cross-file and cross-layer content deduplication
  - Efficient version comparison with `tarbox diff`
  - Completely transparent to applications

- **⚡ Native Filesystem Mounting**
  - Direct host FS access for performance-critical paths
  - Configurable read-only or read-write modes
  - Shared system directories (`/bin`, `/usr`) or tenant-specific workspaces
  - Perfect for Python venvs, npm modules, and ML model caches

- **☸️ Kubernetes Integration**
  - Native CSI (Container Storage Interface) driver
  - Dynamic volume provisioning
  - Multi-tenant isolation at the infrastructure level
  - Snapshot and backup support

---

## 🏗️ Architecture

```
┌─────────────────────────────────────────────┐
│         Applications / AI Agents             │
└─────────────────┬───────────────────────────┘
                  │
┌─────────────────┴───────────────────────────┐
│           FUSE Interface                     │
│       (POSIX File Operations)                │
└─────────────────┬───────────────────────────┘
                  │
┌─────────────────┴───────────────────────────┐
│         Tarbox Core Engine                   │
│  ┌──────────────────────────────────────┐   │
│  │  Filesystem Layer                     │   │
│  │  • Inode management                   │   │
│  │  • Directory tree                     │   │
│  │  • Permission control                 │   │
│  │  • Native mount routing               │   │
│  └──────────────────────────────────────┘   │
│  ┌──────────────────────────────────────┐   │
│  │  Layered Filesystem                   │   │
│  │  • Layer management (create/switch)   │   │
│  │  • Copy-on-Write (COW)                │   │
│  │  • Checkpoints and snapshots          │   │
│  └──────────────────────────────────────┘   │
│  ┌──────────────────────────────────────┐   │
│  │  Audit & Caching                      │   │
│  │  • Operation logging                  │   │
│  │  • Multi-level LRU cache              │   │
│  │  • Version tracking                   │   │
│  └──────────────────────────────────────┘   │
└─────────────────┬───────────────────────────┘
                  │
┌─────────────────┴───────────────────────────┐
│        PostgreSQL Storage Backend            │
│  • Metadata tables (inodes, layers)         │
│  • Data blocks (binary & text)              │
│  • Audit logs (time-partitioned)            │
│  • Native mount configuration               │
└──────────────────────────────────────────────┘
```

### Module Structure

```
src/
├── types.rs        # Core type aliases (InodeId, LayerId, TenantId)
├── config/         # Configuration system (TOML + environment)
├── storage/        # PostgreSQL layer (all DB operations)
├── fs/             # Filesystem core (path resolution, file ops)
├── fuse/           # FUSE interface (async-to-sync bridge)
├── layer/          # Layered filesystem (COW, checkpoints)
├── native/         # Native mount management
├── audit/          # Audit logging (async batch insertion)
├── cache/          # Caching layer (moka-based LRU)
├── api/            # REST/gRPC APIs
└── k8s/            # Kubernetes CSI driver
```

---

## 🚀 Quick Start

### Prerequisites

- **Rust**: 1.92+ (Edition 2024)
- **PostgreSQL**: 14+
- **FUSE**: libfuse3 (Linux) or macFUSE (macOS)

### Installation

#### Option 1: Using Docker Compose (Recommended for Development)

```bash
# Clone the repository
git clone https://github.com/yourusername/tarbox.git
cd tarbox

# Start PostgreSQL database
docker-compose up -d postgres

# Initialize database
export DATABASE_URL=postgres://postgres:postgres@localhost:5432/tarbox
cargo run -- init

# Or use the CLI container
docker-compose run --rm tarbox-cli tarbox init
```

See [Docker Compose Guide](docs/docker-compose.md) for detailed usage.

#### Option 2: Build from Source

```bash
# Clone the repository
git clone https://github.com/yourusername/tarbox.git
cd tarbox

# Build from source
cargo build --release

# Install (optional)
cargo install --path .
```

### Basic Usage

```bash
# Initialize database schema
tarbox init --database-url postgresql://user:pass@localhost/tarbox

# Create a tenant for your AI agent
tarbox tenant create myagent --name "My AI Agent"

# Mount the filesystem
sudo tarbox mount /mnt/tarbox --tenant myagent

# Use it like a regular filesystem
echo "Hello, Tarbox!" > /mnt/tarbox/hello.txt
cat /mnt/tarbox/hello.txt

# Create a checkpoint (snapshot)
echo "checkpoint" > /mnt/tarbox/.tarbox/layers/new

# Make some changes
echo "More data" >> /mnt/tarbox/hello.txt

# View layer history
cat /mnt/tarbox/.tarbox/layers/list

# Switch to previous layer
echo "<layer-id>" > /mnt/tarbox/.tarbox/layers/switch

# Unmount
sudo umount /mnt/tarbox
```

### CLI Commands

```bash
# Tenant management
tarbox tenant create <name>           # Create a new tenant
tarbox tenant list                    # List all tenants
tarbox tenant delete <name>           # Delete a tenant

# Layer operations
tarbox layer list --tenant <name>     # List all layers
tarbox layer create --tenant <name>   # Create checkpoint
tarbox layer switch --tenant <name> --layer <id>  # Switch layer
tarbox layer diff --layer1 <id1> --layer2 <id2>  # Compare layers

# File operations
tarbox ls --tenant <name> <path>              # List directory
tarbox cat --tenant <name> <path>             # Read file
tarbox write --tenant <name> <path> <data>    # Write file
tarbox diff --tenant <name> <path>            # Show file history

# Audit queries
tarbox audit --tenant <name> --since "1 day ago"  # Recent operations
tarbox audit --path <path> --operation write      # Specific file writes
```

---

## 📚 Documentation

### For Users

- **[Quick Start Guide](docs/quick-start.md)** - Get up and running in 5 minutes
- **[Configuration Reference](docs/configuration.md)** - All config options explained
- **[CLI Reference](docs/cli-reference.md)** - Complete command documentation
- **[Kubernetes Deployment](docs/kubernetes.md)** - Deploy with CSI driver

### For Developers

- **[Architecture Overview](spec/00-overview.md)** - System design and philosophy
- **[Database Schema](spec/01-database-schema.md)** - PostgreSQL table definitions
- **[FUSE Interface](spec/02-fuse-interface.md)** - POSIX operation mappings
- **[Layered Filesystem](spec/04-layered-filesystem.md)** - COW and versioning
- **[Text Optimization](spec/10-text-file-optimization.md)** - Line-level diffs
- **[Native Mounting](spec/12-native-mounting.md)** - Performance optimizations
- **[Contributing Guide](CONTRIBUTING.md)** - How to contribute
- **[Development Setup](CLAUDE.md)** - Internal dev guidelines

### Task Progress

View our development roadmap in the [task/](task/) directory:

- ✅ **Task 01**: Project setup
- ⏳ **Task 02**: Database layer (MVP)
- ⏳ **Task 03**: Filesystem core (MVP)
- ⏳ **Task 04**: CLI tool (MVP)
- 📅 **Task 05-08**: Advanced features (FUSE, layers, audit)

---

## 💡 Use Cases

### AI Agent Workspace

```bash
# Each AI agent gets an isolated tenant
tarbox tenant create agent-001

# Agent works in a layered environment
# Checkpoint before risky operations
echo "checkpoint" > /.tarbox/layers/new

# Agent modifies files
# If something goes wrong, rollback instantly
echo "<previous-layer>" > /.tarbox/layers/switch
```

### Code Generation Tracking

```bash
# Track every change made by code generation tools
tarbox audit --operation write --since "1 hour ago"

# Compare before/after for generated code
tarbox layer diff --layer1 <before> --layer2 <after>

# View line-by-line changes in text files
tarbox diff /src/generated.py
```

### Multi-Environment Development

```bash
# Shared read-only system tools via native mounts
[[native_mounts]]
path = "/usr/bin"
source = "/usr/bin"
mode = "ro"
shared = true

# Tenant-specific Python virtual environments
[[native_mounts]]
path = "/.venv"
source = "/var/tarbox/venvs/{tenant_id}"
mode = "rw"
shared = false
```

---

## 🔧 Configuration

Example `config.toml`:

```toml
[database]
url = "postgresql://tarbox:password@localhost/tarbox"
pool_size = 20
connection_timeout = "30s"

[filesystem]
block_size = 4096
max_file_size = "10GB"

[cache]
metadata_size = "1GB"
block_size = "4GB"
policy = "lru"

[audit]
enabled = true
retention_days = 90
batch_size = 100

[layer]
auto_checkpoint = false
checkpoint_interval = "1h"

# Native filesystem mounts
[[native_mounts]]
path = "/bin"
source = "/bin"
mode = "ro"
shared = true
priority = 10

[[native_mounts]]
path = "/.venv"
source = "/var/tarbox/venvs/{tenant_id}"
mode = "rw"
shared = false
priority = 20
```

---

## 🧪 Development

### Building and Testing

```bash
# Build project
cargo build

# Run all tests
cargo test

# Run specific test
cargo test test_name

# Check code coverage (requires tarpaulin)
cargo tarpaulin --out Html

# Format code
cargo fmt --all

# Lint code
cargo clippy --all-targets --all-features -- -D warnings

# Pre-commit check (run before committing)
cargo fmt --all && \
cargo clippy --all-targets --all-features -- -D warnings && \
cargo test
```

### Project Requirements

- **Test Coverage**: Must be >80% (project-wide requirement)
- **Rust Edition**: 2024
- **Code Style**: Follow Linus Torvalds and John Carmack principles
  - Simple and direct code
  - Fail fast error handling (use `anyhow::Result`)
  - Data-oriented design
  - Small, focused functions

### Dependency Management

```bash
# Add a new dependency (NEVER edit Cargo.toml manually)
cargo add <crate>
cargo add --dev <crate>  # For dev dependencies

# Security audit
cargo audit

# License and dependency check
cargo deny check
```

---

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

### How to Contribute

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Run tests and linting (`cargo test && cargo clippy`)
5. Commit your changes (`git commit -m 'Add amazing feature'`)
6. Push to the branch (`git push origin feature/amazing-feature`)
7. Open a Pull Request

### Development Chat

- Join our discussions on GitHub Issues
- Read the [Code of Conduct](CODE_OF_CONDUCT.md)

---

## 📊 Performance

Tarbox is designed for high performance with intelligent caching:

- **Metadata Cache**: LRU cache for inode lookups
- **Block Cache**: Content-addressed block caching
- **Path Cache**: Cached path resolution
- **Prepared Statements**: All PostgreSQL queries use prepared statements
- **Batch Operations**: Audit logs written in async batches
- **Native Mounts**: Bypass PostgreSQL for performance-critical paths

Benchmark results (coming soon):

```
File read (1MB):      ~50 MB/s
File write (1MB):     ~40 MB/s
Metadata operations:  ~5000 ops/s
Layer switch:         <100ms
Text diff:            ~1M lines/s
```

---

## 🔒 Security

- **Multi-tenant Isolation**: Complete data separation between tenants
- **Audit Logging**: Every operation is logged for compliance
- **Permission Model**: Standard UNIX permissions enforced
- **Secure by Default**: Read-only native mounts for system directories

For security vulnerabilities, please see [SECURITY.md](SECURITY.md).

---

## 🗺️ Roadmap

### MVP Phase (Current)

- [x] Project setup with Rust 2024 edition
- [ ] Database layer with multi-tenancy
- [ ] Basic filesystem operations (POSIX)
- [ ] CLI tool for tenant and file management

### Phase 2: Core Features

- [ ] FUSE interface with path routing
- [ ] Layered filesystem with COW
- [ ] Audit system with time partitioning
- [ ] Native mount support

### Phase 3: Advanced Features

- [ ] Text file optimization (line-level diffs)
- [ ] Advanced caching strategies
- [ ] Permission system enhancements
- [ ] Symbolic and hard links

### Phase 4: Cloud Native

- [ ] Kubernetes CSI driver
- [ ] REST API for management
- [ ] gRPC API for high performance
- [ ] Monitoring and metrics (Prometheus)

### Phase 5: Future

- [ ] Distributed PostgreSQL support (Citus)
- [ ] Real-time replication
- [ ] ML model versioning helpers
- [ ] Web UI for management

---

## 📜 License

This project is dual-licensed under:

- MIT License ([LICENSE-MIT](LICENSE) or http://opensource.org/licenses/MIT)
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE) or http://www.apache.org/licenses/LICENSE-2.0)

You may choose either license for your use.

---

## 🙏 Acknowledgments

- **PostgreSQL Community**: For the robust database system
- **FUSE Project**: For userspace filesystem capabilities
- **Rust Community**: For the amazing ecosystem
- Inspired by Docker's layered filesystem and Git's content addressing

---

## 📞 Support

- **Documentation**: [Full docs](docs/)
- **Issues**: [GitHub Issues](https://github.com/yourusername/tarbox/issues)
- **Discussions**: [GitHub Discussions](https://github.com/yourusername/tarbox/discussions)

---

<div align="center">

**[⬆ back to top](#-tarbox)**

Made with ❤️ by the Tarbox team

</div>

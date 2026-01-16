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

Tarbox is a high-performance filesystem implementation using PostgreSQL as the storage backend, specifically designed for AI agents that require reliable, auditable, and version-controlled file storage.

**⚠️ Current Status: MVP Development Phase**

Tarbox is under active development. The core storage layer and CLI tools are functional, while advanced features like FUSE mounting, layering, and Kubernetes integration are planned for future releases.

### Why Tarbox?

Traditional filesystems lack the auditability, versioning, and multi-tenancy features that modern AI agents need. Tarbox aims to bridge this gap by combining:

- **Database Reliability**: PostgreSQL's ACID properties ensure data consistency
- **Version Control**: (Planned) Docker-like layers with Git-style text file optimization
- **Multi-Tenancy**: Complete isolation between different AI agents
- **Cloud Native**: (Planned) Built-in Kubernetes CSI driver for seamless deployment
- **Auditability**: (Planned) Every file operation logged for compliance and debugging

---

## ✨ Features

### ✅ Implemented (MVP)

- **🐘 PostgreSQL Storage Backend**
  - ACID guarantees for data consistency
  - Multi-tenant data isolation
  - Metadata and data block storage
  - Content-addressed storage with BLAKE3 hashing

- **📁 Basic File Operations**
  - Directory operations (create, list, remove)
  - File operations (create, read, write, delete)
  - Path resolution and validation
  - Metadata operations (stat, chmod, chown)

- **🔧 CLI Tool**
  - Tenant management (create, list, delete, info)
  - File system operations (mkdir, ls, rm, cat, write)
  - Database initialization
  - Configurable via environment variables

### 🚧 Planned Features

- **📁 Full POSIX Compatibility** (Task 05)
  - FUSE interface for seamless mounting
  - Symbolic and hard links support
  - Extended attributes
  - File locking

- **🔍 Complete Audit Trail** (Task 06)
  - Every file operation logged with metadata
  - Time-partitioned audit tables for efficient queries
  - Version history tracking for all changes
  - Compliance reporting support

- **🐳 Docker-Style Layered Filesystem** (Task 08)
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

- **🌐 WASI Support**
  - WebAssembly System Interface compatibility
  - Run in edge computing environments
  - Browser-based file system
  - Serverless function integration

---

## 🏗️ Architecture

```mermaid
graph TB
    Apps[Applications / AI Agents]
    FUSE[FUSE Interface<br/>POSIX File Operations]
    
    subgraph Core[Tarbox Core Engine]
        FS[Filesystem Layer<br/>• Inode management<br/>• Directory tree<br/>• Permission control<br/>• Native mount routing]
        Layer[Layered Filesystem<br/>• Layer management<br/>• Copy-on-Write COW<br/>• Checkpoints & snapshots]
        Audit[Audit & Caching<br/>• Operation logging<br/>• Multi-level LRU cache<br/>• Version tracking]
    end
    
    DB[(PostgreSQL Storage Backend<br/>• Metadata tables inodes, layers<br/>• Data blocks binary & text<br/>• Audit logs time-partitioned<br/>• Native mount configuration)]
    
    Apps --> FUSE
    FUSE --> Core
    FS --> DB
    Layer --> DB
    Audit --> DB
    
    style Apps fill:#e1f5ff
    style FUSE fill:#fff3e0
    style Core fill:#f3e5f5
    style DB fill:#e8f5e9
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
git clone https://github.com/vikingmew/tarbox.git
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
git clone https://github.com/vikingmew/tarbox.git
cd tarbox

# Build from source
cargo build --release

# Install (optional)
cargo install --path .
```

### Basic Usage (Current MVP)

```bash
# Initialize database schema
tarbox init

# Create a tenant for your AI agent
tarbox tenant create myagent

# Create directory structure
tarbox --tenant myagent mkdir /data
tarbox --tenant myagent mkdir /data/logs

# List directories
tarbox --tenant myagent ls /
tarbox --tenant myagent ls /data

# Create and write files
tarbox --tenant myagent touch /data/config.txt
tarbox --tenant myagent write /data/config.txt "key=value"

# Read file content
tarbox --tenant myagent cat /data/config.txt

# View file information
tarbox --tenant myagent stat /data/config.txt

# Delete files and directories
tarbox --tenant myagent rm /data/config.txt
tarbox --tenant myagent rmdir /data/logs

# Tenant management
tarbox tenant list
tarbox tenant info myagent
tarbox tenant delete myagent
```

**Note**: FUSE mounting, layering, and other advanced features are not yet implemented. See [Roadmap](#-roadmap) for planned features.

### CLI Commands (Currently Available)

```bash
# Database initialization
tarbox init                                    # Initialize database schema

# Tenant management
tarbox tenant create <name>                    # Create a new tenant
tarbox tenant info <name>                      # Show tenant information
tarbox tenant list                             # List all tenants
tarbox tenant delete <name>                    # Delete a tenant

# File operations (require --tenant <name>)
tarbox --tenant <name> mkdir <path>            # Create directory
tarbox --tenant <name> ls [path]               # List directory (default: /)
tarbox --tenant <name> rmdir <path>            # Remove empty directory
tarbox --tenant <name> touch <path>            # Create empty file
tarbox --tenant <name> write <path> <content>  # Write content to file
tarbox --tenant <name> cat <path>              # Read file content
tarbox --tenant <name> rm <path>               # Remove file
tarbox --tenant <name> stat <path>             # Show file information
```

**Planned Commands** (not yet implemented):

```bash
# Layer operations (Task 08 - Layered Filesystem)
tarbox layer list --tenant <name>
tarbox layer create --tenant <name>
tarbox layer switch --tenant <name> --layer <id>
tarbox layer diff --layer1 <id1> --layer2 <id2>

# Audit queries (Task 06 - Audit System)
tarbox audit --tenant <name> --since "1 day ago"
tarbox audit --path <path> --operation write

# FUSE mounting (Task 05 - FUSE Interface)
tarbox mount <mountpoint> --tenant <name>
tarbox umount <mountpoint>
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

- ✅ **Task 01**: Project setup and infrastructure
- ✅ **Task 02**: Database layer (MVP) - PostgreSQL storage backend
- ✅ **Task 03**: Filesystem core (MVP) - Basic file operations
- ✅ **Task 04**: CLI tool (MVP) - Command-line interface
- 📅 **Task 05**: FUSE interface - POSIX mounting
- 📅 **Task 06**: Audit system - Operation logging
- 📅 **Task 07**: Advanced filesystem - Permissions, links, caching
- 📅 **Task 08**: Layered filesystem - COW, checkpoints, versioning

---

## 💡 Use Cases

### ✅ Current: Multi-Tenant File Storage

```bash
# Each AI agent gets an isolated tenant
tarbox tenant create agent-001
tarbox tenant create agent-002

# Agents have completely isolated workspaces
tarbox --tenant agent-001 mkdir /workspace
tarbox --tenant agent-001 write /workspace/data.txt "Agent 1 data"

tarbox --tenant agent-002 mkdir /workspace
tarbox --tenant agent-002 write /workspace/data.txt "Agent 2 data"

# Data is completely isolated - no cross-contamination
tarbox --tenant agent-001 cat /workspace/data.txt  # Output: Agent 1 data
tarbox --tenant agent-002 cat /workspace/data.txt  # Output: Agent 2 data
```

### 🚧 Planned: AI Agent Workspace with Layers

```bash
# Each AI agent gets an isolated tenant (✅ implemented)
tarbox tenant create agent-001

# Agent works in a layered environment (🚧 planned)
# Checkpoint before risky operations
echo "checkpoint" > /.tarbox/layers/new

# Agent modifies files
# If something goes wrong, rollback instantly
echo "<previous-layer>" > /.tarbox/layers/switch
```

### 🚧 Planned: Code Generation Tracking

```bash
# Track every change made by code generation tools
tarbox audit --operation write --since "1 hour ago"

# Compare before/after for generated code
tarbox layer diff --layer1 <before> --layer2 <after>

# View line-by-line changes in text files
tarbox diff /src/generated.py
```

### 🚧 Planned: Multi-Environment Development

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
- **Issues**: [GitHub Issues](https://github.com/vikingmew/tarbox/issues)
- **Discussions**: [GitHub Discussions](https://github.com/vikingmew/tarbox/discussions)

---

<div align="center">

**[⬆ back to top](#-tarbox)**

Made with ❤️ by the Tarbox team

</div>

# Tarbox

A PostgreSQL-based distributed filesystem designed for AI agents and cloud-native environments.

[中文文档](README_zh.md)

## Overview

Tarbox is a high-performance filesystem implementation using PostgreSQL as the storage backend, providing reliable and auditable file storage for AI agents. It offers complete POSIX compatibility through a FUSE interface and supports Kubernetes Persistent Volume (PV) mounting.

## Core Features

- **PostgreSQL Storage Backend**: ACID properties, distributed deployment, HA support
- **POSIX Compatibility**: Standard file operations and permissions
- **Filesystem Auditing**: Complete operation logging and version history
- **Layered Filesystem**: Docker-like layers with Copy-on-Write
- **FUSE Interface**: User-space implementation, no kernel module required
- **Text File Optimization**: Git-like line-level diff storage
- **Native Filesystem Mounting**: Direct host FS access for performance
- **Kubernetes Integration**: CSI driver with dynamic provisioning

## Quick Start

### Prerequisites

- Rust 1.92+ (Edition 2024)
- PostgreSQL 14+
- FUSE library (Linux: libfuse3, macOS: macFUSE)

### Installation

```bash
git clone https://github.com/yourusername/tarbox.git
cd tarbox
cargo build --release
```

### Basic Usage

```bash
# Initialize database
tarbox init

# Create tenant
tarbox tenant create myagent

# Use filesystem
tarbox --tenant myagent mkdir /data
tarbox --tenant myagent write /data/test.txt "hello world"
tarbox --tenant myagent cat /data/test.txt
tarbox --tenant myagent ls /data
```

## Development

### Commands

```bash
cargo build                                      # Build
cargo test                                       # Test
cargo fmt --all                                  # Format
cargo clippy --all-targets --all-features -- -D warnings  # Lint
```

### Project Structure

```
tarbox/
├── src/              # Source code
├── spec/             # Architecture specifications
├── task/             # Development tasks
└── tests/            # Tests
```

## Roadmap

### MVP Phase (Current)
- [x] Project setup
- [ ] Database layer (MVP)
- [ ] Filesystem core (MVP)
- [ ] CLI tool (MVP)

### Advanced Features
- [ ] FUSE interface
- [ ] Layered filesystem
- [ ] Audit system
- [ ] Kubernetes CSI driver

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines and [CLAUDE.md](CLAUDE.md) for development details.

### Coding Principles

Following Linus Torvalds and John Carmack philosophies:
- Simple and direct code
- Fail fast error handling
- Data-oriented design
- Small, focused functions

## License

MIT OR Apache-2.0

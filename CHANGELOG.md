# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### In Progress

- Database layer (MVP) implementation
- Filesystem core (MVP) implementation
- CLI tool (MVP) implementation

## [0.1.0] - 2024-01-15

### Added

#### Project Setup
- Initial project structure with Rust 2024 edition
- Cargo workspace configuration
- Core module organization (storage, fs, fuse, layer, native, audit, cache, api, k8s)
- Type system foundation (InodeId, LayerId, TenantId, BlockId)
- Development tooling setup (rustfmt, clippy)
- CI/CD configuration placeholder

#### Documentation
- Comprehensive architecture specifications (13 specs):
  - 00: System overview and design philosophy
  - 01: Database schema design
  - 02: FUSE interface specifications
  - 03: Audit system design
  - 04: Layered filesystem architecture
  - 05: Kubernetes CSI driver specs
  - 06: API design (REST/gRPC)
  - 07: Performance optimization strategies
  - 08: Filesystem hooks design
  - 09: Multi-tenancy implementation
  - 10: Text file optimization
  - 11: Dependency management
  - 12: Native mounting system
- Development task roadmap (9 tasks)
- Contributing guidelines (CONTRIBUTING.md)
- Code of conduct (CODE_OF_CONDUCT.md)
- Development guide for Claude AI (CLAUDE.md)
- README in English and Chinese

#### Dependencies
- Core dependencies configured:
  - `sqlx` for PostgreSQL with async support
  - `tokio` for async runtime
  - `fuser` for FUSE interface
  - `anyhow` for error handling
  - `uuid` for ID generation
  - `blake3` and `sha2` for hashing
  - `moka` for LRU caching
  - `similar` for text diffing

#### Build System
- Rust toolchain configuration (1.92+)
- Example configuration file (config.toml.example)
- Cargo manifest with dependency versions
- Test coverage target set to >80%

### Architecture Decisions

- **Storage Backend**: PostgreSQL chosen for ACID properties and distributed deployment
- **Multi-Tenancy**: Complete tenant isolation at database query level
- **Layered Model**: Linear history (no branching) for simplicity
- **Text Optimization**: Line-level diffs using Git-like content addressing
- **Native Mounts**: Direct host FS access for performance-critical paths
- **FUSE Interface**: User-space implementation for easy deployment
- **Caching Strategy**: Multi-level LRU caching (metadata, blocks, paths)

### Planned (MVP Phase)

#### Database Layer (Task 02)
- [ ] PostgreSQL connection pool management
- [ ] Multi-tenant schema implementation
- [ ] Core tables: tenants, inodes, data_blocks
- [ ] Basic CRUD operations
- [ ] Prepared statement management

#### Filesystem Core (Task 03)
- [ ] Path resolution engine
- [ ] Directory operations (mkdir, rmdir, readdir)
- [ ] File operations (create, read, write, delete)
- [ ] Metadata operations (stat, chmod, chown)
- [ ] Error handling and validation

#### CLI Tool (Task 04)
- [ ] Tenant management commands
- [ ] File operation commands
- [ ] Layer operation commands
- [ ] Audit query commands
- [ ] Output formatting (JSON, table, plain text)

## [0.2.0] - Planned

### Planned Features

- FUSE interface with path routing
- Native mount support
- Basic caching layer
- Integration tests

## [0.3.0] - Planned

### Planned Features

- Layered filesystem with Copy-on-Write
- Checkpoint creation and switching
- Layer merging
- Filesystem hooks (/.tarbox/)

## [0.4.0] - Planned

### Planned Features

- Audit system with time partitioning
- Async batch logging
- Audit query API
- Retention policy management

## [0.5.0] - Planned

### Planned Features

- Text file optimization
- Line-level diff storage
- Content deduplication across files and layers
- Text file version comparison

## [1.0.0] - Planned

### Planned Features

- Production-ready release
- Full POSIX compatibility
- Performance optimizations
- Security audit completion
- Comprehensive documentation
- Kubernetes CSI driver
- REST and gRPC APIs
- Web UI for management

---

## Version History Summary

- **0.1.0** - Project setup and architecture design ✅
- **0.2.0** - MVP: Core functionality (database, filesystem, CLI)
- **0.3.0** - Layered filesystem
- **0.4.0** - Audit system
- **0.5.0** - Text optimization
- **1.0.0** - Production ready

---

## Notes on Versioning

- **Major version (X.0.0)**: Breaking API changes or major architectural changes
- **Minor version (0.X.0)**: New features, backwards compatible
- **Patch version (0.0.X)**: Bug fixes, backwards compatible

During development (0.x.x versions), minor version increments may include breaking changes.

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for how to contribute to this changelog.

When adding entries:
- Use present tense ("Add feature" not "Added feature")
- Reference issue numbers where applicable
- Group changes by type (Added, Changed, Deprecated, Removed, Fixed, Security)
- Keep entries concise but descriptive

[Unreleased]: https://github.com/yourusername/tarbox/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/yourusername/tarbox/releases/tag/v0.1.0

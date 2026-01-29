# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Tarbox is a PostgreSQL-based filesystem for AI agents with POSIX compatibility via FUSE. Key characteristics:
- Docker-like layered filesystem (linear history, not branching)
- Git-like text file storage (line-level diffs)
- Multi-tenant with complete isolation
- Write-time-copy (COW) for both text and binary files
- Filesystem composition: combine host directories, shared layers, and working layers
- Filesystem hooks for layer control (e.g., `echo "save" > /.tarbox/layers/new`)

## Commands

### Development
```bash
cargo build                                      # Build project
cargo build --release                            # Build release
cargo test                                       # Run all tests
cargo test <name>                                # Run specific test
cargo fmt --all                                  # Format code
cargo clippy --all-targets --all-features -- -D warnings  # Lint
```

### Git Hooks Setup
```bash
./scripts/install-hooks.sh    # Install pre-commit hooks (fmt + clippy)
git commit --no-verify         # Bypass hooks (not recommended)
```

The pre-commit hook automatically runs `cargo fmt --check` and `cargo clippy` before each commit to ensure code quality.

### Pre-commit Check
```bash
cargo fmt --all && \
cargo clippy --all-targets --all-features -- -D warnings && \
cargo test
```

### Dependencies
```bash
cargo add <crate>              # Add dependency (NEVER edit Cargo.toml manually)
cargo audit                    # Security audit
cargo deny check               # License/dependency check
```

### Requirements
- Rust 1.92+ (Edition 2024)
- PostgreSQL 16+
- Test coverage > 80%

## Architecture

### Module Structure
```
src/
├── types.rs        # Core type aliases (InodeId, LayerId, TenantId, BlockId)
├── config/         # Config system (TOML files + env vars)
├── storage/        # PostgreSQL layer (all DB operations)
├── fs/             # Filesystem core (path resolution, file ops, permissions)
├── fuse/           # FUSE interface (bridges async to sync, path routing)
├── layer/          # Layered filesystem (COW, checkpoints, layer switching)
├── audit/          # Audit logging (async batch insertion, partitioned tables)
├── cache/          # Caching layer (moka-based LRU)
├── api/            # REST/gRPC APIs
└── k8s/            # Kubernetes CSI driver
```

### Key Design Patterns

**Tenant Isolation**: Every database query MUST include `tenant_id` in WHERE clause.

**Layered Storage**: Layers form a linear linked list (parent_layer_id). Each layer tracks file changes (add/modify/delete markers). COW for both text and binary files.

**Text vs Binary Files**: Auto-detected by UTF-8 validation. Text files use line-level diffs with content hashing. Binary files use block-level storage with blake3/sha256.

**Filesystem Composition** (spec/18): Tenant 可以组合多个来源的文件系统：
- **MountSource**: Host（宿主机目录）、Layer（其他 Tenant 的层）、WorkingLayer（当前可写层）
- **MountMode**: ReadOnly、ReadWrite、CopyOnWrite
- **约束**: 挂载路径不可嵌套、不可冲突，支持单文件挂载
- **共享层**: 可以将只读层发布为共享层，供其他 Tenant 挂载

**Filesystem Hooks**: Virtual `/.tarbox/` directory for layer control (not stored in database).

## Coding Principles

### Linus Torvalds + John Carmack Philosophy
- **Fail fast**: Use `anyhow::Result`, let errors bubble up naturally
- **No error hierarchies**: Don't define custom error enums, just use anyhow
- **Simple over clever**: Avoid traits unless polymorphism is needed
- **Data-first**: Design data structures before writing functions
- **Small functions**: Single responsibility, easy to test
- **Explicit**: No magic, make behavior clear

### Specific Rules
- Use cargo commands for dependencies (NEVER edit Cargo.toml/Cargo.lock manually)
- Comments explain "why", not "what"
- All async with tokio runtime
- Use prepared statements for repeated queries

### Documentation Rules
- **NEVER create new document types** (no SUMMARY.md, STATUS.md, REPORT.md, etc.)
- **Only three documentation locations**:
  - `doc/` - External/user-facing documentation
  - `spec/` - Architecture design documents
  - `task/` - Task descriptions and progress tracking
- All three have templates - follow them strictly
- Task status belongs in `task/*.md`, NOT in separate files
- Architecture belongs in `spec/*.md`, NOT in separate files

## Critical Implementation Details

### Multi-tenancy
Every database operation:
```rust
sqlx::query!("SELECT * FROM table WHERE tenant_id = $1 AND ...", tenant_id)
```
Never query without tenant_id. Tenant is UUID type.

### Layer Operations
- Layers use UUID primary keys
- Layer chain traversal: follow parent_layer_id until NULL
- Delete markers: layer_entries.is_deleted = true
- Reference counting for data_blocks and text_blocks

### Text File Storage
- TextBlock: single line with content_hash
- TextLineMap: file + line_number → text_block_id
- On modification: compute diff, create new TextBlocks for changed lines
- Unchanged lines reuse existing TextBlocks across files and layers

### Filesystem Composition
- 挂载路径不可嵌套、不可冲突
- 文件挂载（is_file=true）：精确匹配
- 目录挂载（is_file=false）：前缀匹配
- MountMode: ro（只读）、rw（读写）、cow（写时复制到 WorkingLayer）

## Project Structure

- `spec/` - Architecture design documents (00-18)
  - 核心: overview, database, FUSE, audit, layers, K8s, API
  - 特性: performance, hooks, multi-tenancy, text optimization
  - 高级: WASI, filesystem interface, testing, **filesystem-composition (18)**
- `task/` - Development tasks (MVP: 00-04, Advanced: 05-08)

## Testing Requirements

**Coverage > 80%** using layered approach:
1. **Unit tests (~55%)**: Pure functions in `#[cfg(test)]` modules
2. **Integration tests (~25-30%)**: Use mockall for database mocking in `tests/*_integration_test.rs`
3. **E2E tests (optional)**: Real PostgreSQL in `tests/*_e2e_test.rs`

```bash
cargo test --lib                    # Unit tests
cargo test                          # Unit + integration
cargo llvm-cov --all-targets --html # Coverage report
```

## Important Constraints

- Layer model is LINEAR (no branches)
- Paths: 4096 bytes total, 255 bytes per component
- Text file detection: valid UTF-8 → text, otherwise binary
- Inode IDs are i64, everything else (tenant, layer, block) is UUID
- FUSE is synchronous, bridge to async with block_on or spawn_blocking

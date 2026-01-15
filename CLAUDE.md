# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Tarbox is a PostgreSQL-based filesystem for AI agents with POSIX compatibility via FUSE. Key characteristics:
- Docker-like layered filesystem (linear history, not branching)
- Git-like text file storage (line-level diffs)
- Multi-tenant with complete isolation
- Write-time-copy (COW) for both text and binary files
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
- PostgreSQL 14+
- Test coverage must be > 80% (project-wide requirement)

## Architecture

### Module Structure
```
src/
├── types.rs        # Core type aliases (InodeId, LayerId, TenantId, BlockId)
├── config/         # Config system (TOML files + env vars, native_mounts config)
├── storage/        # PostgreSQL layer (all DB operations)
├── fs/             # Filesystem core (path resolution, file ops, permissions)
├── fuse/           # FUSE interface (bridges async to sync, path routing)
├── layer/          # Layered filesystem (COW, checkpoints, layer switching)
├── native/         # Native mount management (path matching, passthrough)
├── audit/          # Audit logging (async batch insertion, partitioned tables)
├── cache/          # Caching layer (moka-based LRU)
├── api/            # REST/gRPC APIs
└── k8s/            # Kubernetes CSI driver
```

### Key Design Patterns

**Tenant Isolation**: Every database query MUST include `tenant_id` in WHERE clause. Tenant context is established at mount time and flows through all operations.

**Layered Storage Model**:
- Layers form a single-direction linked list (parent_layer_id)
- Switching to historical layer doesn't delete future layers
- Creating new layer on historical layer requires confirmation, then deletes future layers
- Each layer has entries tracking file changes (add/modify/delete markers)

**Text vs Binary Files**:
- Files are auto-detected by UTF-8 validation
- Text files: stored as line blocks with content hashing, use `similar` crate for diffs
- Binary files: stored as data blocks with blake3/sha256 hashing
- Both use content addressing for deduplication

**Filesystem Hooks**: Virtual `/.tarbox/` directory provides layer control:
- Not stored in database
- Writing commands triggers actions (e.g., create layer, switch layer)
- Reading returns current state/results

**Native Mounts**: Certain paths can be mounted to host native filesystem:
- Configured in config.toml or native_mounts table
- Supports ro (read-only) and rw (read-write) modes
- Can be shared across tenants (e.g., /bin, /usr) or tenant-specific (e.g., /.venv)
- Path variables: {tenant_id}, {mount_id}, {user}
- Operations are passed through to native FS, bypassing PostgreSQL

### Data Flow Examples

**File Read (with Native Mount Check)**:
1. FUSE read() receives path and tenant_id
2. Check native_mounts config for path match
3. If matched:
   - Validate mode (must allow read)
   - Resolve source path (replace {tenant_id})
   - Pass through to native FS
   - Log to audit_log with is_native_mount=true
4. If not matched:
   - Normal path resolution (PostgreSQL)
   - Read from data_blocks or text_blocks
   - Return data

**File Write**:
1. FUSE write() → check native mount first
2. If native mount (mode=rw): pass through to native FS
3. Otherwise: fs/write_file()
4. Detect file type (text vs binary)
5. If text: compute diff from parent layer, create text_blocks, update text_line_map
6. If binary: create data_blocks with content hash
7. Update inode metadata
8. Create layer_entry if in layered mode
9. Async write audit_log

**Layer Switch**:
1. User writes layer_id to `/.tarbox/layers/switch`
2. Validate layer exists and belongs to tenant
3. Update tenant's current_layer_id
4. Clear all caches
5. File reads now traverse from new current layer up the chain

**Path Resolution**:
1. Start from root inode
2. For each path component, query inodes WHERE parent_id = current AND name = component AND tenant_id = X
3. Check layer chain: query current layer first, then parent layers
4. Cache resolved paths in LRU

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
- Specs are design docs, NOT code (no code examples in spec/*.md)
- Comments explain "why", not "what"
- All async with tokio runtime
- Use prepared statements for repeated queries

## Critical Implementation Details

### Multi-tenancy
Every database operation pattern:
```rust
sqlx::query!("SELECT * FROM table WHERE tenant_id = $1 AND ...", tenant_id)
```
Never query without tenant_id. Tenant is UUID type.

### Layer Operations
- Layers use UUID primary keys (layer_id)
- Layer chain traversal: start at current_layer_id, follow parent_layer_id until NULL
- Delete markers: layer_entries.is_deleted = true (file appears deleted in union view)
- Reference counting: data_blocks and text_blocks track usage, cleaned when refs = 0

### Text File Storage
- TextBlock: stores single line content with content_hash
- TextFileMetadata: stores file-level info (total_lines, encoding)
- TextLineMap: maps file + line_number → text_block_id
- On modification: compute diff, create new TextBlocks for changed lines, update TextLineMap
- Unchanged lines reuse existing TextBlocks across files and layers

### Audit Logs
- Partitioned by time (monthly/daily depending on volume)
- Async batch insertion (don't block file operations)
- Include text_changes JSON field for text files (lines_added, lines_deleted, etc.)
- Auto-create partitions, clean expired ones based on retention_days

### Virtual Filesystem (/.tarbox/)
- Intercept in FUSE layer before hitting database
- Generate content dynamically
- Writing triggers commands (parsed in layer management code)
- Reading returns results (e.g., layer list, diff output)

### Native Mounts
Database table:
```sql
CREATE TABLE native_mounts (
    mount_id UUID PRIMARY KEY,
    mount_path TEXT NOT NULL,           -- Virtual path (e.g., "/bin")
    source_path TEXT NOT NULL,          -- Host path (e.g., "/bin" or "/var/tarbox/venvs/{tenant_id}")
    mode VARCHAR(2) NOT NULL,           -- 'ro' or 'rw'
    is_shared BOOLEAN NOT NULL,         -- true for cross-tenant, false for tenant-specific
    tenant_id UUID,                     -- NULL if shared
    enabled BOOLEAN NOT NULL,
    priority INTEGER NOT NULL,          -- Lower = higher priority
    ...
)
```

Path matching order:
1. Exact match > prefix match
2. Longer path > shorter path  
3. Lower priority value > higher priority value
4. Tenant-specific > shared

Operations on native mounts:
- Bypass PostgreSQL completely
- Direct passthrough to host FS using std::fs operations
- Write operations: check mode='rw', return EROFS if ro
- Audit logs: set is_native_mount=true, native_source_path=resolved_path
- NOT subject to layering (no COW, no history)

## Project Structure

- `spec/` - Architecture design (12 numbered specs: overview, database, FUSE, audit, layers, K8s, API, performance, hooks, multi-tenancy, text optimization, dependencies, native-mounting)
- `task/` - Development tasks organized by priority
  - **MVP tasks** (00-04): Minimal viable product
    - 00: MVP roadmap
    - 01: Project setup ✅
    - 02: Database layer (MVP) - tenants, inodes, data_blocks only
    - 03: Filesystem core (MVP) - basic POSIX ops, no permissions/caching
    - 04: CLI tool (MVP) - tenant and file operations via command line
  - **Advanced tasks** (05-08): Full features
    - 05: FUSE interface
    - 06: Database layer advanced (audit, layers, text optimization)
    - 07: Filesystem core advanced (permissions, links, caching)
    - 08: Layered filesystem (COW, checkpoints)
- Task files track progress with checkboxes, dependencies, and acceptance criteria

## Important Constraints

- Layer model is LINEAR (no branches), enforced in layer creation logic
- Paths are limited: 4096 bytes total, 255 bytes per component
- Text file detection: if not valid UTF-8, treat as binary
- Text file thresholds: if too large or binary-like, downgrade to binary storage
- Inode IDs are i64, everything else (tenant, layer, block) is UUID
- FUSE is synchronous, bridge to async tokio runtime with block_on or spawn_blocking

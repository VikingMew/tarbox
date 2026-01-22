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
- Test coverage must be > 80% (project-wide requirement)
  - Unit tests (no external dependencies): target 55-60%
  - Integration tests (with mocks): fill the gap to 80%+
  - Use mockall for mocking database and external dependencies

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

**Note**: Native directory mounting (originally planned in `native/` module) will be handled by **bubblewrap** at the container level instead of being implemented in Tarbox itself. This follows the single responsibility principle and reduces complexity.

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

**Container Integration**: Use **bubblewrap** for mounting host directories:
- System directories: `--ro-bind /usr /usr --ro-bind /bin /bin`
- Tenant workspaces: `--bind /host/venvs/{tenant_id} /.venv`
- Shared resources: `--ro-bind /data/models /models`
- This approach is simpler and more performant than implementing mounting inside Tarbox

### Data Flow Examples

**File Read**:
1. FUSE read() receives path and tenant_id
2. Normal path resolution (PostgreSQL)
3. Read from data_blocks or text_blocks
4. Return data
5. Log to audit_log

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

## Testing Strategy

### Coverage Requirements
The project MUST maintain >80% test coverage using a layered testing approach:

**Layer 1: Unit Tests (~55% coverage)**
- Test pure functions and data structures without external dependencies
- Located in: `src/**/*.rs` in `#[cfg(test)]` modules
- Target modules:
  - `fs/error.rs`, `fs/path.rs` (100% achievable)
  - `storage/models.rs`, `storage/traits.rs` (100% achievable)
  - `fuse/interface.rs`, `fuse/backend.rs` (partial coverage)
  - `config/` modules (partial coverage)

**Layer 2: Integration Tests with Mockall (~25-30% additional coverage)**
- Use mockall to mock database and external dependencies
- Located in: `tests/*_integration_test.rs`
- Mock the following traits:
  - `storage::traits::TenantRepository`
  - `storage::traits::InodeRepository`
  - `storage::traits::BlockRepository`
  - Any trait with `#[automock]` annotation
- Test business logic in:
  - `fs/operations.rs` (file operations with mocked storage)
  - `storage/inode.rs`, `storage/tenant.rs`, `storage/block.rs` (with mocked PgPool)
  - Layer management logic
  - Audit logging logic

**Layer 3: E2E Tests with Real Database (optional, for CI/CD)**
- Require PostgreSQL test database
- Located in: `tests/*_e2e_test.rs`
- Run with: `DATABASE_URL=postgres://... cargo test`

### Mockall Usage Guidelines

**1. Define Mockable Traits**
All repository interfaces MUST be defined as traits in `storage/traits.rs`:
```rust
#[cfg_attr(test, automock)]
#[async_trait]
pub trait TenantRepository {
    async fn create_tenant(&self, input: CreateTenantInput) -> Result<Tenant>;
    async fn get_tenant(&self, tenant_id: Uuid) -> Result<Option<Tenant>>;
    async fn list_tenants(&self) -> Result<Vec<Tenant>>;
    async fn delete_tenant(&self, tenant_id: Uuid) -> Result<()>;
}
```

**2. Integration Test Structure**
Create integration tests that mock database operations:
```rust
// tests/filesystem_operations_integration_test.rs
use mockall::predicate::*;
use tarbox::storage::traits::{MockInodeRepository, MockBlockRepository};
use tarbox::fs::operations::FileSystem;

#[tokio::test]
async fn test_create_file_with_mocked_storage() {
    let mut mock_inode_repo = MockInodeRepository::new();
    let mut mock_block_repo = MockBlockRepository::new();
    
    // Setup expectations
    mock_inode_repo
        .expect_create_inode()
        .with(eq(tenant_id), always())
        .times(1)
        .returning(|_, input| Ok(Inode { /* ... */ }));
    
    // Test filesystem operations with mocks
    let fs = FileSystem::with_repos(mock_inode_repo, mock_block_repo);
    let result = fs.create_file("/test.txt").await;
    
    assert!(result.is_ok());
}
```

**3. Test File Naming Convention**
- Unit tests: embedded in source files as `mod tests`
- Integration tests with mocks: `tests/<module>_integration_test.rs`
  - `tests/storage_integration_test.rs` - storage layer with mocked DB
  - `tests/filesystem_operations_integration_test.rs` - FS operations with mocked storage
  - `tests/layer_management_integration_test.rs` - layer operations with mocks
- E2E tests: `tests/<feature>_e2e_test.rs`
  - `tests/storage_e2e_test.rs` - requires real PostgreSQL

**4. Mock vs Real Database Decision Matrix**
| Component | Test Type | Dependency |
|-----------|-----------|------------|
| `fs/error.rs`, `fs/path.rs` | Unit | None |
| `storage/models.rs` | Unit | None |
| `config/` | Unit | None |
| `fs/operations.rs` | Integration | Mock storage traits |
| `storage/inode.rs` | Integration | Mock PgPool or real DB |
| `fuse/adapter.rs` | Integration | Mock FileSystem |
| Full POSIX ops | E2E | Real PostgreSQL |

**5. Running Tests**
```bash
# Unit tests only (fast, no dependencies)
cargo test --lib

# Unit + integration tests with mocks (fast, no DB required)
cargo test

# All tests including E2E (requires PostgreSQL)
DATABASE_URL=postgres://user:pass@localhost/tarbox_test cargo test --all-targets

# Coverage report (unit + integration with mocks)
cargo llvm-cov --all-targets --html
```

**6. Mockall Best Practices**
- Use `#[cfg_attr(test, automock)]` to generate mocks only in test builds
- Define clear expectations with `.expect_method_name()`
- Use `predicate::*` for flexible argument matching
- Mock at trait boundaries, not implementation details
- Keep mocks simple - don't mock what you can construct
- Name mock variables clearly: `mock_tenant_repo`, `mock_inode_repo`

**7. What NOT to Mock**
- Simple data structures (Inode, Tenant, Block)
- Pure functions (path normalization, hash computation)
- Error types
- Configuration structs

### Test Organization

```
tests/
├── config_and_storage_models_test.rs           # Unit tests for config and models
├── storage_models_comprehensive_test.rs        # Comprehensive model tests
├── storage_integration_test.rs                 # Storage with mocked DB traits
├── filesystem_operations_integration_test.rs   # FS ops with mocked storage (TO CREATE)
├── layer_management_integration_test.rs        # Layer ops with mocks (TO CREATE)
└── storage_e2e_test.rs                         # E2E with real PostgreSQL (optional)
```

## Task Completion Requirements

**CRITICAL**: Every task MUST meet these requirements to be considered complete:

### Code Requirements
1. ✅ All planned functionality implemented
2. ✅ Code compiles without errors
3. ✅ `cargo fmt --all` passes
4. ✅ `cargo clippy --all-targets --all-features -- -D warnings` passes

### Testing Requirements (MANDATORY)
Each task MUST achieve **>80% test coverage** before being marked as complete:

#### For Implementation Tasks (Task 02, 03, 04, 05, 06, 07, 08)
**Required**:
- ✅ Unit tests for all pure functions (target: 55-60% coverage)
- ✅ Integration tests with mockall for business logic (additional 25-30% coverage)
- ✅ Total coverage **>80%**
- ✅ All tests pass

**Verification**:
```bash
# Must pass before task completion
cargo test --lib --features mockall
cargo llvm-cov --lib --tests --features mockall --summary-only
# Check TOTAL line shows >80%
```

**Integration Test Requirements**:
- Create `tests/<module>_integration_test.rs` for each major module
- Use `MockInodeRepository`, `MockBlockRepository`, etc. from `storage::traits`
- Test business logic with mocked dependencies
- Cover error cases and edge cases

#### For Infrastructure Tasks (Task 01)
- ✅ Project compiles
- ✅ Basic sanity tests pass
- Coverage requirement: N/A (infrastructure only)

### Documentation Requirements
- ✅ Update `task/XX-task-name.md` with completion status
- ✅ Mark checkboxes for completed subtasks
- ✅ Document any deviations from original plan
- ✅ Update coverage report if applicable

### Common Reasons Tasks Are NOT Complete
❌ "Code works but no tests" - **NOT COMPLETE**
❌ "Unit tests only, 45% coverage" - **NOT COMPLETE**
❌ "Integration tests planned but not written" - **NOT COMPLETE**  
❌ "E2E tests exist but require database setup" - **OK if unit + integration >80%**

### Coverage Enforcement
- Run `cargo llvm-cov` after every task
- Create `COVERAGE_REPORT.md` showing before/after
- If coverage <80%, task status = "Partial - Needs Tests"
- Tests are NOT optional - they are part of the task

### Example Task Completion Checklist
```markdown
## Task XX Completion Status

### Code ✅
- [x] All features implemented
- [x] Compiles successfully
- [x] fmt and clippy pass

### Tests ✅ 
- [x] Unit tests: 94 tests, 55% coverage
- [x] Integration tests: 12 tests, 28% coverage
- [x] **Total coverage: 83%** ✅
- [x] All tests pass

### Documentation ✅
- [x] Task file updated
- [x] Coverage report created

**Status**: ✅ COMPLETE
```

### What to Do if Coverage is Low
If task coverage <80%:
1. Identify untested modules with `cargo llvm-cov --summary-only`
2. Create integration tests in `tests/` directory
3. Use mockall to mock dependencies
4. Add error case tests
5. Re-run coverage until >80%
6. **Do not mark task complete until coverage target met**

## Important Constraints

- Layer model is LINEAR (no branches), enforced in layer creation logic
- Paths are limited: 4096 bytes total, 255 bytes per component
- Text file detection: if not valid UTF-8, treat as binary
- Text file thresholds: if too large or binary-like, downgrade to binary storage
- Inode IDs are i64, everything else (tenant, layer, block) is UUID
- FUSE is synchronous, bridge to async tokio runtime with block_on or spawn_blocking

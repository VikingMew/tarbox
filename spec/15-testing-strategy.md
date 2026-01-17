# Spec 15: 测试策略与覆盖率

## 概述

本规范定义 Tarbox 项目的测试策略、覆盖率目标和测试实践。Tarbox 要求 >80% 的测试覆盖率，通过单元测试、集成测试（使用 mockall）和端到端测试的分层组合来实现。

## 设计原则

### 1. 分层测试金字塔

```
      /\
     /E2E\          少量 (~5%) - 真实数据库
    /------\
   /  集成  \       中等 (~25-30%) - Mockall 模拟
  /----------\
 /  单元测试  \     大量 (~55-60%) - 纯函数
/--------------\
```

**层级说明**：
- **L1 单元测试**：测试纯函数和数据结构，无外部依赖
- **L2 集成测试（Mockall）**：使用 mock 测试业务逻辑，无需数据库
- **L3 端到端测试**：使用真实 PostgreSQL，验证完整流程

### 2. 测试独立性

- 每个测试必须独立运行，不依赖其他测试
- 集成测试使用 mock，不需要外部服务
- E2E 测试使用事务或测试数据库隔离

### 3. 快速反馈

- 单元测试：<1 秒
- 集成测试（mock）：<5 秒
- E2E 测试：<30 秒

## 覆盖率目标

### 项目级别要求

```
总覆盖率 > 80%
├── 单元测试: 55-60% (无外部依赖)
├── 集成测试（mockall）: 25-30% (模拟依赖)
└── E2E 测试: ~5% (真实数据库，可选)
```

### 模块级别目标

| 模块 | 单元测试 | 集成测试 | E2E 测试 | 总目标 |
|------|----------|----------|----------|--------|
| `fs/error.rs` | 100% | - | - | 100% |
| `fs/path.rs` | 95-100% | - | - | 100% |
| `storage/models.rs` | 100% | - | - | 100% |
| `storage/traits.rs` | 100% | - | - | 100% |
| `config/` | 80% | - | - | 80% |
| `fuse/interface.rs` | 80% | - | - | 80% |
| `fuse/backend.rs` | 40% | 40% | - | 80% |
| `fs/operations.rs` | 0% | 80% | - | 80% |
| `storage/inode.rs` | 0% | 70% | 10% | 80% |
| `storage/tenant.rs` | 0% | 70% | 10% | 80% |
| `storage/block.rs` | 30% | 50% | - | 80% |
| `storage/pool.rs` | 40% | 40% | - | 80% |

## L1: 单元测试

### 目标与原则

**目标**：测试纯函数、数据结构、不依赖外部服务的逻辑

**原则**：
- 无 mock，无 I/O
- 测试函数输入输出
- 验证边界条件
- 快速执行（< 1ms）

### 适用模块

```rust
// 100% 单元测试覆盖的模块
src/
├── fs/error.rs              // 错误类型转换
├── fs/path.rs               // 路径处理函数
├── storage/models.rs        // 数据模型
├── storage/traits.rs        // Trait 定义
├── storage/block.rs         // 哈希计算函数
└── types.rs                 // 类型别名

// 部分单元测试覆盖的模块
src/
├── config/                  // 配置解析（80%）
├── fuse/interface.rs        // 数据结构（80%）
├── fuse/backend.rs          // 转换函数（40%）
└── storage/pool.rs          // 配置验证（40%）
```

### 测试组织

**嵌入式测试**（推荐用于简单模块）：
```rust
// src/fs/path.rs
pub fn normalize_path(path: &str) -> String {
    // Implementation
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path_removes_trailing_slash() {
        assert_eq!(normalize_path("/foo/bar/"), "/foo/bar");
    }

    #[test]
    fn test_normalize_path_handles_empty_string() {
        assert_eq!(normalize_path(""), "/");
    }
}
```

**独立测试文件**（用于复杂模块）：
```rust
// tests/unit/path_normalization_test.rs
use tarbox::fs::path::*;

#[test]
fn test_path_normalization_comprehensive() {
    let cases = vec![
        ("", "/"),
        ("/", "/"),
        ("/foo", "/foo"),
        ("/foo/", "/foo"),
        ("//foo//bar//", "/foo/bar"),
    ];
    
    for (input, expected) in cases {
        assert_eq!(normalize_path(input), expected);
    }
}
```

### 命名规范

```
test_<function>_<scenario>_<expected_result>

示例：
- test_normalize_path_empty_string_returns_root
- test_compute_hash_same_content_produces_same_hash
- test_inode_type_file_equals_file
```

## L2: 集成测试（Mockall）

### 目标与原则

**目标**：测试业务逻辑，模拟数据库和外部依赖

**原则**：
- 使用 mockall 模拟 trait
- 测试多个模块交互
- 验证调用顺序和参数
- 快速执行（< 100ms）

### Mockall 策略

#### 1. 定义可 Mock 的 Trait

**所有数据库操作必须通过 trait 抽象**：

```rust
// src/storage/traits.rs
use mockall::automock;
use async_trait::async_trait;

#[cfg_attr(test, automock)]
#[async_trait]
pub trait TenantRepository {
    async fn create_tenant(&self, input: CreateTenantInput) -> Result<Tenant>;
    async fn get_tenant(&self, tenant_id: Uuid) -> Result<Option<Tenant>>;
    async fn get_tenant_by_name(&self, name: &str) -> Result<Option<Tenant>>;
    async fn list_tenants(&self) -> Result<Vec<Tenant>>;
    async fn delete_tenant(&self, tenant_id: Uuid) -> Result<()>;
}

#[cfg_attr(test, automock)]
#[async_trait]
pub trait InodeRepository {
    async fn create_inode(&self, tenant_id: Uuid, input: CreateInodeInput) -> Result<Inode>;
    async fn get_inode(&self, tenant_id: Uuid, inode_id: InodeId) -> Result<Option<Inode>>;
    async fn get_inode_by_path(&self, tenant_id: Uuid, path: &str) -> Result<Option<Inode>>;
    async fn update_inode(&self, tenant_id: Uuid, inode_id: InodeId, input: UpdateInodeInput) -> Result<()>;
    async fn delete_inode(&self, tenant_id: Uuid, inode_id: InodeId) -> Result<()>;
    async fn list_children(&self, tenant_id: Uuid, parent_id: InodeId) -> Result<Vec<Inode>>;
}

#[cfg_attr(test, automock)]
#[async_trait]
pub trait BlockRepository {
    async fn create_block(&self, input: CreateBlockInput) -> Result<BlockId>;
    async fn get_block(&self, block_id: BlockId) -> Result<Option<DataBlock>>;
    async fn delete_block(&self, block_id: BlockId) -> Result<()>;
    async fn get_or_create_block(&self, content_hash: &str, data: &[u8]) -> Result<BlockId>;
}
```

**关键点**：
- 使用 `#[cfg_attr(test, automock)]` 只在测试时生成 mock
- 必须配合 `#[async_trait]` 使用
- Trait 方法必须是 `async fn`

#### 2. 使用依赖注入

**错误做法**（直接依赖具体类型）：
```rust
// ❌ 无法 mock
pub struct FileSystem {
    pool: PgPool,  // 具体类型，无法替换
}

impl FileSystem {
    pub async fn create_file(&self, path: &str) -> Result<InodeId> {
        sqlx::query!("INSERT INTO inodes ...").execute(&self.pool).await?;
        Ok(inode_id)
    }
}
```

**正确做法**（依赖 trait）：
```rust
// ✅ 可以 mock
pub struct FileSystem<I, B>
where
    I: InodeRepository,
    B: BlockRepository,
{
    inode_repo: Arc<I>,
    block_repo: Arc<B>,
    tenant_id: Uuid,
}

impl<I, B> FileSystem<I, B>
where
    I: InodeRepository + Send + Sync,
    B: BlockRepository + Send + Sync,
{
    pub fn new(inode_repo: Arc<I>, block_repo: Arc<B>, tenant_id: Uuid) -> Self {
        Self { inode_repo, block_repo, tenant_id }
    }

    pub async fn create_file(&self, path: &str, mode: u32) -> Result<InodeId> {
        let input = CreateInodeInput {
            name: path.to_string(),
            inode_type: InodeType::File,
            mode,
            ..Default::default()
        };
        
        let inode = self.inode_repo.create_inode(self.tenant_id, input).await?;
        Ok(inode.inode_id)
    }
}
```

#### 3. 编写集成测试

```rust
// tests/filesystem_operations_integration_test.rs
use mockall::predicate::*;
use tarbox::storage::traits::{MockInodeRepository, MockBlockRepository};
use tarbox::fs::operations::FileSystem;
use uuid::Uuid;

#[tokio::test]
async fn test_create_file_calls_inode_repository() {
    // Arrange
    let tenant_id = Uuid::new_v4();
    let mut mock_inode_repo = MockInodeRepository::new();
    let mock_block_repo = MockBlockRepository::new();
    
    // 设置期望：create_inode 会被调用一次
    mock_inode_repo
        .expect_create_inode()
        .with(
            eq(tenant_id),
            function(|input: &CreateInodeInput| {
                input.name == "/test.txt" && input.inode_type == InodeType::File
            })
        )
        .times(1)
        .returning(|_, input| {
            Ok(Inode {
                inode_id: 1001,
                tenant_id,
                name: input.name.clone(),
                inode_type: input.inode_type,
                mode: input.mode,
                ..Default::default()
            })
        });
    
    let fs = FileSystem::new(
        Arc::new(mock_inode_repo),
        Arc::new(mock_block_repo),
        tenant_id,
    );
    
    // Act
    let result = fs.create_file("/test.txt", 0o644).await;
    
    // Assert
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1001);
}

#[tokio::test]
async fn test_write_file_creates_block_and_updates_inode() {
    let tenant_id = Uuid::new_v4();
    let mut mock_inode_repo = MockInodeRepository::new();
    let mut mock_block_repo = MockBlockRepository::new();
    
    // 期望 1: 查找 inode
    mock_inode_repo
        .expect_get_inode_by_path()
        .with(eq(tenant_id), eq("/test.txt"))
        .times(1)
        .returning(move |_, _| {
            Ok(Some(Inode {
                inode_id: 1001,
                tenant_id,
                inode_type: InodeType::File,
                ..Default::default()
            }))
        });
    
    // 期望 2: 创建数据块
    let block_id = Uuid::new_v4();
    mock_block_repo
        .expect_get_or_create_block()
        .with(always(), eq(b"Hello, World!".as_ref()))
        .times(1)
        .returning(move |_, _| Ok(block_id));
    
    // 期望 3: 更新 inode
    mock_inode_repo
        .expect_update_inode()
        .with(
            eq(tenant_id),
            eq(1001),
            function(|input: &UpdateInodeInput| {
                input.size == Some(13)
            })
        )
        .times(1)
        .returning(|_, _, _| Ok(()));
    
    let fs = FileSystem::new(
        Arc::new(mock_inode_repo),
        Arc::new(mock_block_repo),
        tenant_id,
    );
    
    // Act
    let result = fs.write_file("/test.txt", b"Hello, World!").await;
    
    // Assert
    assert!(result.is_ok());
}
```

#### 4. Mock 最佳实践

**✅ 推荐做法**：

```rust
// 1. 使用 predicate 进行灵活匹配
use mockall::predicate::*;

mock.expect_method()
    .with(eq(expected_value), always(), function(|x| x > 0))
    .returning(|_, _, _| Ok(result));

// 2. 验证调用次数
mock.expect_method()
    .times(1)          // 必须调用 1 次
    .times(1..=3)      // 调用 1-3 次
    .times(..)         // 任意次数
    .returning(|| Ok(()));

// 3. 返回不同结果
mock.expect_method()
    .returning(|input| {
        if input.is_valid() {
            Ok(Success)
        } else {
            Err(anyhow!("Invalid input"))
        }
    });

// 4. 序列化调用顺序
let mut seq = Sequence::new();

mock1.expect_method1()
    .times(1)
    .in_sequence(&mut seq)
    .returning(|| Ok(()));

mock2.expect_method2()
    .times(1)
    .in_sequence(&mut seq)
    .returning(|| Ok(()));
```

**❌ 避免做法**：

```rust
// ❌ 不要 mock 简单数据结构
let mut mock_inode = MockInode::new();  // 错误！直接构造 Inode

// ✅ 正确：直接构造
let inode = Inode { inode_id: 1, ..Default::default() };

// ❌ 不要 mock 纯函数
let mut mock_path = MockPath::new();
mock_path.expect_normalize().returning(|p| p);  // 错误！

// ✅ 正确：直接调用
let normalized = normalize_path("/foo/bar/");

// ❌ 不要过度指定
mock.expect_method()
    .with(eq(exact_string), eq(exact_number), eq(exact_uuid))  // 太严格
    .returning(|| Ok(()));

// ✅ 正确：使用谓词
mock.expect_method()
    .with(always(), function(|n| n > 0), always())
    .returning(|| Ok(()));
```

### 测试组织

```
tests/
├── filesystem_operations_integration_test.rs
│   ├── test_create_file_with_mocked_storage
│   ├── test_write_file_creates_block
│   ├── test_read_file_retrieves_block
│   └── test_delete_file_removes_inode_and_block
│
├── storage_layer_integration_test.rs
│   ├── test_tenant_repository_create
│   ├── test_inode_repository_crud
│   └── test_block_repository_deduplication
│
└── layer_management_integration_test.rs
    ├── test_create_layer_with_mocked_storage
    ├── test_switch_layer_updates_current
    └── test_layer_chain_resolution
```

### 命名规范

```
test_<operation>_<scenario>_<expected_behavior>

示例：
- test_create_file_with_valid_path_calls_inode_repository
- test_write_file_missing_inode_returns_error
- test_delete_file_removes_inode_and_decrements_block_refcount
```

## L3: 端到端测试

### 目标与原则

**目标**：验证完整流程，使用真实 PostgreSQL

**原则**：
- 最小化数量（仅关键路径）
- 使用测试数据库
- 事务隔离或 cleanup
- 可接受较慢（< 5s）

### 数据库准备

```rust
// tests/common/mod.rs
use sqlx::{PgPool, Postgres};

pub async fn setup_test_db() -> PgPool {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5433/tarbox_test".to_string());
    
    PgPool::connect(&database_url).await.unwrap()
}

pub async fn cleanup_test_db(pool: &PgPool) {
    sqlx::query("TRUNCATE TABLE inodes, tenants, data_blocks RESTART IDENTITY CASCADE")
        .execute(pool)
        .await
        .unwrap();
}
```

### E2E 测试示例

```rust
// tests/storage_e2e_test.rs
mod common;

#[tokio::test]
async fn test_full_file_lifecycle_with_real_database() {
    let pool = common::setup_test_db().await;
    
    // 创建租户
    let tenant = TenantRepositoryImpl::new(&pool)
        .create_tenant(CreateTenantInput {
            name: "test_tenant".to_string(),
        })
        .await
        .unwrap();
    
    // 创建文件系统
    let fs = FileSystem::new(&pool, tenant.tenant_id);
    
    // 创建文件
    let inode_id = fs.create_file("/test.txt", 0o644).await.unwrap();
    
    // 写入数据
    fs.write_file("/test.txt", b"Hello, World!").await.unwrap();
    
    // 读取数据
    let data = fs.read_file("/test.txt").await.unwrap();
    assert_eq!(data, b"Hello, World!");
    
    // 删除文件
    fs.delete_file("/test.txt").await.unwrap();
    
    // 验证已删除
    let result = fs.read_file("/test.txt").await;
    assert!(result.is_err());
    
    common::cleanup_test_db(&pool).await;
}
```

### 运行 E2E 测试

```bash
# 启动测试数据库（Docker）
docker compose up -d postgres-test

# 运行 E2E 测试
DATABASE_URL=postgres://postgres:postgres@localhost:5433/tarbox_test \
  cargo test --test storage_e2e_test

# 清理
docker compose down
```

## 测试工具和命令

### 覆盖率报告

```bash
# 安装 llvm-cov
cargo install cargo-llvm-cov

# 单元测试覆盖率（不需要数据库）
cargo llvm-cov --lib --html

# 单元 + 集成测试（mockall）
cargo llvm-cov --all-targets --html --ignore-filename-regex="e2e"

# 完整覆盖率（包含 E2E，需要数据库）
DATABASE_URL=postgres://... cargo llvm-cov --all-targets --html

# 查看报告
open target/llvm-cov/html/index.html
```

### CI/CD 集成

```yaml
# .github/workflows/test.yml
name: Tests

on: [push, pull_request]

jobs:
  unit-and-integration:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      # 单元 + 集成测试（无需数据库）
      - name: Run unit and integration tests
        run: cargo test --lib --bins --tests
      
      - name: Check coverage
        run: |
          cargo llvm-cov --all-targets --html --ignore-filename-regex="e2e"
          COVERAGE=$(cargo llvm-cov --all-targets --ignore-filename-regex="e2e" | grep -oP 'TOTAL.*\K[0-9.]+%' | tr -d '%')
          if (( $(echo "$COVERAGE < 80" | bc -l) )); then
            echo "Coverage $COVERAGE% is below 80%"
            exit 1
          fi

  e2e:
    runs-on: ubuntu-latest
    services:
      postgres:
        image: postgres:16-alpine
        env:
          POSTGRES_PASSWORD: postgres
          POSTGRES_DB: tarbox_test
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
        ports:
          - 5432:5432
    
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Run E2E tests
        env:
          DATABASE_URL: postgres://postgres:postgres@localhost:5432/tarbox_test
        run: cargo test --test "*_e2e_test"
```

## 测试数据和 Fixture

### 测试数据生成

```rust
// tests/fixtures/mod.rs
use uuid::Uuid;
use tarbox::storage::models::*;

pub struct TestData;

impl TestData {
    pub fn sample_tenant() -> Tenant {
        Tenant {
            tenant_id: Uuid::new_v4(),
            name: "test_tenant".to_string(),
            created_at: chrono::Utc::now(),
        }
    }
    
    pub fn sample_inode(tenant_id: Uuid, inode_type: InodeType) -> Inode {
        Inode {
            inode_id: 1001,
            tenant_id,
            name: "test_file.txt".to_string(),
            inode_type,
            mode: 0o644,
            uid: 1000,
            gid: 1000,
            size: 0,
            parent_id: None,
            created_at: chrono::Utc::now(),
            modified_at: chrono::Utc::now(),
            accessed_at: chrono::Utc::now(),
        }
    }
    
    pub fn sample_create_inode_input(name: &str) -> CreateInodeInput {
        CreateInodeInput {
            name: name.to_string(),
            inode_type: InodeType::File,
            mode: 0o644,
            uid: 1000,
            gid: 1000,
            parent_id: None,
        }
    }
}
```

### 使用 Fixture

```rust
// tests/filesystem_operations_integration_test.rs
mod fixtures;

use fixtures::TestData;

#[tokio::test]
async fn test_create_file_uses_correct_defaults() {
    let tenant_id = Uuid::new_v4();
    let mut mock_inode_repo = MockInodeRepository::new();
    
    let expected_inode = TestData::sample_inode(tenant_id, InodeType::File);
    
    mock_inode_repo
        .expect_create_inode()
        .returning(move |_, _| Ok(expected_inode.clone()));
    
    // ... test implementation
}
```

## 性能基准测试

### 使用 Criterion

```rust
// benches/path_benchmark.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tarbox::fs::path::normalize_path;

fn bench_normalize_path(c: &mut Criterion) {
    c.bench_function("normalize_path", |b| {
        b.iter(|| normalize_path(black_box("/foo/bar/baz/")))
    });
}

criterion_group!(benches, bench_normalize_path);
criterion_main!(benches);
```

```bash
# 运行基准测试
cargo bench

# 对比基准
cargo bench --bench path_benchmark -- --save-baseline before
# ... make changes ...
cargo bench --bench path_benchmark -- --baseline before
```

## 决策记录

### DR-15-01: 使用 Mockall 而非真实数据库进行集成测试

**决策**: 集成测试使用 mockall 模拟数据库操作，而非每次启动 PostgreSQL。

**理由**:
- **速度**: Mock 测试执行时间 <100ms，真实 DB 需要 >1s
- **简单性**: 无需配置和管理测试数据库
- **隔离性**: 完全独立，不受外部状态影响
- **CI 友好**: 无需 Docker 或 PostgreSQL 服务

**权衡**:
- Mock 无法捕获 SQL 错误或性能问题
- 需要维护 trait 抽象层
- 需要少量 E2E 测试验证真实集成

**替代方案**:
- ❌ 所有测试使用真实 DB：太慢，CI 复杂
- ❌ 使用 SQLite：行为差异大，不可靠
- ✅ 分层测试：Mock 为主，E2E 为辅

### DR-15-02: 80% 覆盖率目标的组成

**决策**: 通过 55% 单元测试 + 25% 集成测试（mock）达到 80%。

**理由**:
- **架构约束**: 45% 代码直接依赖 sqlx::PgPool，无法纯单元测试
- **实用性**: 集成测试提供真实业务逻辑验证
- **速度**: Mock 集成测试仍然很快（< 5s 全部运行）

**权衡**:
- 需要更多集成测试编写工作
- Trait 抽象增加代码复杂度
- Mock 配置相对繁琐

**替代方案**:
- ❌ 降低覆盖率要求：不符合项目标准
- ❌ 重构为纯函数：过度工程，损失性能
- ✅ 当前方案：平衡测试质量和开发效率

### DR-15-03: 测试命名规范

**决策**: 使用 `test_<function>_<scenario>_<expected>` 格式。

**理由**:
- **可读性**: 测试名称即文档
- **可搜索**: 按功能或场景快速定位
- **一致性**: 整个项目统一风格

**示例**:
```rust
// ✅ 好的命名
test_normalize_path_empty_string_returns_root()
test_create_file_with_valid_path_calls_inode_repository()
test_write_file_missing_inode_returns_not_found_error()

// ❌ 不好的命名
test1()
test_path()
test_file_operations()
```

## 实施清单

### Phase 1: 单元测试基础（已完成 ✅）
- [x] fs/error.rs - 100% 覆盖
- [x] fs/path.rs - 95%+ 覆盖
- [x] storage/models.rs - 100% 覆盖
- [x] storage/traits.rs - 100% 覆盖
- [x] storage/block.rs - 哈希函数 100% 覆盖
- [x] config/ - 80% 覆盖
- [x] 创建测试工具模块

### Phase 2: 集成测试框架（待实施）
- [ ] 为所有 repository 添加 `#[cfg_attr(test, automock)]`
- [ ] 重构 FileSystem 使用 trait 依赖注入
- [ ] 创建 `tests/filesystem_operations_integration_test.rs`
- [ ] 创建 `tests/storage_layer_integration_test.rs`
- [ ] 创建测试 fixture 模块

### Phase 3: 达到 80% 覆盖率（待实施）
- [ ] 完成 fs/operations.rs 集成测试（目标 80%）
- [ ] 完成 storage/inode.rs 集成测试（目标 70%）
- [ ] 完成 storage/tenant.rs 集成测试（目标 70%）
- [ ] 完成 fuse/backend.rs 集成测试（目标 40%）
- [ ] 验证总覆盖率 > 80%

### Phase 4: E2E 和 CI（可选）
- [ ] 创建 E2E 测试（关键路径）
- [ ] 配置 GitHub Actions
- [ ] 添加覆盖率徽章
- [ ] 设置覆盖率门槛检查

## 参考资料

### 外部文档
- [mockall 文档](https://docs.rs/mockall/)
- [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov)
- [Rust 测试最佳实践](https://rust-lang.github.io/api-guidelines/)

### 项目文档
- [CLAUDE.md](../CLAUDE.md) - 测试策略章节
- [Task 01](../task/01-project-setup.md) - 项目设置
- [Spec 11](11-dependencies.md) - 依赖管理

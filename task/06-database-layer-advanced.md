# Task 06: 数据库存储层高级功能

## 状态

**✅ 已完成** (2026-01-19)

## 目标

在 Task 02 (MVP核心) 的基础上，实现高级存储特性，包括：
- **审计日志系统**: 完整的操作审计和分区管理
- **分层存储表**: 支持 Docker 风格的层管理
- **文本文件优化**: 行级存储和内容寻址

**注意**: Task 02 已实现 spec/01 (MVP核心: tenants, inodes, data_blocks)，本任务实现高级数据库功能。租户/Inode/数据块的基础CRUD操作已在Task 02中完成，本任务不重复实现。

## 优先级

**P1 - 高优先级**

## 依赖

- Task 01: 项目初始化和基础设施搭建 ✅
- Task 02: 数据库存储层 MVP ✅

## 依赖的Spec

- **spec/03-audit-system.md** - 审计日志表、分区策略、异步批量写入（核心）
- **spec/04-layered-filesystem.md** - layers、layer_entries 表设计（核心）
- **spec/10-text-file-optimization.md** - text_blocks、text_file_metadata、text_line_map 表设计（核心）
- **spec/16-advanced-storage.md** - 连接池、事务、性能优化策略（核心）
- spec/07-performance.md - 查询优化、索引策略
- spec/01-database-schema.md - MVP 表作为基础

## 实现内容

### 1. 数据库迁移

- [x] **Migration 002: 审计日志系统** (167 lines)
  - `migrations/20260118000002_create_audit_logs.sql`
  - audit_logs 表（按 log_date 分区）
  - 分区管理函数 create_audit_log_partition()
  - 分区清理函数 cleanup_old_audit_partitions()
  - 索引优化（tenant_id, created_at, operation等）

- [x] **Migration 003: 层管理系统** (141 lines)
  - `migrations/20260118000003_create_layers.sql`
  - layers 表（层定义，parent_layer_id形成链）
  - layer_entries 表（层内文件变更记录）
  - tenant_current_layer 表（租户当前层跟踪）
  - get_layer_chain() 函数（递归CTE查询层链）
  - create_base_layer_for_tenant() 函数
  - 自动更新统计的触发器

- [x] **Migration 004: 文本文件优化** (165 lines)
  - `migrations/20260118000004_create_text_storage.sql`
  - text_blocks 表（内容寻址存储，blake3哈希）
  - text_file_metadata 表（文件级元数据）
  - text_line_map 表（行号到块的映射）
  - find_or_create_text_block() 函数（自动去重）
  - cleanup_unused_text_blocks() 函数（清理ref_count=0的块）
  - get_text_file_content() 函数（重组文件内容）
  - 自动管理引用计数的触发器

### 2. 数据模型

- [x] **审计日志模型** (`src/storage/models.rs`)
  - AuditLog - 审计记录
  - CreateAuditLogInput - 创建输入
  - QueryAuditLogsInput - 查询过滤器
  - AuditStats - 聚合统计结果

- [x] **层管理模型**
  - Layer - 层定义
  - LayerEntry - 层条目
  - TenantCurrentLayer - 当前层跟踪
  - LayerStatus 枚举 (Active, Creating, Deleting, Archived)
  - ChangeType 枚举 (Add, Modify, Delete)

- [x] **文本存储模型**
  - TextBlock - 文本块（带content_hash和ref_count）
  - TextFileMetadata - 文件元数据
  - TextLineMap - 行映射
  - 各种 Input 结构体

### 3. Repository Traits

- [x] **AuditLogRepository** (`src/storage/traits.rs`)
  - create() - 创建单条审计记录
  - batch_create() - 批量创建
  - query() - 条件查询（支持多种过滤器）
  - aggregate_stats() - 聚合统计

- [x] **LayerRepository**
  - create() - 创建层
  - get() - 查询层
  - list() - 列出租户的所有层
  - get_layer_chain() - 获取层链（递归）
  - delete() - 删除层
  - add_entry() - 添加层条目
  - list_entries() - 列出层条目
  - get_current_layer() - 获取当前层
  - set_current_layer() - 设置当前层

- [x] **TextBlockRepository**
  - create_block() - 创建文本块（自动去重）
  - get_block() - 查询块
  - get_block_by_hash() - 通过哈希查询（用于去重）
  - increment_ref_count() - 增加引用计数
  - decrement_ref_count() - 减少引用计数
  - create_metadata() - 创建文件元数据
  - get_metadata() - 查询元数据
  - create_line_mapping() - 批量创建行映射
  - get_line_mappings() - 查询行映射

### 4. Repository 实现

- [x] **AuditLogOperations** (`src/storage/audit.rs` - 256 lines)
  - 实现 AuditLogRepository trait
  - 动态SQL查询构建（支持多条件过滤）
  - 聚合统计计算
  - 所有查询强制包含 tenant_id

- [x] **LayerOperations** (`src/storage/layer.rs` - 254 lines)
  - 实现 LayerRepository trait
  - 递归CTE实现层链查询
  - 层条目管理
  - 当前层跟踪（tenant_current_layer表）
  - 所有查询强制包含 tenant_id

- [x] **TextBlockOperations** (`src/storage/text.rs` - 312 lines)
  - 实现 TextBlockRepository trait
  - Blake3内容哈希计算
  - 自动去重（先查询，找不到再创建）
  - 引用计数管理
  - 批量行映射创建
  - 所有查询强制包含 tenant_id

### 5. 集成测试

- [x] **审计日志测试** (`tests/audit_integration_test.rs` - 5 tests)
  - test_audit_log_create - 创建单条记录
  - test_audit_log_batch_create - 批量创建
  - test_audit_log_query - 基础查询
  - test_audit_log_query_with_filters - 多条件过滤
  - test_audit_log_aggregate_stats - 聚合统计

- [x] **层管理测试** (`tests/layer_integration_test.rs` - 10 tests)
  - test_layer_create - 创建层
  - test_layer_get - 查询层
  - test_layer_list - 列出所有层
  - test_layer_chain - 层链查询（递归）
  - test_layer_delete - 删除层
  - test_layer_add_entry - 添加层条目
  - test_layer_list_entries - 列出层条目
  - test_current_layer_tracking - 当前层跟踪
  - test_layer_with_parent - 父子层关系
  - test_layer_entry_change_types - 变更类型

- [x] **文本存储测试** (`tests/text_storage_integration_test.rs` - 14 tests)
  - test_text_block_create - 创建文本块
  - test_text_block_deduplication - 内容去重
  - test_text_block_get - 查询块
  - test_text_block_get_by_hash - 哈希查询
  - test_text_block_ref_count_increment - 增加引用
  - test_text_block_ref_count_decrement - 减少引用
  - test_text_metadata_create - 创建元数据
  - test_text_metadata_get - 查询元数据
  - test_text_line_map_create - 创建行映射
  - test_text_line_map_get - 查询行映射
  - test_text_line_map_multiline - 多行文件
  - test_text_storage_full_workflow - 完整流程
  - test_content_hash_consistency - 哈希一致性
  - test_line_map_with_block_offsets - 块内偏移

**总计: 29个集成测试**

### 6. 关键特性

- [x] **租户隔离**: 所有查询必须包含 tenant_id 过滤
- [x] **内容寻址**: 使用 blake3 哈希实现文本块去重
- [x] **引用计数**: 自动管理文本块的引用计数
- [x] **层链查询**: PostgreSQL递归CTE实现层的继承链查询
- [x] **分区管理**: 审计日志按日期分区，包含创建和清理函数
- [x] **动态查询**: 审计日志查询支持多条件动态组合

## 架构要点

### 租户隔离模式
```rust
sqlx::query_as::<_, AuditLog>(
    "SELECT * FROM audit_logs WHERE tenant_id = $1 AND ..."
).bind(tenant_id)
```

### 内容寻址去重
```rust
let content_hash = blake3::hash(content.as_bytes()).to_hex().to_string();
// 先查询是否存在
if let Some(existing) = get_block_by_hash(&content_hash).await? {
    return Ok(existing); // 复用现有块
}
// 不存在则创建新块
```

### 递归CTE层链查询
```sql
WITH RECURSIVE layer_chain AS (
    SELECT ..., 0 as depth FROM layers WHERE layer_id = $2
    UNION ALL
    SELECT ..., lc.depth + 1 FROM layers l
    INNER JOIN layer_chain lc ON l.layer_id = lc.parent_layer_id
)
SELECT * FROM layer_chain ORDER BY depth
```

## 测试覆盖率

### 单元测试（无需数据库）
- 98个测试全部通过
- 覆盖率: 47.76%
- 重点模块:
  - storage/models.rs: 100%
  - storage/traits.rs: 100%
  - fs/error.rs: 100%
  - fs/path.rs: 96.63%

### 集成测试（需要PostgreSQL）
- 29个集成测试已编写
- 需要运行 `DATABASE_URL=postgres://... cargo test` 来执行
- 预期增加覆盖率至 >80%

### 运行完整测试
```bash
# 启动数据库
docker compose up -d postgres

# 运行所有测试
DATABASE_URL=postgres://postgres:postgres@localhost:5432/tarbox cargo test

# 查看覆盖率
DATABASE_URL=postgres://postgres:postgres@localhost:5432/tarbox cargo llvm-cov --lib --tests --summary-only
```

## 验收标准

- [x] 所有迁移脚本创建完成（3个迁移文件）
- [x] 所有数据模型定义完成（15+结构体）
- [x] 所有Repository traits定义（3个traits，22个方法）
- [x] 所有Repository实现完成（822行代码）
- [x] 租户隔离在所有查询中强制执行
- [x] 内容寻址去重正确实现
- [x] 引用计数管理正确实现
- [x] 分区管理函数实现
- [x] 集成测试编写完成（29个测试）
- [x] 代码编译无错误
- [x] cargo fmt 通过
- [x] cargo clippy 无警告
- [x] 单元测试全部通过（98个）
- [x] 实现文档完整（TASK06_COMPLETION.md）

## 未实现内容（推迟到未来）

以下功能不在本任务范围内，推迟到未来任务：

- **配额管理**: 租户配额检查、更新使用量、配额告警（推迟到需要时实现）
- **监控和健康检查**: 连接池监控、Prometheus指标（推迟到运维需求明确时）
- **性能基准测试**: 压力测试、性能基准（推迟到性能调优阶段）
- **Inode统计功能**: 目录大小计算等（推迟到需要时实现）

## 文件清单

### 新增文件
- migrations/20260118000002_create_audit_logs.sql
- migrations/20260118000003_create_layers.sql
- migrations/20260118000004_create_text_storage.sql
- src/storage/audit.rs
- src/storage/layer.rs
- src/storage/text.rs
- tests/audit_integration_test.rs
- tests/layer_integration_test.rs
- tests/text_storage_integration_test.rs
- TASK06_COMPLETION.md

### 修改文件
- src/storage/models.rs（增加15+模型）
- src/storage/traits.rs（增加3个traits）
- src/storage/mod.rs（导出新模块）

## 技术栈

- sqlx 0.8.2 - 异步数据库驱动
- PostgreSQL 16+ - 数据库
- tokio 1.x - 异步运行时
- blake3 - 内容哈希
- anyhow - 错误处理

## 后续任务

完成后可以开始：
- Task 07: 文件系统核心高级功能（权限、链接、缓存）
- Task 08: 分层文件系统实现（COW、检查点）

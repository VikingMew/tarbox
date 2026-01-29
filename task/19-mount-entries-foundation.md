# Task 19: 挂载条目基础设施 (Mount Entries Foundation)

## 概述

实现 spec/18（文件系统组合）的基础数据结构和数据库层，包括 mount_entries 表、MountSource 类型、路径解析核心逻辑。

## 依赖

- ✅ Task 02: 数据库存储层 MVP
- ✅ Task 06: 数据库层高级功能
- ✅ Task 08: 分层文件系统

## 交付物

### 1. 数据库 Schema

**文件**: `migrations/YYYYMMDDHHMMSS_create_mount_entries.sql`

```sql
-- mount_entries 表
CREATE TABLE mount_entries (
    mount_entry_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    
    -- 挂载点名称（用于 API 引用）
    name VARCHAR(255) NOT NULL,
    
    -- 虚拟路径（实际挂载位置）
    virtual_path TEXT NOT NULL,
    is_file BOOLEAN NOT NULL DEFAULT false,
    
    -- 源类型: 'host', 'layer', 'working_layer', 'published'
    source_type VARCHAR(20) NOT NULL,
    
    -- Host source
    host_path TEXT,
    
    -- Layer source
    source_mount_id UUID REFERENCES mount_entries(mount_entry_id),
    source_layer_id UUID,  -- 暂时不加外键，Task 21 会修改 layers 表
    source_subpath TEXT,
    
    -- Published source
    source_publish_name VARCHAR(255),
    
    -- WorkingLayer 的当前工作层 ID
    current_layer_id UUID,
    
    -- 访问模式: 'ro', 'rw', 'cow'
    mode VARCHAR(3) NOT NULL DEFAULT 'ro',
    
    enabled BOOLEAN NOT NULL DEFAULT true,
    metadata JSONB,
    
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    
    -- 约束
    CONSTRAINT valid_source CHECK (
        (source_type = 'host' AND host_path IS NOT NULL) OR
        (source_type = 'layer' AND source_mount_id IS NOT NULL) OR
        (source_type = 'published' AND source_publish_name IS NOT NULL) OR
        (source_type = 'working_layer')
    ),
    
    UNIQUE(tenant_id, name),
    UNIQUE(tenant_id, virtual_path)
);

CREATE INDEX idx_mount_entries_tenant ON mount_entries(tenant_id);
CREATE INDEX idx_mount_entries_name ON mount_entries(tenant_id, name);
CREATE INDEX idx_mount_entries_source ON mount_entries(source_mount_id);
```

### 2. Rust 数据结构

**文件**: `src/storage/models/mount_entry.rs`

```rust
// MountSource 枚举
pub enum MountSource {
    Host { path: PathBuf },
    Layer {
        source_mount_id: Uuid,
        layer_id: Option<Uuid>,
        subpath: Option<PathBuf>,
    },
    Published {
        publish_name: String,
        subpath: Option<PathBuf>,
    },
    WorkingLayer,
}

// MountMode 枚举
pub enum MountMode {
    ReadOnly,
    ReadWrite,
    CopyOnWrite,
}

// MountEntry 结构
pub struct MountEntry {
    pub mount_entry_id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub virtual_path: PathBuf,
    pub source: MountSource,
    pub mode: MountMode,
    pub is_file: bool,
    pub enabled: bool,
    pub current_layer_id: Option<Uuid>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// 创建输入
pub struct CreateMountEntry {
    pub name: String,
    pub virtual_path: PathBuf,
    pub source: MountSource,
    pub mode: MountMode,
    pub is_file: bool,
    pub metadata: Option<serde_json::Value>,
}

// 更新输入
pub struct UpdateMountEntry {
    pub mode: Option<MountMode>,
    pub enabled: Option<bool>,
    pub metadata: Option<serde_json::Value>,
}
```

### 3. 存储层 Repository

**文件**: `src/storage/mount_entry.rs`

实现以下 trait：

```rust
#[async_trait]
pub trait MountEntryRepository {
    /// 创建挂载条目
    async fn create_mount_entry(
        &self,
        tenant_id: Uuid,
        input: CreateMountEntry,
    ) -> Result<MountEntry>;
    
    /// 获取挂载条目
    async fn get_mount_entry(&self, mount_entry_id: Uuid) -> Result<Option<MountEntry>>;
    
    /// 通过名称获取
    async fn get_mount_entry_by_name(
        &self,
        tenant_id: Uuid,
        name: &str,
    ) -> Result<Option<MountEntry>>;
    
    /// 通过路径获取
    async fn get_mount_entry_by_path(
        &self,
        tenant_id: Uuid,
        path: &Path,
    ) -> Result<Option<MountEntry>>;
    
    /// 列出 Tenant 的所有挂载
    async fn list_mount_entries(&self, tenant_id: Uuid) -> Result<Vec<MountEntry>>;
    
    /// 更新挂载条目
    async fn update_mount_entry(
        &self,
        mount_entry_id: Uuid,
        input: UpdateMountEntry,
    ) -> Result<MountEntry>;
    
    /// 删除挂载条目
    async fn delete_mount_entry(&self, mount_entry_id: Uuid) -> Result<()>;
    
    /// 批量设置挂载（替换所有）
    async fn set_mount_entries(
        &self,
        tenant_id: Uuid,
        entries: Vec<CreateMountEntry>,
    ) -> Result<Vec<MountEntry>>;
    
    /// 检查路径冲突
    async fn check_path_conflict(
        &self,
        tenant_id: Uuid,
        path: &Path,
        exclude_id: Option<Uuid>,
    ) -> Result<bool>;
}
```

### 4. 路径解析器

**文件**: `src/composition/resolver.rs`

```rust
pub struct ResolvedPath {
    pub mount_entry: MountEntry,
    pub relative_path: PathBuf,
    pub source: ResolvedSource,
}

pub enum ResolvedSource {
    Host { full_path: PathBuf },
    Layer { tenant_id: Uuid, layer_id: Uuid, path: PathBuf },
    WorkingLayer { path: PathBuf },
}

#[async_trait]
pub trait PathResolver {
    /// 解析路径，找到对应的挂载点和相对路径
    async fn resolve_path(
        &self,
        tenant_id: Uuid,
        path: &Path,
    ) -> Result<ResolvedPath>;
    
    /// 验证路径不冲突、不嵌套
    fn validate_no_conflict(
        mounts: &[MountEntry],
        new_path: &Path,
        is_file: bool,
    ) -> Result<()>;
}
```

### 5. 路径冲突验证

**验证规则**（来自 spec/18）：
- 同一 Tenant 的挂载路径不可嵌套
- 同一 Tenant 的挂载路径不可冲突
- 在 `add_mount()` 时检查，冲突则返回 `MountPathConflict` 错误

```rust
fn validate_no_conflict(
    existing: &[MountEntry],
    new_path: &Path,
    is_file: bool,
) -> Result<()> {
    for entry in existing {
        // 检查精确冲突
        if entry.virtual_path == new_path {
            return Err(anyhow!("MountPathConflict: path already exists"));
        }
        
        // 检查嵌套（目录挂载）
        if !is_file && !entry.is_file {
            if new_path.starts_with(&entry.virtual_path) 
               || entry.virtual_path.starts_with(new_path) {
                return Err(anyhow!("MountPathConflict: nested paths not allowed"));
            }
        }
    }
    Ok(())
}
```

## 功能点

| 功能 | 说明 | 验收标准 |
|------|------|----------|
| 创建挂载条目 | 支持 4 种 source 类型 | 能创建 host/layer/published/working_layer 类型的挂载 |
| 路径冲突检测 | 不允许嵌套和冲突 | 嵌套/冲突时返回 MountPathConflict 错误 |
| 名称唯一性 | 同一 tenant 内名称唯一 | 重复名称时返回错误 |
| 路径解析 | 找到路径对应的挂载点 | 精确匹配文件挂载，前缀匹配目录挂载 |
| CRUD 操作 | 增删改查 | 所有操作正常工作 |
| 批量设置 | 替换所有挂载配置 | 事务性替换，失败时回滚 |

## 测试要求

### 单元测试 (target: 30+)

1. **MountSource 序列化/反序列化** (5 tests)
   - Host 类型
   - Layer 类型（有/无 subpath）
   - Published 类型
   - WorkingLayer 类型
   - 错误情况

2. **MountMode 转换** (3 tests)
   - ro/rw/cow 字符串转换
   - 默认值

3. **路径冲突检测** (10 tests)
   - 精确冲突
   - 嵌套冲突（目录在目录下）
   - 嵌套冲突（目录在目录上）
   - 文件不冲突
   - 平级目录不冲突
   - 空列表不冲突

4. **路径解析** (10 tests)
   - 精确匹配文件挂载
   - 前缀匹配目录挂载
   - 无匹配时错误
   - 多挂载点选择正确

### 集成测试 (target: 15+)

1. **数据库操作** (需要 PostgreSQL)
   - create_mount_entry
   - get_mount_entry_by_name
   - get_mount_entry_by_path
   - list_mount_entries
   - update_mount_entry
   - delete_mount_entry
   - set_mount_entries (批量)
   - 路径冲突检测

## 文件清单

```
src/
├── storage/
│   ├── models/
│   │   └── mount_entry.rs     # 数据模型
│   └── mount_entry.rs         # Repository 实现
├── composition/
│   ├── mod.rs
│   └── resolver.rs            # 路径解析器
migrations/
└── YYYYMMDDHHMMSS_create_mount_entries.sql
```

## 不包含（后续 Task）

- Layer 发布机制 (Task 20)
- 挂载点级别的 layer 链 (Task 21)
- HTTP API (Task 22)
- CLI 命令 (Task 23)
- FUSE 集成

## 完成标准

- [ ] 数据库 migration 文件创建并可执行
- [ ] MountEntry 相关数据结构实现
- [ ] MountEntryRepository trait 和实现
- [ ] PathResolver 实现
- [ ] 路径冲突验证逻辑
- [ ] 30+ 单元测试通过
- [ ] 15+ 集成测试通过
- [ ] cargo fmt 通过
- [ ] cargo clippy 通过
- [ ] 测试覆盖率 > 80%

## 预计工作量

2-3 天

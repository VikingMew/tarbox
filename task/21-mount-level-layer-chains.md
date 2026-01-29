# Task 21: 挂载点级别 Layer 链 (Mount-Level Layer Chains)

## 概述

实现挂载点级别的独立 Layer 链，每个 WorkingLayer 类型的挂载点有自己的 layer 历史，支持细粒度的 snapshot 和发布。

## 依赖

- ✅ Task 19: 挂载条目基础设施
- ✅ Task 20: Layer 发布机制
- ✅ Task 08: 分层文件系统

## 核心概念

```
Tenant: agent-001
├── /memory   -> WorkingLayer (layer 链 A: base -> snap1 -> snap2 -> working)
├── /workspace -> WorkingLayer (layer 链 B: base -> working)  
├── /claude.md -> WorkingLayer (layer 链 C: base -> snap1 -> working)
├── /usr      -> Host:/usr (只读，无 layer)
└── /models   -> Published:bert-v1 (只读，无 layer)
```

**关键点**：
- 每个 WorkingLayer 挂载点有独立的 layer 链
- 可以只 snapshot 特定挂载点（如只 snap /memory）
- 可以只发布特定挂载点的 layer

## 交付物

### 1. 数据库 Schema 修改

**文件**: `migrations/YYYYMMDDHHMMSS_update_layers_for_mounts.sql`

```sql
-- 修改 layers 表，添加 mount_entry_id
ALTER TABLE layers ADD COLUMN mount_entry_id UUID REFERENCES mount_entries(mount_entry_id);
ALTER TABLE layers ADD COLUMN is_working BOOLEAN NOT NULL DEFAULT false;

-- 每个挂载点只能有一个 working layer
CREATE UNIQUE INDEX idx_layers_unique_working 
    ON layers(mount_entry_id) 
    WHERE is_working = true;

CREATE INDEX idx_layers_mount ON layers(mount_entry_id);

-- 更新 mount_entries 的 current_layer_id 外键
ALTER TABLE mount_entries 
    ADD CONSTRAINT fk_current_layer 
    FOREIGN KEY (current_layer_id) REFERENCES layers(layer_id);
```

### 2. 更新 Layer 数据结构

**文件**: `src/storage/models/layer.rs` (更新)

```rust
pub struct Layer {
    pub layer_id: Uuid,
    pub tenant_id: Uuid,
    
    // 新增：所属挂载点（WorkingLayer 类型的挂载点）
    pub mount_entry_id: Option<Uuid>,
    
    pub parent_layer_id: Option<Uuid>,
    pub name: Option<String>,
    pub description: Option<String>,
    
    // 新增：是否是当前工作层
    pub is_working: bool,
    
    pub created_at: DateTime<Utc>,
}

/// 创建 Layer 输入
pub struct CreateLayerInput {
    pub mount_entry_id: Uuid,
    pub parent_layer_id: Option<Uuid>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub is_working: bool,
}
```

### 3. Layer Repository 更新

**文件**: `src/storage/layer.rs` (更新)

```rust
#[async_trait]
pub trait LayerRepository {
    // 现有方法保留...
    
    /// 为挂载点创建初始 layer 链（base + working）
    async fn create_initial_layers(
        &self,
        tenant_id: Uuid,
        mount_entry_id: Uuid,
    ) -> Result<(Layer, Layer)>;  // (base, working)
    
    /// 获取挂载点的 layer 链
    async fn get_mount_layers(
        &self,
        mount_entry_id: Uuid,
    ) -> Result<Vec<Layer>>;
    
    /// 获取挂载点的 working layer
    async fn get_working_layer(
        &self,
        mount_entry_id: Uuid,
    ) -> Result<Option<Layer>>;
    
    /// Snapshot：将当前 working layer 变为 snapshot，创建新 working layer
    async fn create_snapshot(
        &self,
        mount_entry_id: Uuid,
        name: &str,
        description: Option<&str>,
    ) -> Result<Layer>;  // 返回新的 working layer
    
    /// 批量 snapshot 多个挂载点
    async fn batch_snapshot(
        &self,
        tenant_id: Uuid,
        mount_names: &[String],
        name: &str,
        skip_unchanged: bool,
    ) -> Result<Vec<SnapshotResult>>;
}

pub struct SnapshotResult {
    pub mount_name: String,
    pub layer_id: Option<Uuid>,  // None if skipped
    pub skipped: bool,
    pub reason: Option<String>,
}
```

### 4. 挂载点 Layer 链管理

**文件**: `src/composition/layer_chain.rs`

```rust
pub struct LayerChainManager {
    layer_repo: Arc<dyn LayerRepository>,
    mount_entry_repo: Arc<dyn MountEntryRepository>,
}

impl LayerChainManager {
    /// 初始化挂载点的 layer 链
    /// 
    /// 在创建 WorkingLayer 类型的挂载点时调用
    pub async fn initialize_layer_chain(
        &self,
        tenant_id: Uuid,
        mount_entry_id: Uuid,
    ) -> Result<()>;
    
    /// 获取挂载点的完整 layer 链
    pub async fn get_layer_chain(
        &self,
        mount_entry_id: Uuid,
    ) -> Result<LayerChain>;
    
    /// Snapshot 单个挂载点
    pub async fn snapshot(
        &self,
        tenant_id: Uuid,
        mount_name: &str,
        snapshot_name: &str,
        description: Option<&str>,
    ) -> Result<Layer>;
    
    /// Snapshot 多个挂载点
    pub async fn snapshot_multiple(
        &self,
        tenant_id: Uuid,
        mount_names: &[String],
        snapshot_name: &str,
        skip_unchanged: bool,
    ) -> Result<Vec<SnapshotResult>>;
    
    /// Snapshot 所有 WorkingLayer 挂载点
    pub async fn snapshot_all(
        &self,
        tenant_id: Uuid,
        snapshot_name: &str,
        skip_unchanged: bool,
    ) -> Result<Vec<SnapshotResult>>;
    
    /// 检查挂载点是否有未保存的变化
    pub async fn has_changes(
        &self,
        mount_entry_id: Uuid,
    ) -> Result<bool>;
}

pub struct LayerChain {
    pub mount_entry_id: Uuid,
    pub mount_name: String,
    pub layers: Vec<Layer>,
    pub working_layer: Layer,
}
```

### 5. 与现有 FileSystem 集成

**更新**: `src/fs/operations.rs`

```rust
impl FileSystem {
    /// 写入文件时，写入到对应挂载点的 working layer
    pub async fn write_file(
        &self,
        tenant_id: Uuid,
        path: &Path,
        data: &[u8],
    ) -> Result<usize> {
        let resolved = self.resolver.resolve_path(tenant_id, path).await?;
        
        match resolved.mount_entry.mode {
            MountMode::ReadWrite => {
                match &resolved.source {
                    ResolvedSource::WorkingLayer { .. } => {
                        // 获取挂载点的 working layer
                        let working_layer = self.layer_chain_manager
                            .get_working_layer(resolved.mount_entry.mount_entry_id)
                            .await?;
                        
                        // 写入到该 layer
                        self.write_to_layer(
                            tenant_id,
                            working_layer.layer_id,
                            &resolved.relative_path,
                            data,
                        ).await
                    }
                    ResolvedSource::Host { full_path } => {
                        std::fs::write(full_path, data)?;
                        Ok(data.len())
                    }
                    _ => Err(anyhow!("Cannot write to this source")),
                }
            }
            MountMode::CopyOnWrite => {
                // COW: 写入到当前挂载点的 working layer
                // ...
            }
            MountMode::ReadOnly => {
                Err(anyhow!("EROFS: Read-only mount"))
            }
        }
    }
}
```

## 功能点

| 功能 | 说明 | 验收标准 |
|------|------|----------|
| 初始化 Layer 链 | 创建 WorkingLayer 挂载时自动创建 base + working | 挂载点有完整的 layer 链 |
| 获取 Working Layer | 获取挂载点的当前工作层 | 正确返回 is_working=true 的 layer |
| 单点 Snapshot | Snapshot 单个挂载点 | 创建新 working layer，旧的变为 snapshot |
| 批量 Snapshot | Snapshot 多个挂载点 | 事务性操作，部分成功时全部回滚 |
| Skip Unchanged | 跳过无变化的挂载点 | 无变化时不创建空 snapshot |
| Layer 链查询 | 查询挂载点的完整历史 | 返回从 base 到 working 的完整链 |

## 测试要求

### 单元测试 (target: 20+)

1. **LayerChain 结构** (5 tests)
   - 空链
   - 只有 base + working
   - 多个 snapshot

2. **Snapshot 逻辑** (8 tests)
   - 正常 snapshot
   - 已有同名 snapshot
   - 挂载点不存在
   - 非 WorkingLayer 挂载点

3. **变化检测** (7 tests)
   - 无变化检测
   - 有变化检测
   - 空 working layer 检测

### 集成测试 (target: 20+)

1. **Layer 链生命周期** (8 tests)
   - 创建 WorkingLayer 挂载点自动初始化
   - 写入后 snapshot
   - 多次 snapshot
   - 删除挂载点级联删除 layer 链

2. **批量 Snapshot** (7 tests)
   - 多个挂载点同时 snapshot
   - skip_unchanged 生效
   - 部分挂载点不存在时失败

3. **与 FileSystem 集成** (5 tests)
   - 写入到正确的 working layer
   - COW 写入
   - 读取跨 layer 数据

## 文件清单

```
src/
├── storage/
│   └── layer.rs                # 更新：新增方法
├── composition/
│   ├── layer_chain.rs          # Layer 链管理
│   └── resolver.rs             # 更新：集成 layer 链
├── fs/
│   └── operations.rs           # 更新：集成 layer 链写入
migrations/
└── YYYYMMDDHHMMSS_update_layers_for_mounts.sql
```

## 不包含（后续 Task）

- HTTP API (Task 22)
- CLI 命令 (Task 23)

## 完成标准

- [ ] 数据库 migration 文件创建并可执行
- [ ] Layer 数据结构更新（添加 mount_entry_id, is_working）
- [ ] LayerRepository 更新（新增挂载点相关方法）
- [ ] LayerChainManager 实现
- [ ] FileSystem 集成更新
- [ ] 20+ 单元测试通过
- [ ] 20+ 集成测试通过
- [ ] cargo fmt 通过
- [ ] cargo clippy 通过
- [ ] 测试覆盖率 > 80%

## 预计工作量

3-4 天

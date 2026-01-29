# Spec 18: 文件系统组合 (Filesystem Composition)

## 概述

Tarbox 支持将多个来源的文件系统组合成一个 Tenant 的统一视图。这类似于 Docker 的 overlay 但更灵活——可以指定不同路径使用不同的数据源。

## 设计动机

### bubblewrap 的局限性

之前计划使用 bubblewrap 在容器层实现目录挂载，但存在以下问题：

1. **无法跨层组合**：bubblewrap 只能绑定宿主机目录，无法从 Tarbox 的不同 Layer 组合
2. **无法共享 Layer**：不同 Tenant 无法共享同一个只读 Layer（如预训练模型）
3. **运行时限制**：需要容器支持，裸机部署困难
4. **动态性差**：挂载配置在容器启动时固定，运行时无法调整

### 新需求

AI Agent 场景需要：
- 从宿主机挂载系统目录（`/bin`, `/usr` 等）
- 从其他 Tenant 的 Layer 挂载预训练模型（跨租户共享）
- 从其他 Tenant 的 Layer 挂载数据集
- 自己的工作空间（可写层）
- 运行时可以切换数据源

## 设计约束

### 路径不可嵌套、不可冲突

**关键约束**：同一个 Tenant 的挂载路径之间**不允许嵌套和冲突**。

```
❌ 不允许的配置：
MountEntry 1: virtual_path="/data"
MountEntry 2: virtual_path="/data/models"     # 错误：嵌套在 /data 下

MountEntry 1: virtual_path="/usr"
MountEntry 2: virtual_path="/usr"             # 错误：路径冲突

MountEntry 1: virtual_path="/config.yaml"     # 单文件挂载
MountEntry 2: virtual_path="/config.yaml"     # 错误：路径冲突

✅ 允许的配置：
MountEntry 1: virtual_path="/data"
MountEntry 2: virtual_path="/models"          # OK：平级目录
MountEntry 3: virtual_path="/usr"             # OK：平级目录
MountEntry 4: virtual_path="/config.yaml"     # OK：单文件挂载
```

**原因**：
1. **简化路径解析**：无需处理嵌套优先级和覆盖逻辑
2. **避免歧义**：明确每个路径由哪个挂载负责
3. **性能**：O(n) 前缀匹配即可，无需复杂的最长前缀匹配
4. **可预测**：用户配置即所得，无隐式覆盖行为

**验证时机**：在 `add_mount()` 时检查，冲突则返回错误 `MountPathConflict`。

### 支持单文件挂载

除了目录，还支持挂载单个文件：

```rust
// 挂载单个配置文件
MountEntry {
    virtual_path: "/etc/config.yaml",      // 单文件路径
    source: MountSource::Host { 
        path: "/host/configs/app.yaml" 
    },
    mode: MountMode::ReadOnly,
    is_file: true,                         // 标记为文件挂载
}

// 挂载其他 Tenant 层中的单个模型文件
MountEntry {
    virtual_path: "/models/bert.bin",
    source: MountSource::Layer {
        tenant_id: model_hub_tenant,
        layer_id: bert_layer,
        subpath: Some("/weights/model.bin"),
    },
    mode: MountMode::ReadOnly,
    is_file: true,
}
```

**单文件挂载规则**：
- `is_file: true` 表示挂载的是文件而非目录
- 路径解析时精确匹配，不做前缀匹配
- 不能在单文件挂载路径下创建子文件/目录
- Host 源必须指向实际文件（不是目录）
- Layer 源的 subpath 必须指向文件

## 核心概念

### 挂载点级别的 Layer 链

**关键设计**：每个 `WorkingLayer` 类型的挂载点都有**独立的 layer 链**。

```
Tenant: agent-001
├── /memory   -> WorkingLayer (独立的 layer 链 A)
│   └── base -> snap1 -> snap2 -> snap3 -> working
├── /workspace -> WorkingLayer (独立的 layer 链 B)  
│   └── base -> working  (没改过，没有 snapshot)
├── /claude.md -> WorkingLayer (独立的 layer 链 C，单文件)
│   └── base -> snap1 -> working
├── /usr      -> Host:/usr (只读，无 layer)
└── /models   -> Published:bert-v1 (只读，无 layer)
```

**好处**：
1. **细粒度 Snapshot**：只 snap 有变化的挂载点，没变化的不产生新 layer
2. **细粒度发布**：可以只发布 `/memory` 的 layer，不暴露 `/workspace`
3. **存储高效**：workspace 没变就不产生新 layer
4. **语义清晰**：每个挂载点是独立的数据单元

### Layer 所有权模型

**每个 Layer 都属于一个挂载点（mount_entry_id）**，挂载点属于 Tenant。

```rust
pub struct Layer {
    pub layer_id: Uuid,
    pub mount_entry_id: Uuid,  // 所属的挂载点
    pub tenant_id: Uuid,       // 所属的 Tenant
    pub parent_layer_id: Option<Uuid>,
    pub name: Option<String>,
    pub is_working: bool,      // 是否是当前工作层
    pub created_at: DateTime<Utc>,
}
```

**权限规则**：
- **Owner（挂载点所属 tenant）**：可读、可写
- **其他 Tenant**：只能在发布后只读访问

### Mount Source（挂载源）

```rust
pub enum MountSource {
    /// 宿主机目录/文件
    Host {
        path: PathBuf,
    },
    
    /// 其他挂载点的 Layer（只读访问已发布的）
    Layer {
        source_mount_id: Uuid,    // 源挂载点
        layer_id: Option<Uuid>,   // 特定 layer，None 表示 working layer
        subpath: Option<PathBuf>,
    },
    
    /// 通过发布名称引用
    Published {
        publish_name: String,
        subpath: Option<PathBuf>,
    },
    
    /// 当前挂载点自己的工作层（可写）
    /// 每个 WorkingLayer 挂载点有独立的 layer 链
    WorkingLayer,
}
```

### Mount Mode（挂载模式）

```rust
pub enum MountMode {
    /// 只读 - 任何源都可以
    ReadOnly,
    
    /// 读写 - 仅限 Host 或 WorkingLayer
    ReadWrite,
    
    /// 写时复制 - 读取来自源，写入到当前挂载点的 WorkingLayer
    CopyOnWrite,
}
```

**模式与源的组合规则**：

| Source | ro | rw | cow |
|--------|----|----|-----|
| Host | ✅ | ✅ | ✅ |
| Layer (owner) | ✅ | ❌* | ✅ |
| Layer (非 owner) | ✅ | ❌ | ✅ |
| WorkingLayer | ✅ | ✅ | N/A |

*注：owner 对自己的 layer 写入应该通过 WorkingLayer，不是直接写历史 layer。

### Mount Entry（挂载条目）

```rust
pub struct MountEntry {
    pub mount_entry_id: Uuid,
    pub tenant_id: Uuid,
    
    /// 挂载点名称（用于 API 引用，同一 tenant 内唯一）
    /// 例如: "memory", "workspace", "claude-md"
    pub name: String,
    
    /// 虚拟路径（实际挂载位置）
    pub virtual_path: PathBuf,
    
    pub source: MountSource,
    pub mode: MountMode,
    pub is_file: bool,
    pub enabled: bool,
    
    // WorkingLayer 类型的挂载点有自己的 layer 链
    pub current_layer_id: Option<Uuid>,  // 当前工作层 ID
    
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

**名称 vs 路径**：
- `name`：用于 API 引用（snapshot、publish 等），简短易记
- `virtual_path`：实际挂载位置，可以是任意路径

示例：
| name | virtual_path |
|------|--------------|
| `memory` | `/memory` |
| `workspace` | `/workspace` |
| `claude-md` | `/claude.md` |
| `system-usr` | `/usr` |

### Tenant Composition（租户组合配置）

```rust
pub struct TenantComposition {
    pub tenant_id: Uuid,
    pub mounts: Vec<MountEntry>,
    // 注意：不再有全局 working_layer_id，每个挂载点有自己的 layer 链
}
```

## Layer 与 Snapshot 机制

### 挂载点级别的 Layer 链

每个 `WorkingLayer` 类型的挂载点维护独立的 layer 链：

```rust
pub struct Layer {
    pub layer_id: Uuid,
    pub mount_entry_id: Uuid,      // 所属挂载点
    pub tenant_id: Uuid,
    pub parent_layer_id: Option<Uuid>,
    pub name: Option<String>,      // snapshot 名称
    pub is_working: bool,          // true = 当前工作层
    pub created_at: DateTime<Utc>,
}
```

### Snapshot 操作

Snapshot 是针对**特定挂载点**的操作，使用挂载点**名称**引用：

```bash
# 只 snapshot memory 挂载点
tarbox snapshot --tenant agent-001 --mount memory --name "memory-v1"

# snapshot 多个挂载点
tarbox snapshot --tenant agent-001 --mount memory --mount workspace --name "checkpoint-1"

# snapshot 所有 WorkingLayer 挂载点
tarbox snapshot --tenant agent-001 --all --name "full-checkpoint"
```

**Snapshot 行为**：
- 将当前 working layer 变为只读 snapshot
- 创建新的 working layer 作为子层
- 如果挂载点内容没有变化，可以跳过（不创建空 snapshot）

## Layer 发布机制

Owner 可以将自己**某个挂载点**的 Layer 发布，让其他 Tenant 只读访问。

### 发布类型

```rust
pub struct PublishInput {
    pub mount_entry_id: Uuid,        // 要发布的挂载点
    pub publish_name: String,        // 公开名称，全局唯一
    pub description: Option<String>,
    pub target: PublishTarget,
    pub scope: PublishScope,
}

pub enum PublishTarget {
    /// 发布特定的 snapshot（内容固定）
    Layer(Uuid),
    
    /// 发布当前 working layer（实时跟随）
    WorkingLayer,
}

pub enum PublishScope {
    Public,                          // 所有 tenant 可见
    AllowList(Vec<Uuid>),           // 仅指定 tenant 可见
}
```

**示例**：
```bash
# 只发布 memory 挂载点的 working layer
tarbox publish --tenant agent-001 --mount memory \
    --target working_layer \
    --name "agent1-memory" \
    --scope public

# 发布 memory 的特定 snapshot
tarbox publish --tenant agent-001 --mount memory \
    --layer memory-v1 \
    --name "agent1-memory-stable"
```

**Working Layer 发布的特点**：
- 实时性：写入后其他 tenant 立即可见
- 无需每次创建 snapshot
- 只发布指定挂载点，其他挂载点不受影响

### 数据库设计

```sql
-- layers 表：每个挂载点的 layer 链
CREATE TABLE layers (
    layer_id UUID PRIMARY KEY,
    mount_entry_id UUID NOT NULL REFERENCES mount_entries(mount_entry_id) ON DELETE CASCADE,
    tenant_id UUID NOT NULL REFERENCES tenants(tenant_id),
    parent_layer_id UUID REFERENCES layers(layer_id),
    name VARCHAR(255),              -- snapshot 名称
    is_working BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    
    -- 每个挂载点只能有一个 working layer
    CONSTRAINT unique_working_layer UNIQUE (mount_entry_id, is_working) 
        WHERE is_working = true
);

CREATE INDEX idx_layers_mount ON layers(mount_entry_id);
CREATE INDEX idx_layers_tenant ON layers(tenant_id);
CREATE INDEX idx_layers_parent ON layers(parent_layer_id);

-- 发布配置表
CREATE TABLE published_mounts (
    publish_id UUID PRIMARY KEY,
    mount_entry_id UUID NOT NULL REFERENCES mount_entries(mount_entry_id),
    tenant_id UUID NOT NULL REFERENCES tenants(tenant_id),
    publish_name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    
    -- 发布目标：'layer' 或 'working_layer'
    target_type VARCHAR(20) NOT NULL,
    layer_id UUID REFERENCES layers(layer_id),  -- NULL if working_layer
    
    -- 访问控制
    scope VARCHAR(20) NOT NULL DEFAULT 'public',
    allowed_tenants UUID[],
    
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    
    CONSTRAINT valid_target CHECK (
        (target_type = 'layer' AND layer_id IS NOT NULL) OR
        (target_type = 'working_layer' AND layer_id IS NULL)
    )
);

CREATE INDEX idx_published_mounts_name ON published_mounts(publish_name);
CREATE INDEX idx_published_mounts_tenant ON published_mounts(tenant_id);
```

### mount_entries 表

```sql
CREATE TABLE mount_entries (
    mount_entry_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    
    -- 挂载点名称（用于 API 引用，同一 tenant 内唯一）
    name VARCHAR(255) NOT NULL,
    
    -- 虚拟路径（实际挂载位置）
    virtual_path TEXT NOT NULL,
    is_file BOOLEAN NOT NULL DEFAULT false,
    
    -- 源类型: 'host', 'layer', 'working_layer', 'published'
    source_type VARCHAR(20) NOT NULL,
    
    -- Host source
    host_path TEXT,
    
    -- Layer source (直接引用)
    source_mount_id UUID REFERENCES mount_entries(mount_entry_id),
    source_layer_id UUID REFERENCES layers(layer_id),
    source_subpath TEXT,
    
    -- Published source (通过名称引用)
    source_publish_name VARCHAR(255),
    
    -- WorkingLayer 的当前工作层 ID
    current_layer_id UUID REFERENCES layers(layer_id),
    
    -- 访问模式: 'ro', 'rw', 'cow'
    mode VARCHAR(3) NOT NULL DEFAULT 'ro',
    
    enabled BOOLEAN NOT NULL DEFAULT true,
    metadata JSONB,
    
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    
    CONSTRAINT valid_source CHECK (
        (source_type = 'host' AND host_path IS NOT NULL) OR
        (source_type = 'layer' AND source_mount_id IS NOT NULL) OR
        (source_type = 'published' AND source_publish_name IS NOT NULL) OR
        (source_type = 'working_layer')
    ),
    
    -- 同一 tenant 内名称唯一
    UNIQUE(tenant_id, name),
    -- 同一 tenant 内路径唯一
    UNIQUE(tenant_id, virtual_path)
);

CREATE INDEX idx_mount_entries_tenant ON mount_entries(tenant_id);
CREATE INDEX idx_mount_entries_name ON mount_entries(tenant_id, name);
CREATE INDEX idx_mount_entries_source ON mount_entries(source_mount_id);
```

## 路径解析

### 解析规则

由于路径不可嵌套，解析非常简单：

```
查找文件 /models/bert/config.json:

1. 遍历所有 MountEntry
2. 对于文件挂载（is_file=true）：精确匹配 virtual_path
3. 对于目录挂载（is_file=false）：前缀匹配 virtual_path
4. 找到匹配后，根据 source 类型读取数据

示例配置：
MountEntry 1: virtual_path="/workspace",   source=WorkingLayer,                 mode=rw
MountEntry 2: virtual_path="/models",      source=Layer{model-hub, bert-v1},    mode=ro
MountEntry 3: virtual_path="/usr",         source=Host{/usr},                   mode=ro
MountEntry 4: virtual_path="/claude.md",   source=WorkingLayer (file),          mode=rw, is_file=true

查找 /models/bert/config.json:
1. "/workspace" 不匹配
2. "/models" 匹配！
3. 在 model-hub tenant 的 bert-v1 layer 中查找 /bert/config.json

查找 /claude.md:
1. 精确匹配文件挂载 "/claude.md"
2. 从当前 tenant 的 working layer 读取
```

### 写入规则

```
写入文件时检查权限：

1. mode=ro → 返回 EROFS
2. mode=rw:
   - Host source → 直接写入宿主机
   - WorkingLayer → 写入当前 tenant 的工作层
   - Layer source → 错误（layer 不支持 rw）
3. mode=cow:
   - 写入到当前 tenant 的 WorkingLayer
   - 原始源不受影响
```

## API 设计

> 完整 API 定义参见 [spec/06-api-design.md](./06-api-design.md)

### REST API

```http
# 批量设置挂载配置
PUT /tenants/{tenant_id}/mounts
Content-Type: application/json
{
    "mounts": [
        {"virtual_path": "/workspace", "source_type": "working_layer", "mode": "rw"},
        {"virtual_path": "/models", "source_type": "layer", "source_tenant_id": "...", "source_layer_id": "...", "mode": "ro"},
        {"virtual_path": "/claude.md", "is_file": true, "source_type": "working_layer", "mode": "rw"}
    ]
}

# 导入挂载配置（简化格式）
POST /tenants/{tenant_id}/mounts/import
Content-Type: application/json

# 导出挂载配置
GET /tenants/{tenant_id}/mounts/export

# 获取当前挂载配置
GET /tenants/{tenant_id}/mounts

# 发布 Layer
POST /tenants/{tenant_id}/layers/{layer_id}/publish
{
    "publish_name": "bert-base-v1",
    "description": "BERT base model",
    "scope": "public"
}

# 取消发布
DELETE /tenants/{tenant_id}/layers/{layer_id}/publish

# 获取已发布的 Layer 列表
GET /published-layers
Query Parameters:
  - scope: public|all
  - owner_tenant_id: 过滤特定 owner
```

**注意**：所有 HTTP 接口均使用 JSON 格式。TOML 仅用于 CLI 的配置文件。

### CLI

```bash
# 应用配置文件
tarbox mount apply --tenant <tenant-id> --config mounts.toml

# 导出当前配置
tarbox mount export --tenant <tenant-id> --output mounts.toml

# 查看当前挂载
tarbox mount list --tenant <tenant-id>

# 验证配置文件
tarbox mount validate --config mounts.toml

# 发布 Layer
tarbox layer publish --tenant <tenant-id> --layer <layer-id> \
    --name "bert-base-v1" \
    --description "BERT base model" \
    --scope public

# 取消发布
tarbox layer unpublish --tenant <tenant-id> --layer <layer-id>

# 查看已发布的 Layer
tarbox layer list-published
```

### 配置文件格式 (TOML)

```toml
# mounts.toml

[[mounts]]
path = "/workspace"
source = "working_layer"
mode = "rw"

[[mounts]]
path = "/claude.md"
file = true
source = "working_layer"
mode = "rw"

[[mounts]]
path = "/models"
source = "published:bert-base-v1"    # 引用已发布的 layer
mode = "ro"

[[mounts]]
path = "/memory"
source = "layer:memory-agent:current"  # 直接引用 tenant:layer
mode = "ro"

[[mounts]]
path = "/usr"
source = "host:/usr"
mode = "ro"

[[mounts]]
path = "/bin"
source = "host:/bin"
mode = "ro"
```

**Source 格式**：
| 格式 | 说明 | 示例 |
|------|------|------|
| `working_layer` | 当前 Tenant 的工作层 | `source = "working_layer"` |
| `host:<path>` | 宿主机目录/文件 | `source = "host:/usr"` |
| `layer:<tenant>:<layer>` | 指定 Tenant 的 Layer | `source = "layer:model-hub:bert-v1"` |
| `published:<name>` | 已发布的 Layer（按名称） | `source = "published:bert-base-v1"` |

## 内部 Rust API

```rust
#[async_trait]
pub trait CompositionManager {
    /// 获取 Tenant 的组合配置
    async fn get_composition(&self, tenant_id: Uuid) -> Result<TenantComposition>;
    
    /// 添加挂载
    async fn add_mount(&self, tenant_id: Uuid, entry: CreateMountEntry) -> Result<MountEntry>;
    
    /// 更新挂载
    async fn update_mount(&self, mount_entry_id: Uuid, update: UpdateMountEntry) -> Result<MountEntry>;
    
    /// 删除挂载
    async fn remove_mount(&self, mount_entry_id: Uuid) -> Result<()>;
    
    /// 解析路径
    async fn resolve_path(&self, tenant_id: Uuid, path: &Path) -> Result<ResolvedPath>;
}

#[async_trait]
pub trait LayerPublisher {
    /// 发布 Layer
    async fn publish_layer(&self, input: PublishLayerInput) -> Result<()>;
    
    /// 取消发布
    async fn unpublish_layer(&self, layer_id: Uuid) -> Result<()>;
    
    /// 获取已发布的 Layer 列表
    async fn list_published_layers(&self, filter: PublishedLayerFilter) -> Result<Vec<PublishedLayer>>;
    
    /// 检查访问权限
    async fn check_layer_access(&self, layer_id: Uuid, accessor_tenant_id: Uuid) -> Result<bool>;
}
```

## 实现细节

### 权限检查

```rust
async fn add_mount(&self, tenant_id: Uuid, entry: CreateMountEntry) -> Result<MountEntry> {
    // 检查 Layer source 的访问权限
    if let MountSource::Layer { tenant_id: source_tenant, layer_id, .. } = &entry.source {
        if *source_tenant != tenant_id {
            // 非 owner，检查是否已发布
            let layer = self.get_layer(*layer_id).await?;
            
            if !layer.is_published {
                return Err(anyhow!("Access denied: layer not published"));
            }
            
            // 检查 scope
            match layer.publish_scope {
                PublishScope::Public => { /* OK */ }
                PublishScope::AllowList(allowed) => {
                    if !allowed.contains(&tenant_id) {
                        return Err(anyhow!("Access denied: tenant not in allow list"));
                    }
                }
            }
        }
        
        // 非 owner 不能用 rw 模式
        if *source_tenant != tenant_id && entry.mode == MountMode::ReadWrite {
            return Err(anyhow!("Cannot mount other tenant's layer as read-write"));
        }
    }
    
    // ... 创建挂载 ...
}
```

### Copy-on-Write 实现

```rust
async fn write_file(&self, tenant_id: Uuid, path: &Path, data: &[u8]) -> Result<()> {
    let resolved = self.resolve_path(tenant_id, path).await?;
    
    match resolved.mount_entry.mode {
        MountMode::ReadOnly => {
            return Err(anyhow!("EROFS: Read-only mount"));
        }
        
        MountMode::ReadWrite => {
            match resolved.source {
                ResolvedSource::Host { path } => {
                    std::fs::write(path, data)?;
                }
                ResolvedSource::WorkingLayer { path } => {
                    self.write_to_working_layer(tenant_id, &path, data).await?;
                }
                ResolvedSource::Layer { .. } => {
                    return Err(anyhow!("Cannot write directly to layer"));
                }
            }
        }
        
        MountMode::CopyOnWrite => {
            // 写入到 WorkingLayer，保持相同的虚拟路径
            self.write_to_working_layer(tenant_id, path, data).await?;
        }
    }
    
    Ok(())
}
```

## 使用场景

### 场景：多 Agent 共享实时 Memory

```
需求：
- Agent 1 有自己的 claude.md（可写，独立 layer 链）
- Agent 1 管理一个 memory（可写，独立 layer 链，实时共享给其他 agent）
- Agent 1 有自己的 workspace（可写，独立 layer 链）
- Agent 1 需要系统的 /usr 和 /bin

- Agent 2 有自己的 claude.md（可写）
- Agent 2 读取 Agent 1 的 memory（只读，实时看到 Agent 1 的更新）
- Agent 2 有自己的 workspace（可写）

关键点：Agent 1 的 /memory、/workspace、/claude.md 各自有独立的 layer 链！
只发布 /memory，不会暴露 /workspace 和 /claude.md。

实现：

1. 创建 agent-001 tenant

2. Agent 1 配置 (agent-001 tenant):
   # 每个 working_layer 挂载点有独立的 layer 链
   # name 用于 API 引用，path 是实际挂载位置
   [[mounts]]
   name = "claude-md"           # API 引用名称
   path = "/claude.md"          # 实际挂载路径
   file = true
   source = "working_layer"     # layer 链 A
   mode = "rw"

   [[mounts]]
   name = "memory"              # API 引用名称
   path = "/memory"
   source = "working_layer"     # layer 链 B (独立!)
   mode = "rw"

   [[mounts]]
   name = "workspace"           # API 引用名称
   path = "/workspace"
   source = "working_layer"     # layer 链 C (独立!)
   mode = "rw"

   [[mounts]]
   name = "system-usr"
   path = "/usr"
   source = "host:/usr"
   mode = "ro"

   [[mounts]]
   name = "system-bin"
   path = "/bin"
   source = "host:/bin"
   mode = "ro"

3. Agent 1 只发布 memory 挂载点的 working layer:
   tarbox publish --tenant agent-001 --mount memory \
       --target working_layer \
       --name "agent1-memory" \
       --scope public
   
   # 使用挂载点名称 "memory"，不是路径 "/memory"
   # 只有 memory 被发布，workspace 和 claude-md 不受影响！

4. Agent 1 可以单独 snapshot memory:
   tarbox snapshot --tenant agent-001 --mount memory --name "memory-v1"
   # workspace 没改过，不需要 snapshot，不会产生空 layer

5. Agent 2 配置 (agent-002 tenant):
   [[mounts]]
   name = "claude-md"
   path = "/claude.md"
   file = true
   source = "working_layer"     # Agent 2 自己的 layer 链
   mode = "rw"

   [[mounts]]
   name = "shared-memory"       # Agent 2 自己的名称
   path = "/memory"             # 挂载到同样的路径
   source = "published:agent1-memory"  # 引用 Agent 1 发布的
   mode = "ro"                  # 只读！

   [[mounts]]
   name = "workspace"
   path = "/workspace"
   source = "working_layer"     # Agent 2 自己的 layer 链
   mode = "rw"

结果：
- Agent 1 写入 /memory/data.json → 写入 /memory 挂载点的 working layer
- Agent 2 读取 /memory/data.json → 实时读取 Agent 1 的 /memory working layer（立即可见）
- Agent 2 写入 /memory/xxx → EROFS（只读）
- 各自的 /claude.md 和 /workspace 互不影响（独立 layer 链）
- Agent 1 可以只 snapshot /memory，不影响 /workspace
- Agent 1 只发布了 /memory，/workspace 和 /claude.md 不会被暴露
```

### 场景：细粒度 Snapshot

```
Agent 1 的挂载点各自有独立的 layer 链：

memory (path=/memory) 的 layer 链:
  base -> snap1 -> snap2 -> snap3 -> working  (频繁更新)

workspace (path=/workspace) 的 layer 链:
  base -> working  (很少变化，没有 snapshot)

claude-md (path=/claude.md) 的 layer 链:
  base -> snap1 -> working  (偶尔更新)

操作示例：

# 只 snapshot memory（因为它变化最多）
# 使用挂载点名称 "memory"，不是路径
tarbox snapshot --tenant agent-001 --mount memory --name "memory-after-task-1"

# workspace 没变，不需要 snapshot

# snapshot 所有有变化的挂载点
tarbox snapshot --tenant agent-001 --all --name "checkpoint-1" --skip-unchanged
# 结果：只有 memory 产生新 snapshot，workspace 跳过
```

### 场景：模型共享

```
Tenant: model-hub
├── /bert-base   -> WorkingLayer (独立 layer 链，发布为 "bert-base-v1")
├── /gpt2        -> WorkingLayer (独立 layer 链，发布为 "gpt2-medium-v1")
└── Owner 可以单独更新每个模型，创建新版本

Tenant: agent-001
├── /models -> published:bert-base-v1 [ro]
└── /workspace -> working_layer [rw]

Tenant: agent-002
├── /models/bert -> published:bert-base-v1 [ro]
├── /models/gpt2 -> published:gpt2-medium-v1 [ro]
└── /workspace -> working_layer [rw]

优势：
- 模型只存一份
- model-hub 可以单独更新 bert 而不影响 gpt2
- 使用者只读访问，安全隔离
```

## 安全考虑

### 访问控制

1. **Layer 访问**：只有 owner 可写，其他 tenant 需要 is_published=true 才能读
2. **Host 目录**：配置白名单，限制可挂载的宿主机路径
3. **审计**：记录跨 tenant 访问

### Host 目录白名单

```toml
[security.host_mounts]
allowed_paths = [
    "/usr",
    "/bin",
    "/lib",
    "/lib64",
    "/var/tarbox/shared",
]

denied_paths = [
    "/etc/passwd",
    "/etc/shadow",
]
```

## 与现有系统的集成

### 与 Layer 系统集成

- Layer 已有 tenant_id 字段，天然支持 owner 模型
- 新增 is_published, publish_name 等字段
- 发布不影响 layer 的 COW 和 checkpoint 功能

### 与 FUSE 集成

- FUSE 调用 CompositionManager.resolve_path()
- 根据解析结果决定数据来源和写入目标

## 迁移计划

### Phase 1: 基础实现
- mount_entries 表
- 基本的路径解析
- Host 和 WorkingLayer 支持

### Phase 2: Layer 发布
- 扩展 layers 表
- 发布/取消发布 API
- 跨 tenant 只读访问

### Phase 3: 高级功能
- COW 和 Whiteout
- 动态挂载
- 性能优化

## 废弃说明

本 spec 取代 spec/12-native-mounting.md。

与之前 shared_layers 表设计的区别：
- **之前**：单独的 shared_layers 表，需要额外管理
- **现在**：直接在 layers 表添加发布字段，更简单
- **之前**：共享层概念独立于 layer
- **现在**：发布只是 layer 的一个属性，owner 不变

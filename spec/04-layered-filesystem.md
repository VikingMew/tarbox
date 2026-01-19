# 分层文件系统设计

## 实现状态

**✅ 已实现** (Task 06, 2026-01-19)

核心数据库表和操作已完成：
- ✅ layers表（parent_layer_id形成链式结构）
- ✅ layer_entries表（记录文件变更）
- ✅ tenant_current_layer表（跟踪租户当前层）
- ✅ 递归CTE查询层链（get_layer_chain函数）
- ✅ LayerRepository trait（9个方法）
- ✅ LayerOperations实现（层CRUD、层链查询、条目管理）
- ✅ 10个集成测试
- ⚠️ 文件系统层集成待实现（将在Task 08实现）

**代码位置**:
- 迁移: `migrations/20260118000003_create_layers.sql`
- 模型: `src/storage/models.rs` (Layer, LayerEntry, TenantCurrentLayer, LayerStatus, ChangeType)
- Trait: `src/storage/traits.rs` (LayerRepository)
- 实现: `src/storage/layer.rs` (LayerOperations)
- 测试: `tests/layer_integration_test.rs`

## 概述

Tarbox 的分层系统类似于 Docker 镜像层，采用写时复制（Copy-on-Write）技术，支持文件系统的版本化和快速回溯。

## 设计理念

### 核心概念

```
线性历史模型（类比 Git 但更简单）：

base -> checkpoint-1 -> checkpoint-2 -> checkpoint-3 [current]

特点：
- 单向链表结构，每层有唯一父层
- 每层记录相对于父层的变化
- 历史层只读，当前层可写
- 可以自由切换到任何历史层查看
- 在历史层创建新层时，才删除"未来"的层
- 联合视图：从 base 到 current 层层叠加
```

### 与传统快照的区别

```
传统快照：
- 完整的时间点副本
- 独立存在
- 占用大量空间

分层系统：
- 增量式记录变化
- 层层依赖（栈式结构）
- 空间高效（共享不变数据）
- 支持层间跳转
```

## 数据模型

### Layer（层）

```
Layer {
    layer_id: UUID,                // 层 ID
    tenant_id: UUID,               // 租户 ID（关键：多租户隔离）
    parent_layer_id: Option<UUID>, // 父层 ID（None 表示基础层）
    created_at: Timestamp,         // 创建时间
    name: String,                  // 层名称（如 "checkpoint-001"）
    description: String,           // 描述
    is_readonly: bool,             // 是否只读
    
    // 层的统计信息
    file_count: i64,               // 此层变化的文件数
    added_files: i64,              // 新增文件数
    modified_files: i64,           // 修改文件数
    deleted_files: i64,            // 删除文件数
    total_size: i64,               // 此层数据大小
    
    // 元数据
    metadata: JSONB,               // 额外元数据（如标签）
}

注意：
- 每个租户有独立的层历史
- 租户之间的层完全隔离
- 层 ID 可以在不同租户间重复，通过 tenant_id 区分
```

### LayerEntry（层条目）

```
LayerEntry {
    entry_id: UUID,
    tenant_id: UUID,               // 租户 ID
    layer_id: UUID,                // 所属层
    inode_id: i64,                 // 文件 inode
    path: String,                  // 文件路径
    
    // 操作类型
    operation: enum {
        Add,      // 新增文件
        Modify,   // 修改文件
        Delete,   // 删除文件（墓碑标记）
    },
    
    // 变化记录
    old_metadata: Option<JSONB>,   // 修改前的元数据
    new_metadata: JSONB,           // 修改后的元数据
    
    // 数据引用
    data_blocks: Vec<BlockRef>,    // 数据块引用
}
```

### 联合视图

```
读取文件时的查找顺序：
1. 从最上层（当前层）开始查找
2. 如果找到 Delete 操作 -> 文件不存在
3. 如果找到 Add/Modify -> 返回该层的数据
4. 如果未找到 -> 继续向下查找父层
5. 直到找到或到达基础层

示例：
Layer 3: /data/file.txt -> Delete
Layer 2: /data/file.txt -> Modify (version 2)
Layer 1: /data/file.txt -> Add (version 1)
Layer 0: (无此文件)

查询 /data/file.txt:
-> 在 Layer 3 发现 Delete -> 返回文件不存在
```

## 写时复制（COW）

### 写入操作

```
场景：在 Layer 3 修改文件 /data/file.txt

流程：
1. 查找文件当前状态（可能在 Layer 1）
2. 在 Layer 3 创建新的 LayerEntry
   - operation = Modify
   - old_metadata = Layer 1 的元数据
   - new_metadata = 新的元数据
3. 复制需要修改的数据块（COW）
4. 在新数据块上进行修改
5. 原始数据块保持不变（保护父层）

优势：
- 父层永远不变
- 支持快速回滚
- 数据共享（未修改的块）
```

### 删除操作

```
场景：删除文件 /data/file.txt

流程：
1. 不实际删除任何数据
2. 在当前层创建 Delete 条目（墓碑）
3. 读取时遇到墓碑返回文件不存在

优势：
- 可以回滚删除操作
- 父层数据不受影响
- 历史可追溯
```

## 层操作

### 创建新层（Checkpoint）

```rust
// 基于当前层创建新的检查点
fn create_checkpoint(
    parent_layer_id: UUID,
    name: String,
    description: String,
) -> Result<Layer> {
    // 1. 将当前层标记为只读
    set_readonly(parent_layer_id, true);
    
    // 2. 创建新层
    let new_layer = Layer {
        layer_id: generate_uuid(),
        parent_layer_id: Some(parent_layer_id),
        name,
        description,
        is_readonly: false,
        created_at: now(),
        ...
    };
    
    // 3. 新层初始为空（继承父层所有内容）
    insert_layer(new_layer);
    
    // 4. 切换当前工作层
    set_active_layer(new_layer.layer_id);
    
    Ok(new_layer)
}
```

### 层间跳转

```rust
// 跳转到指定层（类似 git checkout）
fn switch_to_layer(target_layer_id: UUID) -> Result<()> {
    // 1. 验证目标层存在
    let target_layer = get_layer(target_layer_id)?;
    
    // 2. 如果当前层有未保存的修改
    if has_unsaved_changes() {
        // 选项 1: 提示用户先创建检查点
        // 选项 2: 自动创建临时检查点
        // 选项 3: 丢弃修改（需要确认）
        return Err("Unsaved changes");
    }
    
    // 3. 切换活动层
    set_active_layer(target_layer_id);
    
    // 4. 刷新缓存（重新构建联合视图）
    invalidate_cache();
    
    // 5. 如果目标层是只读的，可以选择：
    //    a) 自动创建新的可写层
    //    b) 以只读模式工作
    if target_layer.is_readonly {
        let new_layer = create_checkpoint(
            target_layer_id,
            format!("{}-branch", target_layer.name),
            "Auto-created branch"
        )?;
        set_active_layer(new_layer.layer_id);
    }
    
    Ok(())
}
```

### 层合并（Squash）

```rust
// 合并多层为一层（类似 Docker squash）
fn squash_layers(
    layer_ids: Vec<UUID>,
    new_layer_name: String,
) -> Result<Layer> {
    // 1. 验证层的连续性（父子关系）
    validate_layer_chain(&layer_ids)?;
    
    // 2. 创建新层
    let base = layer_ids[0];
    let new_layer = Layer {
        layer_id: generate_uuid(),
        parent_layer_id: get_layer(base).parent_layer_id,
        name: new_layer_name,
        is_readonly: true,
        ...
    };
    
    // 3. 合并所有层的变化
    for layer_id in layer_ids {
        let entries = get_layer_entries(layer_id);
        for entry in entries {
            // 应用变化到新层
            apply_entry_to_layer(&new_layer, entry);
        }
    }
    
    // 4. 优化：去重和压缩
    //    - 如果同一文件多次修改，只保留最终状态
    //    - 如果文件先添加后删除，可以完全移除
    optimize_layer(&new_layer);
    
    // 5. 可选：删除旧层
    // delete_layers(&layer_ids);
    
    Ok(new_layer)
}
```



## 数据块共享

### 引用计数

```sql
CREATE TABLE data_blocks (
    block_id UUID PRIMARY KEY,
    data BYTEA NOT NULL,
    size INTEGER NOT NULL,
    checksum VARCHAR(64) NOT NULL,
    
    -- 引用计数
    ref_count INTEGER NOT NULL DEFAULT 0,
    
    -- 层引用
    layer_refs JSONB,  -- [{layer_id: UUID, inode_id: i64}, ...]
    
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- 当块被层引用时
UPDATE data_blocks 
SET ref_count = ref_count + 1,
    layer_refs = layer_refs || jsonb_build_object(
        'layer_id', :layer_id,
        'inode_id', :inode_id
    )
WHERE block_id = :block_id;

-- 当层被删除时
UPDATE data_blocks 
SET ref_count = ref_count - 1
WHERE block_id IN (SELECT block_id FROM layer_blocks WHERE layer_id = :layer_id);

-- 清理无引用的块
DELETE FROM data_blocks WHERE ref_count = 0;
```

### 内容寻址

```
使用内容哈希实现自动去重：

1. 二进制文件写入数据块时：
   - 计算 SHA256 checksum
   - 查询是否已存在相同 checksum 的块
   - 如果存在，直接引用（ref_count++）
   - 如果不存在，插入新块

2. 文本文件写入时：
   - 按行分块，计算每个 TextBlock 的哈希
   - 查询是否已存在相同哈希的文本块
   - 如果存在，复用 TextBlock（ref_count++）
   - 如果不存在，插入新 TextBlock
   - 层间只存储变化的行

3. 优势：
   - 相同内容的块只存储一份
   - 跨层、跨文件去重
   - 文本文件支持行级增量存储
   - 节省大量空间

4. 示例：
   二进制文件：
   Layer 1: /app/lib.so (version 1.0)
   Layer 2: /app/lib.so (version 1.0, 未修改)
   -> 两层共享相同的数据块
   
   文本文件：
   Layer 1: config.yaml (100 行)
   Layer 2: config.yaml (修改了第 50 行)
   -> Layer 2 只存储第 50 行，其他 99 行复用 Layer 1
```

## 数据库设计

### layers 表

```sql
CREATE TABLE layers (
    layer_id UUID PRIMARY KEY,
    parent_layer_id UUID REFERENCES layers(layer_id) ON DELETE RESTRICT,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    is_readonly BOOLEAN NOT NULL DEFAULT FALSE,
    
    -- 统计
    file_count BIGINT NOT NULL DEFAULT 0,
    added_files BIGINT NOT NULL DEFAULT 0,
    modified_files BIGINT NOT NULL DEFAULT 0,
    deleted_files BIGINT NOT NULL DEFAULT 0,
    total_size BIGINT NOT NULL DEFAULT 0,
    
    -- 元数据
    metadata JSONB,
    
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    
    UNIQUE(name)
);

CREATE INDEX idx_layers_parent ON layers(parent_layer_id);
CREATE INDEX idx_layers_readonly ON layers(is_readonly);
```

### layer_entries 表

```sql
CREATE TABLE layer_entries (
    entry_id UUID PRIMARY KEY,
    layer_id UUID NOT NULL REFERENCES layers(layer_id) ON DELETE CASCADE,
    inode_id BIGINT NOT NULL,
    path TEXT NOT NULL,
    
    -- 操作类型
    operation VARCHAR(10) NOT NULL CHECK (operation IN ('add', 'modify', 'delete')),
    
    -- 变化记录
    old_metadata JSONB,
    new_metadata JSONB NOT NULL,
    
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    
    UNIQUE(layer_id, path)
);

CREATE INDEX idx_layer_entries_layer ON layer_entries(layer_id);
CREATE INDEX idx_layer_entries_path ON layer_entries(layer_id, path);
CREATE INDEX idx_layer_entries_inode ON layer_entries(inode_id);
```

### layer_blocks 表

```sql
CREATE TABLE layer_blocks (
    layer_id UUID NOT NULL REFERENCES layers(layer_id) ON DELETE CASCADE,
    inode_id BIGINT NOT NULL,
    block_id UUID NOT NULL REFERENCES data_blocks(block_id) ON DELETE RESTRICT,
    block_index INTEGER NOT NULL,
    
    PRIMARY KEY (layer_id, inode_id, block_index)
);

CREATE INDEX idx_layer_blocks_layer ON layer_blocks(layer_id);
CREATE INDEX idx_layer_blocks_block ON layer_blocks(block_id);
```

### active_layer 表

```sql
CREATE TABLE active_layer (
    mount_id INTEGER PRIMARY KEY REFERENCES mount_points(mount_id),
    layer_id UUID NOT NULL REFERENCES layers(layer_id),
    switched_at TIMESTAMP NOT NULL DEFAULT NOW()
);
```

## 查询优化

### 路径查找

```sql
-- 查找文件在联合视图中的状态
WITH RECURSIVE layer_chain AS (
    -- 从当前活动层开始
    SELECT layer_id, parent_layer_id, 0 as depth
    FROM layers
    WHERE layer_id = :active_layer_id
    
    UNION ALL
    
    -- 递归查找父层
    SELECT l.layer_id, l.parent_layer_id, lc.depth + 1
    FROM layers l
    JOIN layer_chain lc ON l.layer_id = lc.parent_layer_id
)
SELECT le.*
FROM layer_entries le
JOIN layer_chain lc ON le.layer_id = lc.layer_id
WHERE le.path = :file_path
ORDER BY lc.depth ASC  -- 从上到下查找
LIMIT 1;  -- 找到第一个就停止
```

### 缓存策略

```
多级缓存：

1. 路径缓存：
   Key: (active_layer_id, path)
   Value: LayerEntry
   TTL: 无限（直到层切换）

2. 层链缓存：
   Key: layer_id
   Value: [layer_id, parent_id, grandparent_id, ...]
   TTL: 无限（层关系不变）

3. 联合视图缓存：
   Key: (active_layer_id, path)
   Value: 完整文件状态
   失效：当前层修改该路径时
```

## 使用场景

### 场景 1：AI Agent 训练流程

```
初始状态：
Layer 0 (base): 基础代码和配置

实验 1：
Layer 1: 修改超参数 -> 训练 -> 不理想

回到基础：
切换到 Layer 0 -> 创建 Layer 2

实验 2：
Layer 2: 修改模型结构 -> 训练 -> 效果好

保存检查点：
Layer 2 标记为 "best-model-v1"

继续改进：
基于 Layer 2 创建 Layer 3 -> 进一步优化
```

### 场景 2：数据处理管道

```
Layer 0: 原始数据

Layer 1: 数据清洗 -> checkpoint "cleaned"

Layer 2: 特征工程 -> checkpoint "features"

Layer 3: 模型训练 -> checkpoint "trained"

需要重新清洗时：
切换到 Layer 0 -> 创建新分支 Layer 1'
不影响现有的训练结果（Layer 3）
```

### 场景 3：版本回滚

```
生产环境：
Layer 10 (production): 当前运行版本

部署新版本：
Layer 11 (staging): 新代码

发现问题：
切换回 Layer 10（秒级回滚）

修复后：
基于 Layer 10 创建 Layer 12（修复版本）
```

## API 接口

### 层管理 API

```rust
// 创建检查点
POST /api/v1/layers/checkpoint
{
    "name": "checkpoint-001",
    "description": "After data preprocessing"
}

// 列出所有层
GET /api/v1/layers
Response: [
    {
        "layer_id": "uuid",
        "name": "checkpoint-001",
        "parent_layer_id": "parent-uuid",
        "is_readonly": true,
        "file_count": 1234,
        "total_size": 1048576,
        "created_at": "2026-01-14T..."
    },
    ...
]

// 查看层详情
GET /api/v1/layers/{layer_id}

// 切换层
POST /api/v1/layers/switch
{
    "layer_id": "target-uuid",
    "create_branch_if_readonly": true
}

// 查看当前层
GET /api/v1/layers/current

// 查看层的变化
GET /api/v1/layers/{layer_id}/changes
Response: {
    "added": ["/path/file1.txt", ...],
    "modified": ["/path/file2.txt", ...],
    "deleted": ["/path/file3.txt", ...]
}

// 合并层
POST /api/v1/layers/squash
{
    "layer_ids": ["uuid1", "uuid2", "uuid3"],
    "new_name": "squashed-layer"
}

// 创建分支
POST /api/v1/layers/{layer_id}/branch
{
    "branch_name": "experiment-2"
}

// 删除层（如果没有子层）
DELETE /api/v1/layers/{layer_id}
```

### CLI 命令

```bash
# 创建检查点
tarbox layer checkpoint "after-preprocessing" \
  --description "Data preprocessing completed"

# 列出层
tarbox layer list

# 查看当前层
tarbox layer current

# 切换层
tarbox layer switch checkpoint-001

# 查看层的变化
tarbox layer diff layer-2

# 查看层历史（类似 git log）
tarbox layer history

# 创建分支
tarbox layer branch experiment-2 --from checkpoint-001

# 合并层
tarbox layer squash layer-1 layer-2 layer-3 \
  --output final-layer

# 可视化层关系
tarbox layer tree
```

## 与其他组件集成

### 与审计系统集成

```
审计记录包含层信息：
- 每个文件操作记录当前 layer_id
- 可以追溯到具体的层
- 支持按层查询审计日志

查询示例：
SELECT * FROM audit_logs 
WHERE layer_id = :target_layer_id;
```

### 与 Kubernetes 集成

```yaml
# PVC 支持指定初始层
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: agent-workspace
  annotations:
    tarbox.io/base-layer: "checkpoint-prod-v1"
    tarbox.io/auto-checkpoint: "true"
    tarbox.io/checkpoint-interval: "1h"
spec:
  storageClassName: tarbox
  accessModes:
    - ReadWriteMany
  resources:
    requests:
      storage: 10Gi
```

## 性能考虑

### 层深度限制

```
问题：层数太多影响查询性能

建议：
- 定期合并层（squash）
- 限制最大层深度（如 100 层）
- 超过深度时自动触发合并

性能影响：
- 每增加 10 层，查询延迟 +5%
- 通过缓存可以减轻影响
```

### 写入性能

```
COW 开销：
- 首次修改文件需要复制数据块
- 小文件影响较小
- 大文件建议分块复制（只复制修改的块）

优化：
- 延迟复制：标记为 COW，实际读取时才复制
- 智能复制：只复制修改的部分
```

## 未来增强

### 层压缩

```
- 压缩历史只读层
- 减少存储空间
- 自动压缩策略
```

### 远程层

```
- 主存储：PostgreSQL 数据库
- 归档存储：对象存储（可选）
- 按需拉取历史层
```

### 层共享

```
- 跨 PVC 共享只读层
- 类似 Docker 镜像共享
- 节省空间和加速创建
```

## 文本文件的写时复制

### 设计原理

```
文本文件采用行级 COW，与二进制文件的块级 COW 不同：

二进制文件 COW：
- 以固定大小块（如 4KB）为单位
- 修改任何字节需要复制整个块
- 适合二进制数据（无语义结构）

文本文件 COW：
- 以行为单位进行差异存储
- 只复制修改的行
- 类似 Git 的 diff 机制
- 适合有行结构的文本数据
```

### 层间文本文件处理

```
场景：在 Layer 2 修改 Layer 1 的文本文件

Layer 1: config.yaml (100 行)
├── line 1-50  -> TextBlock A (50 行)
├── line 51-100 -> TextBlock B (50 行)

用户在 Layer 2 修改第 50 行：

Layer 2: config.yaml
├── line 1-49  -> 继承 TextBlock A (通过 line_map 引用)
├── line 50    -> TextBlock C (新建，1 行) 
├── line 51-100 -> 继承 TextBlock B (通过 line_map 引用)

存储效率：
- Layer 1: 存储 100 行
- Layer 2: 只存储 1 行
- 总存储: 101 行（而非 200 行）
```

### 文本文件切换层

```
切换到历史层时的文本文件处理：

当前层 (Layer 3): /app/config.yaml
├── 使用 text_line_map (tenant_id, inode_id, layer_3)
└── 重组出 Layer 3 的文件内容

切换到 Layer 2:
1. 清除缓存
2. 查询 text_line_map (tenant_id, inode_id, layer_2)
3. 根据 Layer 2 的 line_map 重组文件
4. 展示 Layer 2 时的文件内容

优势：
- 即时切换（无需物理复制文件）
- 可以对比任意两个层的文本差异
- 每层的文本文件都是完整的逻辑视图
```

### 文本文件的联合视图

```
联合视图构建（类似 Git）：

base -> layer1 -> layer2 -> layer3 [current]

读取文件 /data/notes.md：

1. 从当前层 (layer3) 开始查询
2. 查找 text_file_metadata(tenant_id, inode_id, layer3)
3. 如果存在：
   - 读取 text_line_map(tenant_id, inode_id, layer3)
   - 根据 line_map 获取所有 TextBlock
   - 重组文件内容
4. 如果不存在：
   - 向上查找 layer2、layer1、base
   - 找到最近的祖先层中的版本

注意：
- 每层的文本文件元数据独立存储
- 不需要逐层合并（与二进制文件不同）
- 每层的 line_map 已经是完整的逻辑视图
```

### 大文本文件优化

```
对于大型文本文件（如大型日志、代码文件）：

1. 分块策略：
   - 每 100 行一个 TextBlock（可配置）
   - 修改某行只影响该行所在的 block
   - 其他 block 可以继续共享

2. 示例：
   Layer 1: large.log (10000 行)
   └── 分为 100 个 TextBlock，每个 100 行
   
   Layer 2: 修改第 5000 行
   └── 只需要创建 1 个新 TextBlock（第 50 个 block）
       其他 99 个 block 继承自 Layer 1

3. 空间效率：
   - 10000 行的文件
   - 修改 1 行
   - Layer 2 只增加约 100 行的存储（1%）
```

### 文本去重示例

```
跨文件、跨层的内容去重：

Layer 1:
├── /app/config.yaml (前 50 行包含标准头部)
└── /app/config-dev.yaml (前 50 行相同头部)

Layer 2:
└── /app/config-prod.yaml (前 50 行也是相同头部)

存储优化：
- 相同的 50 行头部只存储一个 TextBlock
- 三个文件的 line_map 都引用同一个 TextBlock
- ref_count = 3

实际场景：
- 多个配置文件的公共部分
- 代码文件的相同导入语句
- 文档的标准模板部分
```

### 与二进制文件 COW 的对比

```
特性对比：

| 特性 | 二进制文件 COW | 文本文件 COW |
|------|---------------|-------------|
| 粒度 | 固定块（4KB） | 行 |
| 修改开销 | 复制整个块 | 只复制变化的行 |
| 差异对比 | 字节级 diff | 行级 diff（类似 git） |
| 存储效率 | 中等 | 高（增量存储） |
| 查询性能 | 快（连续读取） | 稍慢（需要重组）|
| 适用场景 | 二进制文件、图片 | 代码、配置、日志、文档 |
| 去重效果 | 相同块去重 | 行级去重 + 跨文件 |

选择策略：
- 自动检测文件类型
- 文本文件使用行级 COW
- 二进制文件使用块级 COW
- 对用户完全透明
```

### 实现要点

```
1. 文件类型检测：
   - 检查文件扩展名
   - 检测 UTF-8 编码
   - 检测是否包含大量二进制字符
   - 限制最大行数（超大文本降级为二进制）

2. 写入时处理：
   - 文本文件：解析行，创建/更新 text_line_map
   - 二进制文件：分块，创建/更新 data_blocks
   - 混合型大文件：根据阈值降级

3. 读取时处理：
   - 检查 text_file_metadata 是否存在
   - 如果存在：从 text_line_map 重组
   - 如果不存在：从 data_blocks 读取
   - 缓存重组结果

4. 层切换时：
   - 清除文本文件缓存
   - 重新查询对应层的 line_map
   - 重建文件内容

5. 层删除时：
   - 减少 TextBlock 的 ref_count
   - ref_count = 0 时可以清理
   - 考虑延迟删除（保留一段时间）
```

### 性能优化

```
1. 缓存策略：
   - 缓存热门文件的 line_map
   - 缓存常用的 TextBlock
   - LRU 淘汰策略

2. 批量操作：
   - 批量读取 line_map
   - 批量查询 TextBlock
   - 减少数据库往返

3. 预读优化：
   - 读取文件时预读相邻行的 block
   - 假设顺序读取模式
   - 适合日志和代码阅读场景

4. 写入优化：
   - 批量创建 TextBlock
   - 批量更新 line_map
   - 使用事务保证一致性
```


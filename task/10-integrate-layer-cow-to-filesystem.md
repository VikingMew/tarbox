# Task 10: 将 Layer 和 COW 集成到文件系统操作

## 问题描述

当前 `FileSystem::write_file()` 直接写入 `data_blocks` 表，没有使用 `CowHandler` 和 `FileTypeDetector`。这导致：

1. **文本文件被当作二进制存储**
   - `touch` 创建文件后，`echo` 写入文本内容
   - 数据存储在 `data_blocks` 而不是 `text_blocks`
   - 没有进行文本/二进制检测

2. **没有 COW 语义**
   - 文件修改直接覆盖数据块
   - 没有记录到 `layer_entries`
   - 无法创建层快照和历史

3. **Layer 功能未生效**
   - `LayerManager` 已实现但未被调用
   - 每个租户的文件系统**必须**有至少一个 base layer
   - 这是默认行为，无需配置开关

## 设计原则

### Layer 是默认必须的

- **每个租户在首次写入时自动创建 base layer**
- **所有文件操作都通过 layer 进行**
- **没有"非 layer 模式"**，这是架构的核心设计
- 类似 Docker：你不能在没有 layer 的情况下运行容器

### 自动文件类型识别是默认必须的

文件类型（文本/二进制）在**每次写入时自动检测**，无需用户指定：

**检测规则**（按优先级顺序）：

| 条件 | 结果 |
|-----|------|
| 文件为空 (0 字节) | Text (UTF-8) |
| 文件大小 > 10 MB | Binary |
| 包含 null 字节 (0x00) | Binary |
| 非 UTF-8 且非 Latin-1 | Binary |
| 非打印字符 > 5% | Binary |
| 单行长度 > 10 KB | Binary |
| 以上都不满足 | Text |

**文本文件额外信息**：
- 编码检测：UTF-8 / ASCII / Latin-1
- 行结束符检测：LF / CRLF / CR / Mixed / None
- 行数统计

**存储策略**：
- **Text** → `text_blocks` + `text_line_map`（行级 COW，支持 diff）
- **Binary** → `data_blocks`（4KB 块级存储）

**重要**：
- 没有"强制文本模式"或"强制二进制模式"开关
- 检测完全自动，基于文件内容
- 同一文件多次写入可能改变类型（如先写文本后写二进制）

### 当前代码路径（错误）

```
FUSE write() 
  → TarboxBackend::write_file()
    → FileSystem::write_file()
      → BlockOperations::delete() + create()
        → 直接写入 data_blocks ❌ (绕过了 layer)
```

### 期望代码路径（正确）

```
FUSE write()
  → TarboxBackend::write_file()
    → LayeredFileSystem::write_file()
      → 确保 base layer 存在
      → CowHandler::write_file()
        → FileTypeDetector::detect()
          ├─ Text → write_text_file() → text_blocks + text_line_map
          └─ Binary → write_binary_file() → data_blocks
      → LayerManager::record_change() → layer_entries
```

## 实现方案

### 修改 FileSystem 结构

**文件**: `src/fs/operations.rs`

```rust
pub struct FileSystem<'a> {
    pool: &'a PgPool,
    tenant_id: TenantId,
    layer_manager: LayerManager<'a>,
    current_layer_id: LayerId,  // 缓存当前层 ID
}

impl<'a> FileSystem<'a> {
    /// 创建文件系统实例，自动确保 base layer 存在
    pub async fn new(pool: &'a PgPool, tenant_id: TenantId) -> Result<Self> {
        let layer_manager = LayerManager::new(pool, tenant_id);
        
        // 确保 base layer 存在（幂等操作）
        let base_layer = layer_manager.initialize_base_layer().await?;
        let current_layer = layer_manager.get_current_layer().await
            .unwrap_or(base_layer);
        
        info!(
            tenant_id = %tenant_id,
            layer_id = %current_layer.layer_id,
            layer_name = %current_layer.layer_name,
            "FileSystem initialized with layer"
        );
        
        Ok(Self { 
            pool, 
            tenant_id, 
            layer_manager,
            current_layer_id: current_layer.layer_id,
        })
    }

    pub async fn write_file(&self, path: &str, data: &[u8]) -> FsResult<()> {
        let inode = self.resolve_path(path).await?;
        
        debug!(
            path = %path,
            size = data.len(),
            inode_id = inode.inode_id,
            layer_id = %self.current_layer_id,
            "Writing file"
        );
        
        // 读取旧数据用于 diff 计算
        let old_data = self.read_file_internal(inode.inode_id).await.ok();
        
        // 使用 CowHandler 处理写入
        let cow = CowHandler::new(self.pool, self.tenant_id, self.current_layer_id);
        let result = cow.write_file(
            inode.inode_id, 
            data, 
            old_data.as_deref()
        ).await.map_err(|e| FsError::Other(e.to_string()))?;
        
        info!(
            path = %path,
            is_text = result.is_text,
            change_type = ?result.change_type,
            size_delta = result.size_delta,
            "File written via COW"
        );
        
        // 记录变更到当前层
        self.layer_manager.record_change(
            inode.inode_id,
            path,
            result.change_type,
            Some(result.size_delta),
            result.text_changes,
        ).await.map_err(|e| FsError::Other(e.to_string()))?;
        
        // 更新 inode 元数据
        // ...
        
        Ok(())
    }
}
```

### 添加 Debug 日志

**文件**: `src/layer/detection.rs`

```rust
use tracing::{debug, trace};

impl FileTypeDetector {
    pub fn detect(&self, data: &[u8]) -> FileTypeInfo {
        trace!(size = data.len(), "Detecting file type");
        
        if data.is_empty() {
            debug!("Empty file -> Text (UTF-8)");
            return FileTypeInfo::Text { ... };
        }
        
        if data.len() > self.config.max_text_file_size {
            debug!(size = data.len(), max = self.config.max_text_file_size, "File too large -> Binary");
            return FileTypeInfo::Binary;
        }
        
        if data.contains(&0) {
            debug!("Contains null byte -> Binary");
            return FileTypeInfo::Binary;
        }
        
        // ... 其他检测逻辑，每个分支都加日志
        
        debug!(
            encoding = %encoding,
            line_ending = %line_ending,
            line_count = line_count,
            "Detected as Text"
        );
        FileTypeInfo::Text { encoding, line_ending, line_count }
    }
}
```

**文件**: `src/layer/cow.rs`

```rust
use tracing::{debug, info, warn};

impl CowHandler {
    pub async fn write_file(&self, inode_id: InodeId, data: &[u8], old_data: Option<&[u8]>) -> Result<CowResult> {
        let file_type = self.detector.detect(data);
        
        info!(
            inode_id = inode_id,
            new_size = data.len(),
            old_size = old_data.map(|d| d.len()).unwrap_or(0),
            file_type = ?file_type,
            "COW write_file"
        );
        
        match file_type {
            FileTypeInfo::Text { encoding, line_ending, line_count } => {
                debug!(
                    encoding = %encoding,
                    line_ending = %line_ending,
                    line_count = line_count,
                    "Writing as text file"
                );
                self.write_text_file(inode_id, data, old_data, encoding, line_ending, line_count).await
            }
            FileTypeInfo::Binary => {
                debug!("Writing as binary file");
                self.write_binary_file(inode_id, data, old_data.is_none(), old_data.map(|d| d.len()).unwrap_or(0)).await
            }
        }
    }
}
```

**文件**: `src/layer/manager.rs`

```rust
use tracing::{debug, info, warn};

impl LayerManager {
    pub async fn initialize_base_layer(&self) -> Result<Layer> {
        info!(tenant_id = %self.tenant_id, "Initializing base layer");
        // ...
    }
    
    pub async fn record_change(&self, inode_id: InodeId, path: &str, change_type: ChangeType, ...) -> Result<()> {
        debug!(
            inode_id = inode_id,
            path = %path,
            change_type = ?change_type,
            layer_id = %self.current_layer_id,
            "Recording change to layer"
        );
        // ...
    }
}
```

### 日志使用方式

```bash
# 查看所有 tarbox 模块的 debug 日志
RUST_LOG=tarbox=debug tarbox --tenant myagent mount /mnt/tarbox

# 查看文件类型检测详情
RUST_LOG=tarbox::layer::detection=trace tarbox --tenant myagent mount /mnt/tarbox

# 查看 COW 处理详情
RUST_LOG=tarbox::layer::cow=debug tarbox --tenant myagent mount /mnt/tarbox

# 查看完整写入路径（推荐调试用）
RUST_LOG=tarbox::fs=debug,tarbox::layer=debug,tarbox::fuse::backend=debug tarbox --tenant myagent mount /mnt/tarbox

# 生产环境（只看 info 和警告）
RUST_LOG=tarbox=info tarbox --tenant myagent mount /mnt/tarbox
```

## 测试规范

### 1. 单元测试

#### 1.1 FileSystem 初始化测试 (`src/fs/operations.rs`)

| 测试 | 内容 | 预期 |
|-----|------|-----|
| FileSystem 创建时自动创建 base layer | 新建 FileSystem 实例 | layers 表有一条 name="base" 的记录 |
| FileSystem 创建幂等性 | 同一租户多次创建 FileSystem | 只有一个 base layer |
| 当前层初始化 | 新建 FileSystem | current_layer_id 指向 base layer |

#### 1.2 文件类型检测测试 (`src/layer/detection.rs`)

| 测试 | 输入内容 | 预期结果 |
|-----|---------|---------|
| 空文件 | 0 字节 | Text (UTF-8, None, 0 lines) |
| ASCII 文本 | `hello\nworld\n` | Text (ASCII, LF, 2 lines) |
| UTF-8 文本 | 包含中文的代码 | Text (UTF-8, LF) |
| Shell 脚本 | `#!/bin/bash\necho hello` | Text (ASCII, LF) |
| JSON | `{"key": "value"}` | Text (ASCII, None) |
| CRLF 文本 | `line1\r\nline2\r\n` | Text (ASCII, CRLF) |
| 混合行结束符 | `line1\nline2\r\n` | Text (ASCII, Mixed) |
| 超大文件 | > 10 MB | Binary |
| 含 null 字节 | `hello\x00world` | Binary |
| 超长单行 | 单行 > 10 KB | Binary |
| 高非打印字符 | > 5% 非打印 | Binary |
| PNG 文件头 | `\x89PNG\r\n\x1a\n` | Binary |
| PDF 文件头 | `%PDF-1.4` + 二进制内容 | Binary |

#### 1.3 COW 处理测试 (`src/layer/cow.rs`)

| 测试 | 内容 | 预期 |
|-----|------|-----|
| CowHandler 创建 | 传入 pool, tenant_id, layer_id | 正常创建 |
| 行级 diff 计算 - 新增行 | 原文 2 行，新文 4 行 | lines_added = 2 |
| 行级 diff 计算 - 删除行 | 原文 4 行，新文 2 行 | lines_deleted = 2 |
| 行级 diff 计算 - 修改行 | 修改中间一行 | lines_modified = 1 |
| 行级 diff 计算 - 混合变更 | 增删改都有 | 各字段正确统计 |
| 文本哈希计算 | 相同内容 | 相同哈希 |
| 文本哈希计算 | 不同内容 | 不同哈希 |

#### 1.4 Layer Manager 测试 (`src/layer/manager.rs`)

| 测试 | 内容 | 预期 |
|-----|------|-----|
| 初始化 base layer | 首次调用 | 创建 name="base" 的 layer |
| 初始化 base layer 幂等 | 多次调用 | 返回同一个 layer |
| 记录变更 | 调用 record_change | layer_entries 有记录 |
| 记录变更类型 | Add/Modify/Delete | change_type 正确 |

### 2. 集成测试

#### 2.1 FileSystem + Layer 集成 (`tests/filesystem_layer_integration_test.rs`)

| 测试 | 操作 | 验证 |
|-----|------|-----|
| FileSystem 自动创建 base layer | 创建 FileSystem | layers 表有 base layer |
| 文本文件存储位置 | 写入 `hello\nworld\n` | text_blocks 有数据，data_blocks 为空 |
| 二进制文件存储位置 | 写入含 null 字节数据 | data_blocks 有数据，text_blocks 为空 |
| 新建文件记录 layer_entry | create + write | layer_entries 有 change_type=Add |
| 修改文件记录 layer_entry | write 已存在文件 | layer_entries 有 change_type=Modify |
| 文本变更统计 | 修改文本文件 | text_changes JSON 有 lines_added/deleted/modified |

#### 2.2 COW + 存储集成 (`tests/cow_storage_integration_test.rs`)

| 测试 | 操作 | 验证 |
|-----|------|-----|
| 文本文件行级存储 | 写入 3 行文本 | text_line_map 有 3 条记录 |
| 文本文件去重 | 两个文件有相同行 | 相同行共享 text_block |
| 二进制文件块存储 | 写入 5KB 数据 | data_blocks 有 2 条记录 (4KB + 1KB) |
| 二进制文件去重 | 两个文件内容相同 | 共享相同 content_hash 的 block |

### 3. 跨 Layer 文件类型变化测试 (`tests/layer_file_type_transition_test.rs`)

这是**关键边界情况**：同一文件在不同 layer 可能有不同的存储类型。

#### 3.1 类型转换测试

| 测试 | 操作流程 | 验证 |
|-----|---------|-----|
| Text → Binary | Layer1 写文本 → checkpoint → Layer2 写二进制 | Layer1 有 text_metadata，Layer2 有 data_blocks |
| Binary → Text | Layer1 写二进制 → checkpoint → Layer2 写文本 | Layer1 有 data_blocks，Layer2 有 text_metadata |
| 多次类型切换 | Text → Binary → Text → Binary (4 layers) | 每个 layer 存储类型独立正确 |

#### 3.2 Layer 切换后读取

| 测试 | 操作流程 | 验证 |
|-----|---------|-----|
| 切换后读取正确类型 | Layer1(text) → Layer2(binary) → 切换回 Layer1 | 读取得到文本内容 |
| 多次切换读取 | 在 4 个 layer 间随机切换 | 每次读取内容与该 layer 写入内容一致 |

#### 3.3 Layer Entry 类型记录

| 测试 | 操作流程 | 验证 |
|-----|---------|-----|
| 新建文本文件 | Layer1 创建并写入文本 | change_type=Add, text_changes 有值 |
| 文本改为二进制 | Layer2 用二进制覆盖 | change_type=Modify, text_changes=None |
| 二进制改为文本 | Layer3 用文本覆盖 | change_type=Modify, text_changes 有值 |

### 4. E2E 测试 (FUSE)

#### 4.1 FUSE 基础测试 (`tests/fuse_layer_e2e_test.rs`)

| 测试 | 操作 | 验证 |
|-----|------|-----|
| 挂载自动有 base layer | mount 后读取 `/.tarbox/layers/current` | 返回 name="base" |
| FUSE 写入文本文件 | `echo "hello" > test.txt` | DB 中 text_blocks 有数据 |
| FUSE 写入二进制文件 | 写入含 null 字节 | DB 中 data_blocks 有数据 |
| touch + echo 场景 | `touch test.txt && echo "hello" > test.txt` | 最终在 text_blocks (当前失败场景) |

#### 4.2 FUSE Layer 操作测试

| 测试 | 操作 | 验证 |
|-----|------|-----|
| 通过 hook 创建 checkpoint | 写入 `/.tarbox/layers/new` | 新 layer 创建 |
| 通过 hook 切换 layer | 写入 `/.tarbox/layers/switch` | current_layer 变更 |
| 切换后文件内容正确 | 切换到历史 layer 后 cat 文件 | 内容为该 layer 版本 |

#### 4.3 FUSE 跨 Layer 类型变化

| 测试 | 操作 | 验证 |
|-----|------|-----|
| FUSE 下 Text → Binary | mount → 写文本 → checkpoint → 写二进制 | 两个 layer 存储类型不同 |
| FUSE 切换后读取正确 | 切换到文本 layer 后 cat | 得到文本内容而非二进制 |

### 5. 测试文件清单

| 文件 | 类型 | 测试数量 |
|-----|------|---------|
| `src/fs/operations.rs` (mod tests) | 单元测试 | ~7 |
| `src/layer/detection.rs` (mod tests) | 单元测试 | ~15 |
| `src/layer/cow.rs` (mod tests) | 单元测试 | ~8 |
| `src/layer/manager.rs` (mod tests) | 单元测试 | ~5 |
| `tests/filesystem_layer_integration_test.rs` | 集成测试 | ~10 |
| `tests/cow_storage_integration_test.rs` | 集成测试 | ~6 |
| `tests/layer_file_type_transition_test.rs` | 集成测试 | ~8 |
| `tests/fuse_layer_e2e_test.rs` | E2E 测试 | ~10 |

**总计: ~70 个测试**

## 子任务清单

### 10.1 添加 Debug 日志

- [ ] `src/layer/detection.rs` - 文件类型检测日志
- [ ] `src/layer/cow.rs` - COW 处理日志
- [ ] `src/layer/manager.rs` - Layer 管理日志
- [ ] `src/fs/operations.rs` - 文件系统操作日志
- [ ] `src/fuse/backend.rs` - FUSE 回调日志

### 10.2 修改 FileSystem

- [ ] 添加 `LayerManager` 字段
- [ ] 修改 `new()` 自动初始化 base layer
- [ ] 修改 `write_file()` 使用 `CowHandler`
- [ ] 修改 `write_file()` 记录 `layer_entries`
- [ ] 处理生命周期问题

### 10.3 修改读取路径

- [ ] `read_file()` 支持从 `text_blocks` 读取
- [ ] 根据文件类型选择读取来源
- [ ] 考虑 Union View（未来）

### 10.4 单元测试

- [ ] FileSystem 初始化测试
- [ ] COW 路由测试
- [ ] 文件类型检测补充测试

### 10.5 集成测试

- [ ] `tests/filesystem_layer_integration_test.rs`
- [ ] 文本/二进制存储验证
- [ ] Layer entry 记录验证
- [ ] 变更统计验证

### 10.6 E2E 测试

- [ ] `tests/fuse_layer_e2e_test.rs`
- [ ] FUSE 挂载 base layer 验证
- [ ] touch + echo 场景测试
- [ ] 二进制文件测试

## 依赖

- Task 06: 数据库层高级功能 ✅
- Task 08: 分层文件系统实现 ✅

## 验收标准

- [ ] `touch` + `echo` 写入的文本内容存储在 `text_blocks`
- [ ] 二进制文件存储在 `data_blocks`
- [ ] 每个租户首次操作自动创建 base layer
- [ ] 文件变更记录到 `layer_entries`
- [ ] `RUST_LOG=tarbox=debug` 显示文件类型检测结果
- [ ] 所有单元测试通过
- [ ] 所有集成测试通过
- [ ] 所有 E2E 测试通过
- [ ] 代码覆盖率 > 80%

## 风险和注意事项

1. **生命周期问题**: `FileSystem` 持有 `LayerManager` 需要处理好生命周期
2. **性能**: 读取旧数据用于 diff 会增加开销，考虑缓存
3. **向后兼容**: CLI 命令需要继续工作
4. **并发**: 多个写入操作的层一致性
5. **错误恢复**: Layer 操作失败时的回滚策略

## 预估时间

- Debug 日志: 1 小时
- FileSystem 修改: 3-4 小时
- 单元测试: 2 小时
- 集成测试: 2 小时
- E2E 测试: 2 小时
- 调试和修复: 2-3 小时

**总计: 12-14 小时 (2 天)**

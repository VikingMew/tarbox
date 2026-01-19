# 文本文件优化

## 实现状态

**✅ 已实现** (Task 06, 2026-01-19)

核心数据库表和操作已完成：
- ✅ text_blocks表（内容寻址存储，blake3哈希，ref_count管理）
- ✅ text_file_metadata表（文件级元数据）
- ✅ text_line_map表（行号到块的映射）
- ✅ find_or_create_text_block函数（自动去重）
- ✅ cleanup_unused_text_blocks函数（清理未使用块）
- ✅ TextBlockRepository trait（9个方法）
- ✅ TextBlockOperations实现（blake3哈希、去重、引用计数、行映射）
- ✅ 14个集成测试
- ⚠️ 文件系统层集成待实现（将在Task 08实现diff功能）

**代码位置**:
- 迁移: `migrations/20260118000004_create_text_storage.sql`
- 模型: `src/storage/models.rs` (TextBlock, TextFileMetadata, TextLineMap)
- Trait: `src/storage/traits.rs` (TextBlockRepository)
- 实现: `src/storage/text.rs` (TextBlockOperations, compute_content_hash)
- 测试: `tests/text_storage_integration_test.rs`

## 概述

针对 Agent 常用的文本文件（CSV、Markdown、YAML、HTML、JSON、代码等），提供类似 Git 的行级差异存储和管理，优化存储空间和版本对比体验。

## 设计目标

### 核心目标

1. **行级差异存储**：层间只存储变化的行，而非整个文件
2. **高效版本对比**：快速展示文件在不同层之间的变化
3. **透明操作**：对应用层完全透明，仍然是标准的文件读写
4. **存储优化**：减少文本文件在多层间的存储冗余

### 非目标

- 不进行结构化解析（不解析 CSV 列、YAML 字段等）
- 不提供文件内部内容查询
- 不改变 POSIX 文件系统语义

## 文件类型识别

### 自动检测

```
基于以下特征识别文本文件：

1. 文件扩展名：
   - 代码：.rs, .py, .js, .go, .java, .c, .cpp, .h, etc.
   - 配置：.yaml, .yml, .toml, .json, .xml, .ini, .conf
   - 文档：.md, .txt, .rst, .adoc
   - 数据：.csv, .tsv, .log
   - Web：.html, .htm, .css, .svg

2. 文件内容：
   - UTF-8 编码检测
   - 无二进制字符（或比例很低）
   - 包含换行符

3. MIME 类型：
   - text/*
   - application/json
   - application/xml
   - application/yaml
```

### 配置选项

```toml
[text_optimization]
enabled = true

# 自动检测
auto_detect = true

# 显式文本扩展名
text_extensions = [
    "txt", "md", "csv", "json", "yaml", "yml",
    "html", "xml", "toml", "ini", "conf",
    "rs", "py", "js", "go", "java", "c", "cpp"
]

# 最大文件大小（超过则当二进制处理）
max_text_file_size = "10MB"

# 最大行数（超过则当二进制处理）
max_lines = 100000
```

## 数据模型

### 文本块存储

```
TextBlock {
    block_id: UUID,
    content_hash: String,      // 内容哈希（用于去重）
    content: String,            // 实际文本内容（单行或多行）
    line_count: i32,            // 行数
    byte_size: i32,             // 字节大小
    encoding: String,           // 编码（通常是 UTF-8）
}
```

### 文本文件元数据

```
TextFileMetadata {
    inode_id: i64,
    tenant_id: UUID,
    layer_id: UUID,
    total_lines: i32,           // 总行数
    encoding: String,           // 文件编码
    line_ending: String,        // 行结束符（LF/CRLF）
    has_trailing_newline: bool, // 是否以换行结尾
}
```

### 行映射

```
TextLineMap {
    inode_id: i64,
    tenant_id: UUID,
    layer_id: UUID,
    line_number: i32,           // 逻辑行号（从 1 开始）
    block_id: UUID,             // 指向 TextBlock
    block_line_offset: i32,     // 在 block 内的行偏移
}
```

## 存储策略

### 初始存储（基础层）

```
当文件首次创建时：

1. 检测文件是否为文本文件
2. 如果是文本文件：
   - 按行分割内容
   - 将连续的行分组为 TextBlock（例如每 100 行一个 block）
   - 计算每个 block 的内容哈希
   - 存储到 text_blocks 表
   - 创建 line_map 映射
3. 如果是二进制文件：
   - 使用传统的数据块存储方式
```

### 层间差异存储

```
当文件在新层中被修改时：

1. 读取父层的文件内容（逻辑视图）
2. 对比新旧内容的差异（行级 diff）
3. 只存储变化的部分：
   - 新增的行：创建新的 TextBlock
   - 删除的行：line_map 中标记删除
   - 修改的行：创建新的 TextBlock
   - 未变化的行：复用父层的 TextBlock

4. 创建当前层的 line_map：
   - 继承未变化的行映射
   - 添加新增/修改的行映射
```

### 块大小策略

```
TextBlock 的分块策略：

1. 默认块大小：100 行或 8KB（取较小者）
2. 好处：
   - 提高去重效率
   - 减少小修改的影响范围
   - 平衡查询性能

3. 特殊情况：
   - 单行超过 8KB：独立成块
   - 大文件：使用更大的块（1000 行）
```

## 差异算法

### 行级 Diff

```
使用经典的 diff 算法（Myers 算法或类似）：

1. 计算两个版本文件的行差异
2. 生成操作序列：
   - Add(line_num, content)
   - Delete(line_num)
   - Modify(line_num, old, new)
   - Keep(line_num, line_num_in_parent)

3. 基于操作序列生成新层的存储：
   - Keep：复用父层的 block 引用
   - Add/Modify：创建新 block
   - Delete：不在 line_map 中记录
```

### 内容哈希去重

```
跨文件和跨层的内容去重：

1. 对每个 TextBlock 计算内容哈希（SHA-256）
2. 存储时检查是否已存在相同内容的 block
3. 如果存在，直接引用已有的 block_id
4. 引用计数管理：
   - 记录每个 block 被引用的次数
   - 当引用计数为 0 时可以删除
```

## 数据库设计

### text_blocks 表

```sql
CREATE TABLE text_blocks (
    block_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    content_hash VARCHAR(64) NOT NULL UNIQUE,
    content TEXT NOT NULL,
    line_count INTEGER NOT NULL,
    byte_size INTEGER NOT NULL,
    encoding VARCHAR(20) NOT NULL DEFAULT 'UTF-8',
    ref_count INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    last_accessed_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_text_blocks_hash ON text_blocks(content_hash);
CREATE INDEX idx_text_blocks_ref_count ON text_blocks(ref_count);
```

### text_file_metadata 表

```sql
CREATE TABLE text_file_metadata (
    tenant_id UUID NOT NULL,
    inode_id BIGINT NOT NULL,
    layer_id UUID NOT NULL,
    total_lines INTEGER NOT NULL,
    encoding VARCHAR(20) NOT NULL DEFAULT 'UTF-8',
    line_ending VARCHAR(10) NOT NULL DEFAULT 'LF',
    has_trailing_newline BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    PRIMARY KEY (tenant_id, inode_id, layer_id),
    FOREIGN KEY (tenant_id, inode_id) REFERENCES inodes(tenant_id, inode_id),
    FOREIGN KEY (layer_id) REFERENCES layers(layer_id)
);
```

### text_line_map 表

```sql
CREATE TABLE text_line_map (
    tenant_id UUID NOT NULL,
    inode_id BIGINT NOT NULL,
    layer_id UUID NOT NULL,
    line_number INTEGER NOT NULL,
    block_id UUID NOT NULL REFERENCES text_blocks(block_id),
    block_line_offset INTEGER NOT NULL,
    PRIMARY KEY (tenant_id, inode_id, layer_id, line_number),
    FOREIGN KEY (tenant_id, inode_id, layer_id) 
        REFERENCES text_file_metadata(tenant_id, inode_id, layer_id)
);

CREATE INDEX idx_text_line_map_lookup 
    ON text_line_map(tenant_id, inode_id, layer_id, line_number);
```

## 读写操作

### 读取文本文件

```
1. 检查文件是否为文本文件类型
2. 如果是文本文件：
   a. 查询 text_file_metadata 获取元数据
   b. 查询 text_line_map 获取行映射（按 line_number 排序）
   c. 根据 block_id 批量读取 text_blocks
   d. 重组文件内容
   e. 返回完整文本

3. 如果是二进制文件：
   - 使用传统数据块读取方式

4. 优化：
   - 缓存热门文件的 line_map
   - 缓存常用的 text_blocks
   - 支持范围读取（只读取指定行范围）
```

### 写入文本文件

```
完整覆盖写入：

1. 检测是否为文本文件
2. 如果是文本文件：
   a. 解析新内容为行
   b. 如果存在父层版本：
      - 计算与父层的 diff
      - 只存储变化的部分
   c. 如果是新文件：
      - 分块存储所有内容
   d. 创建/更新 text_file_metadata
   e. 创建/更新 text_line_map

3. 如果是二进制文件：
   - 使用传统数据块写入方式
```

### 追加写入

```
对于日志文件等追加场景：

1. 识别追加操作（写入位置 = 文件末尾）
2. 只添加新的行到 text_blocks
3. 扩展 text_line_map（增加新行）
4. 不需要复制已有内容
5. 高效支持日志文件场景
```

## 版本对比

### Diff 接口

```
提供文件在不同层之间的差异查询：

输入：
- tenant_id
- inode_id
- layer_id_old
- layer_id_new

输出：
DiffResult {
    old_layer: LayerInfo,
    new_layer: LayerInfo,
    changes: Vec<LineChange>
}

LineChange {
    change_type: ChangeType,  // Add, Delete, Modify
    old_line_num: Option<i32>,
    new_line_num: Option<i32>,
    old_content: Option<String>,
    new_content: Option<String>,
}
```

### 实现

```
1. 获取两个层的 text_line_map
2. 逐行对比：
   - 行号和内容都相同：unchanged
   - 行号相同但内容不同：modified
   - 只在新层存在：added
   - 只在旧层存在：deleted

3. 生成 unified diff 格式输出
4. 支持多种输出格式：
   - unified diff（类似 git diff）
   - side-by-side
   - JSON 结构化输出
```

## 命令行工具

### 查看文件历史

```bash
# 查看文件在所有层的版本
tarbox history /data/config.yaml

# 输出：
# Layer: base (2026-01-10 10:00:00)
#   Size: 1.2KB, Lines: 45
# Layer: checkpoint-1 (2026-01-11 14:30:00)
#   Size: 1.3KB, Lines: 48 (+3 lines)
# Layer: checkpoint-2 (2026-01-12 09:15:00)
#   Size: 1.1KB, Lines: 42 (-6 lines)
```

### 对比两个层

```bash
# 对比两个层的文件差异
tarbox diff checkpoint-1 checkpoint-2 /data/config.yaml

# 输出类似 git diff：
# --- a/data/config.yaml (checkpoint-1)
# +++ b/data/config.yaml (checkpoint-2)
# @@ -10,7 +10,9 @@
#  database:
#    host: localhost
# -  port: 5432
# +  port: 5433
# +  pool_size: 20
#    database: tarbox
```

### 导出差异

```bash
# 导出为 patch 文件
tarbox diff checkpoint-1 checkpoint-2 /data/config.yaml --output config.patch

# 导出所有文本文件的差异
tarbox diff checkpoint-1 checkpoint-2 --all --format unified > changes.diff
```

## 性能优化

### 读性能

```
1. 缓存策略：
   - LRU 缓存热门文件的 line_map
   - 缓存常用的 text_blocks
   - 预加载：读取文件时预读相邻的 blocks

2. 批量操作：
   - 批量查询 line_map
   - 批量读取 text_blocks
   - 减少数据库往返次数

3. 索引优化：
   - 联合索引：(tenant_id, inode_id, layer_id)
   - 覆盖索引：line_map 查询包含所有需要的字段
```

### 写性能

```
1. 批量插入：
   - 批量创建 text_blocks
   - 批量插入 line_map
   - 使用事务保证一致性

2. 去重优化：
   - 先批量计算所有 block 的哈希
   - 一次查询检查哪些已存在
   - 只插入新的 blocks

3. 写缓冲：
   - 小文件直接写入
   - 大文件使用流式处理
```

### 存储优化

```
1. 内容去重：
   - 跨文件共享相同的 text_blocks
   - 跨层共享未修改的内容

2. 压缩：
   - text_blocks 的 content 字段启用压缩
   - PostgreSQL 支持 TOAST 自动压缩

3. 清理策略：
   - 定期清理 ref_count = 0 的 blocks
   - 归档旧层的 text_blocks
```

## 与现有系统集成

### 与分层文件系统集成

```
无缝集成到现有的分层架构：

1. 文件类型透明：
   - 文本文件：使用 text_blocks + line_map
   - 二进制文件：使用传统 data_blocks
   - 对上层（FUSE）完全透明

2. 层操作兼容：
   - 创建层：支持文本和二进制文件
   - 切换层：自动处理两种存储方式
   - 删除层：清理 text_blocks 引用计数

3. COW 语义保持：
   - 文本文件的 COW：行级共享
   - 二进制文件的 COW：块级共享
```

### 与审计系统集成

```
增强审计能力：

1. 记录文本文件修改细节：
   - 修改了哪些行
   - 增加/删除了多少行
   - 修改摘要（可选）

2. 审计日志扩展：
   audit_logs {
       ...
       text_changes: JSONB,  // 文本文件的变化详情
   }
   
   text_changes 示例：
   {
       "lines_added": 10,
       "lines_deleted": 5,
       "lines_modified": 3,
       "old_line_count": 100,
       "new_line_count": 105
   }
```

### 与 FUSE 接口集成

```
透明的文件系统操作：

1. read()：自动从 text_blocks 重组
2. write()：自动触发 diff 和存储
3. 支持 mmap（如果需要）
4. 支持部分读写（seek + read/write）
```

## 文本文件特性支持

### 大文件支持

```
对于超大文本文件（如大型日志）：

1. 流式处理：
   - 不一次性加载全文
   - 按 block 范围读取

2. 分段索引：
   - 每 N 行创建一个索引点
   - 支持快速定位到任意行

3. 限制：
   - 超过阈值的文件降级为二进制处理
   - 可配置阈值（行数、大小）
```

### 不同编码支持

```
支持多种文本编码：

1. 自动检测：
   - UTF-8（默认）
   - UTF-16
   - GB2312/GBK
   - ISO-8859-1

2. 统一处理：
   - 内部统一使用 UTF-8 存储
   - 读取时转换为原始编码（可选）
```

### 行尾符处理

```
正确处理不同平台的行尾符：

1. 检测：
   - LF (Unix/Linux)
   - CRLF (Windows)
   - CR (旧 Mac)

2. 保持原样：
   - 记录原始行尾符类型
   - 读取时恢复原始格式
```

## 监控指标

### 存储指标

```
tarbox_text_blocks_total
tarbox_text_blocks_bytes
tarbox_text_dedup_ratio          # 去重率
tarbox_text_vs_binary_ratio      # 文本文件占比
```

### 性能指标

```
tarbox_text_read_duration_seconds
tarbox_text_write_duration_seconds
tarbox_text_diff_duration_seconds
tarbox_text_cache_hit_ratio
```

### 使用统计

```
tarbox_text_files_by_type{ext="md"}
tarbox_text_lines_total
tarbox_text_avg_lines_per_file
```

## 未来增强

### 智能差异

```
- 基于语义的 diff（例如 JSON 的键值对比）
- 忽略空白符的对比选项
- 更智能的 diff 算法（考虑移动/重命名）
```

### 搜索支持

```
- 跨层的全文搜索
- 正则表达式搜索
- 基于行号的快速定位
```

### 协作功能

```
- 多层并行编辑的冲突检测
- 合并不同分支的文本文件（如果支持分支）
```

# 文件系统 Hook 设计

## 概述

Tarbox 提供特殊的虚拟文件系统路径，允许用户通过标准文件操作来控制层管理，无需使用专门的 CLI 工具或 API。

## 设计理念

### Unix 哲学

```
"一切皆文件"

通过文件系统接口控制层操作：
- 创建层：写入特殊文件
- 切换层：写入层 ID
- 查询状态：读取虚拟文件
- 简单直观：使用 cat, echo 等标准工具
```

### 设计目标

- **零学习成本**：使用标准 Unix 工具，无需学习新命令
- **脚本友好**：易于集成到 Shell 脚本和自动化流程
- **Agent 优化**：AI Agent 可以用最基础的文件 I/O 操作
- **一致性**：文件操作语义清晰，行为可预测

## 虚拟文件系统结构

### 顶层布局

```
/.tarbox/                      # 控制目录（隐藏）
├── layers/                    # 层管理
│   ├── current                # 当前层信息（只读）
│   ├── list                   # 所有层列表（只读）
│   ├── new                    # 创建新层（只写）
│   ├── switch                 # 切换层（只写）
│   ├── drop                   # 丢弃层（只写）
│   ├── tree                   # 层树结构（只读）
│   └── tags/                  # 标签目录
│       └── <tag-name>         # 符号链接到层
├── snapshots/                 # 快照视图（只读）
│   ├── <layer-name>/          # 历史层的只读视图
│   └── ...
├── audit/                     # 审计查询
│   ├── recent                 # 最近操作（只读）
│   └── query                  # 查询接口（读写）
├── stats/                     # 统计信息
│   ├── usage                  # 空间使用（只读）
│   ├── performance            # 性能指标（只读）
│   └── cache                  # 缓存状态（只读）
└── control/                   # 系统控制
    ├── flush                  # 刷新缓存（只写）
    └── gc                     # 垃圾回收（只写）
```

### 文件属性设计

每个虚拟文件有明确的权限和用途：

```
只读文件（0444）：
- 查询信息，返回 JSON 或文本
- 示例：current, list, tree

只写文件（0200）：
- 执行操作，写入参数
- 示例：new, switch, drop

读写文件（0644）：
- 交互式查询
- 示例：query（写入查询条件，读取结果）
```

## 层管理操作

### 查看当前层

**操作**：读取 `/.tarbox/layers/current`

**输入**：无

**输出**：JSON 格式的当前层信息
- layer_id：层的唯一标识
- name：层名称
- parent_id：父层 ID
- created_at：创建时间
- file_count：文件数量
- total_size：总大小
- is_readonly：是否只读

### 创建新层（Checkpoint）

**操作**：写入 `/.tarbox/layers/new`

**输入格式**：
- 简单模式：纯文本层名称
- 完整模式：JSON 对象包含 name 和 description

**行为**：
1. 当前层被标记为只读
2. 创建新的可写层，父层为当前层
3. 自动切换到新层
4. 返回确认消息

**输出**：写入成功后返回的字节数

### 列出所有层

**操作**：读取 `/.tarbox/layers/list`

**输入**：无

**输出**：JSON 数组，包含所有层的基本信息
- 按创建时间排序
- 包含每层的 ID、名称、父层、创建时间、只读状态

### 切换层

**操作**：写入 `/.tarbox/layers/switch`

**输入格式**：
- 简单模式：层名称或层 ID
- 完整模式：JSON 对象，可包含额外选项

**行为**：
1. 验证目标层存在
2. 检查当前层是否有未提交修改
3. 如果目标层是只读的，自动创建新分支
4. 切换到目标层（或新分支）
5. 刷新文件系统视图

**特殊处理**：
- 只读层：自动创建带时间戳的分支
- 未保存修改：拒绝切换或自动创建临时层
- 分支命名：`<original-name>-branch-<timestamp>`

### 丢弃层

**操作**：写入 `/.tarbox/layers/drop`

**输入格式**：
- 简单模式：层名称或层 ID
- 特殊值："current" 表示当前层
- 完整模式：JSON 对象，可指定 force 选项

**行为**：
1. 检查层是否有子层依赖
2. 如果有子层且未指定 force，拒绝删除
3. 如果是当前层，先切换到父层
4. 删除层的元数据和数据引用
5. 触发垃圾回收清理孤立数据块

**安全机制**：
- 有子层时需要明确 force
- 删除前的确认机制
- 无法删除基础层

### 查看层历史

**操作**：读取 `/.tarbox/layers/tree`

**输入**：无（可选：通过 URL 参数指定格式）

**输出**：
- 默认：线性历史列表（从旧到新）
- format=json：JSON 数组
- 标记当前层

**示例输出**：
```
base
├─ checkpoint-1
├─ checkpoint-2
└─ checkpoint-3 [current]
```

## 层跳转策略（单向线性）

### 线性历史模型

**设计原则**：
- 层历史是单向链表：base -> layer1 -> layer2 -> layer3
- 只能在当前最新层（链表尾部）创建新层
- 切换到历史层时自动删除后续所有层
- 保持简单、可预测的行为

### 切换到历史层

**行为**：
```
初始状态：
base -> checkpoint-1 -> checkpoint-2 -> checkpoint-3 [current]

操作：切换到 checkpoint-1
echo "checkpoint-1" > /.tarbox/layers/switch

结果：
base -> checkpoint-1 [current] (checkpoint-2, checkpoint-3 仍然存在但不可见)

可以切换回来：
echo "checkpoint-3" > /.tarbox/layers/switch
结果：base -> checkpoint-1 -> checkpoint-2 -> checkpoint-3 [current]
```

**切换规则**：
1. 切换到历史层时，后续层仍然存在但不可见
2. 可以自由切换回任何历史层或未来层
3. 只有在历史层上创建新层时，才会删除"未来"的层

### 创建新层

**在最新层创建（安全操作）**：
```
当前状态：
base -> checkpoint-1 -> checkpoint-2 [current]

操作：创建新层
echo "checkpoint-3" > /.tarbox/layers/new

结果：
base -> checkpoint-1 -> checkpoint-2 -> checkpoint-3 [current]
```

**在历史层创建（删除未来）**：
```
当前状态：
base -> checkpoint-1 [current] -> checkpoint-2 -> checkpoint-3 (后续层存在)

操作：创建新层
echo "new-direction" > /.tarbox/layers/new

警告：This will delete 2 future layers:
  - checkpoint-2
  - checkpoint-3
Confirm? (use {"name": "new-direction", "confirm": true})

确认后结果：
base -> checkpoint-1 -> new-direction [current]
（checkpoint-2 和 checkpoint-3 被永久删除）
```

### 回退并覆盖工作流程

**场景**：回退到历史点并开始新的工作

**操作流程**：
```
1. 初始状态
   base -> v1 -> v2 -> v3 [current]

2. 发现 v2 有问题，回退到 v1
   echo "v1" > /.tarbox/layers/switch
   
3. 切换后状态（v2、v3 仍然存在）
   base -> v1 [current] -> v2 -> v3

4. 可以先检查状态，确认是否要覆盖
   cat /.tarbox/layers/list  # 查看所有层
   
5. 决定创建新分支，覆盖 v2、v3
   echo "v2-fixed" > /.tarbox/layers/new
   警告：Will delete v2, v3
   
6. 确认创建
   echo '{"name": "v2-fixed", "confirm": true}' > /.tarbox/layers/new
   
7. 最终状态（v2、v3 被永久删除）
   base -> v1 -> v2-fixed [current]
```

**或者，如果只是临时查看历史**：
```
1. 切换到 v1 查看
   echo "v1" > /.tarbox/layers/switch
   
2. 查看文件
   cat /data/some-file.txt
   
3. 决定不修改，切换回最新
   echo "v3" > /.tarbox/layers/switch
   
4. 回到原状态
   base -> v1 -> v2 -> v3 [current]
```

### 安全措施

**创建层时的确认机制**：
- 检测当前层是否为历史层（即后面还有层）
- 如果是，显示警告并要求确认
- 未确认的情况下拒绝创建

**警告信息**：
```
Writing to /.tarbox/layers/new without confirm flag:
Warning: You are at a historical layer. Creating a new layer will delete future layers:
  - checkpoint-2 (created: 2026-01-14 11:00, 50MB, 123 files)
  - checkpoint-3 (created: 2026-01-14 12:00, 30MB, 87 files)

To proceed, write JSON with confirm flag:
{"name": "new-layer", "confirm": true}

Or switch back to the latest layer to preserve history:
echo "checkpoint-3" > /.tarbox/layers/switch
```

**配置选项**：
```toml
[filesystem.hooks.layer_create]
require_confirm_on_history = true   # 在历史层创建时需要确认
auto_confirm = false                # 自动确认（危险）
show_warnings = true                # 显示警告信息
```

## 快照视图

### 设计

**目的**：以只读方式访问历史层的文件

**实现**：
- 每个层在 `/.tarbox/snapshots/<layer-name>/` 下有完整的文件树镜像
- 完全只读，任何写入操作返回错误
- 提供历史版本的随机访问

**用途**：
- 对比当前文件和历史版本
- 从历史版本恢复特定文件
- 审计和检查历史状态

**示例访问**：
```
读取历史文件：
/.tarbox/snapshots/checkpoint-1/data/config.json

对比差异：
diff /data/config.json /.tarbox/snapshots/checkpoint-1/data/config.json

恢复文件：
cp /.tarbox/snapshots/checkpoint-1/data/config.json /data/config.json
```

## 标签系统

### 设计

**目的**：为层提供有意义的别名

**实现**：
- `/.tarbox/layers/tags/` 目录下的符号链接
- 链接名称是标签，目标是层名称
- 可以通过标签名称进行切换和操作

**操作**：
- 创建标签：写入层名到 `/.tarbox/layers/tags/<tag-name>`
- 删除标签：删除对应的文件
- 列出标签：ls `/.tarbox/layers/tags/`
- 通过标签切换：写入标签名到 switch

**典型用例**：
- production：生产环境版本
- staging：测试环境版本
- stable：稳定版本
- last-good：最后一个正常工作的版本

## 审计查询

### 最近操作

**操作**：读取 `/.tarbox/audit/recent`

**输出**：
- 最近 N 条操作记录（默认 100）
- 文本格式，每行一条记录
- 包含时间、操作、路径、用户、结果

### 自定义查询

**操作**：写入查询条件到 `/.tarbox/audit/query`，然后读取结果

**查询格式**：JSON 对象
- operation：操作类型过滤
- path：路径模式
- uid：用户 ID
- time_range：时间范围
- limit：结果数量限制

**输出**：JSON 数组，匹配的审计记录

## 统计信息

### 空间使用

**操作**：读取 `/.tarbox/stats/usage`

**输出**：JSON 对象
- total_size：总空间
- used_size：已用空间
- file_count：文件数量
- layer_count：层数量
- 每层的详细统计

### 性能指标

**操作**：读取 `/.tarbox/stats/performance`

**输出**：JSON 对象
- ops_per_second：每秒操作数
- avg_latency_ms：平均延迟
- cache_hit_rate：缓存命中率
- active_connections：活跃连接数

### 缓存状态

**操作**：读取 `/.tarbox/stats/cache`

**输出**：JSON 对象
- metadata_cache：元数据缓存统计
- block_cache：数据块缓存统计
- path_cache：路径缓存统计
- 每个缓存的大小、使用率、命中率

## 系统控制

### 刷新缓存

**操作**：写入 `/.tarbox/control/flush`

**输入**：
- "all"：刷新所有缓存
- "metadata"：仅刷新元数据缓存
- "blocks"：仅刷新数据块缓存
- "paths"：仅刷新路径缓存

**效果**：清空指定缓存，下次访问重新加载

### 垃圾回收

**操作**：写入 `/.tarbox/control/gc`

**输入**：
- "run"：执行垃圾回收
- "dry-run"：仅报告可回收的空间

**效果**：
- 清理未被任何层引用的数据块
- 整理碎片空间
- 返回回收的空间大小

## 错误处理

### 错误表示

通过标准 Unix 错误码和错误消息传递：

**常见错误码**：
- EINVAL (22)：无效参数（格式错误、JSON 解析失败）
- ENOENT (2)：层不存在
- EACCES (13)：权限拒绝（只读文件写入、只写文件读取）
- EBUSY (16)：资源忙（层正在使用中）
- ENOTEMPTY (39)：目录非空（层有子层时删除）
- EIO (5)：I/O 错误（数据库错误）

**错误消息**：
- 写入失败时，错误信息通过 stderr 输出
- 或在下次读取时返回错误详情

### 错误恢复

**设计原则**：
- 操作应该是原子的
- 失败后系统状态不变
- 提供清晰的错误信息
- 支持重试

## 配置选项

### Hook 系统配置

```
[filesystem.hooks]
enabled: 启用/禁用 hook 系统
base_path: Hook 目录位置（默认 /.tarbox）
show_hidden: ls 时是否显示 hook 目录
permissions_strict: 严格权限检查
```

### 层切换行为

```
[filesystem.hooks.layer_switch]
auto_branch_readonly: 切换到只读层时自动创建分支
confirm_drop: 删除层时需要确认
preserve_current: 切换前保存当前层
branch_naming: 分支命名模式
```

### 审计配置

```
[filesystem.hooks.audit]
recent_limit: recent 文件返回的默认记录数
query_timeout: 查询超时时间
```

## 安全考虑

### 权限模型

**原则**：
- Hook 目录的可见性和权限可配置
- 默认只对挂载所有者可见
- 可选择性地对特定用户/组开放
- 支持只读模式（禁用所有写入 hook）

**级别**：
- 0700：仅所有者
- 0750：所有者和组
- 0755：所有用户只读，所有者读写
- 自定义：细粒度控制

### 审计要求

**记录内容**：
- 所有 hook 操作都记录到审计日志
- 包含操作类型、输入参数、执行结果
- 记录执行者身份（uid/gid/pid）
- 记录时间戳

**用途**：
- 追踪系统状态变化
- 调试问题
- 合规性审查
- 安全分析

### 防御性设计

**保护措施**：
- 输入验证：严格检查输入格式
- 资源限制：限制查询结果大小
- 速率限制：防止滥用
- 原子操作：保证操作的一致性

## 使用场景

### 场景 1：Agent 自动检查点

**需求**：AI Agent 在训练过程中自动创建检查点

**实现思路**：
- 训练开始前：写入 new 创建检查点
- 训练过程中：定期创建检查点
- 训练失败：切换回上一个检查点
- 训练成功：创建成功标记层

**优势**：
- Agent 只需要文件 I/O
- 无需特殊 API 调用
- 易于集成到任何编程语言

### 场景 2：实验分支管理

**需求**：数据科学家进行多个并行实验

**实现思路**：
- 从稳定点创建实验分支
- 每个实验独立工作
- 成功的实验合并回主线
- 失败的实验直接丢弃

**优势**：
- 清晰的分支结构
- 易于可视化（tree）
- 简单的回滚机制

### 场景 3：版本对比和回溯

**需求**：查看历史版本，对比差异

**实现思路**：
- 使用 snapshots 目录访问历史
- 标准 diff 工具对比差异
- 选择性恢复特定文件
- 标签标记重要版本

**优势**：
- 使用标准 Unix 工具
- 不需要学习新命令
- 脚本化容易

### 场景 4：Shell 脚本集成

**需求**：在 Shell 脚本中自动化层管理

**实现思路**：
- 脚本开始创建检查点
- 执行关键操作
- 根据结果决定保留或回滚
- 使用 tree 可视化状态

**优势**：
- 标准 Shell 语法
- 易于调试
- 可移植性好

## 与其他接口的关系

### 与 CLI 工具的对比

**Hook 方式**：
- 优势：零依赖，标准工具，脚本友好
- 劣势：功能相对简单，错误提示有限

**CLI 工具**：
- 优势：丰富的选项，交互式，更好的 UX
- 劣势：需要额外安装，环境依赖

**定位**：
- Hook：基础操作、自动化、脚本
- CLI：高级功能、交互式、人类用户

### 与 API 的对比

**Hook 方式**：
- 优势：语言无关，简单直接
- 劣势：类型安全性弱，异步支持有限

**API（REST/gRPC）**：
- 优势：类型安全，丰富的返回值，更好的错误处理
- 劣势：需要网络，需要客户端库

**定位**：
- Hook：本地操作、简单集成
- API：远程管理、复杂查询、编程集成

## 实现优先级

### 第一阶段（MVP）

**必需功能**：
- current：查看当前层
- new：创建新层
- switch：切换层
- list：列出层

**目标**：基本的层管理能力

### 第二阶段

**扩展功能**：
- drop：删除层
- tree：层树可视化
- snapshots：历史访问
- stats/usage：空间统计

**目标**：完善的层管理体验

### 第三阶段

**高级功能**：
- tags：标签系统
- audit：审计查询
- stats/performance：性能监控
- control：系统控制

**目标**：生产就绪

## 未来扩展

### 事件流

**设计思路**：
- `/.tarbox/events/layers` 提供实时事件流
- 类似 tail -f 的持续输出
- 用于监控和自动化响应

### 触发器系统

**设计思路**：
- 在特定事件发生时执行脚本
- `/.tarbox/triggers/on_layer_create` 等
- 支持自定义自动化工作流

### 交互式文件

**设计思路**：
- 某些操作需要确认
- 通过特殊文件提供交互
- 支持批处理和交互两种模式

### 远程 Hook

**设计思路**：
- 通过网络访问 Hook
- WebSocket 或 HTTP 映射
- 支持远程管理

# 审计系统设计

## 概述

审计系统是 Tarbox 的核心特性之一，专为 AI Agent 设计，提供完整的文件操作追踪和分析能力。

## 设计目标

### 功能目标

- **完整追踪**：记录所有文件系统操作
- **行为分析**：支持 Agent 行为模式分析
- **合规审计**：满足合规性要求
- **问题诊断**：帮助定位和解决问题

### 性能目标

- **低延迟**：审计不阻塞文件操作
- **高吞吐**：支持每秒数万条审计记录
- **可扩展**：支持长期存储和查询

## 审计数据模型

### 审计事件结构

```
AuditEvent {
    // 基本信息
    event_id: 唯一标识符
    timestamp: 事件时间
    
    // 操作信息
    operation: 操作类型
    path: 文件路径
    inode_id: 文件 inode
    
    // 主体信息
    uid: 用户 ID
    gid: 组 ID
    pid: 进程 ID
    process_name: 进程名称
    
    // 结果信息
    success: 是否成功
    error_code: 错误码
    error_message: 错误描述
    
    // 性能信息
    duration_ms: 操作耗时
    bytes_read: 读取字节数
    bytes_written: 写入字节数
    
    // 文本文件变化信息（仅文本文件）
    text_changes: {
        is_text_file: bool,
        lines_added: i32,
        lines_deleted: i32,
        lines_modified: i32,
        old_line_count: i32,
        new_line_count: i32,
        change_summary: String,  // 可选的变化摘要
    }
    
    // 上下文信息
    namespace: 命名空间
    session_id: 会话 ID
    
    // 扩展信息
    metadata: 额外元数据（JSON）
}
```

### 操作类型分类

```
元数据操作：
- getattr: 获取属性
- setattr: 设置属性
- readdir: 读取目录
- lookup: 路径查找

文件操作：
- create: 创建文件
- open: 打开文件
- read: 读取数据
- write: 写入数据
- truncate: 截断文件
- release: 关闭文件
- unlink: 删除文件

目录操作：
- mkdir: 创建目录
- rmdir: 删除目录
- rename: 重命名

权限操作：
- chmod: 修改权限
- chown: 修改所有者

链接操作：
- symlink: 创建符号链接
- hardlink: 创建硬链接
- readlink: 读取链接

扩展属性：
- setxattr: 设置扩展属性
- getxattr: 获取扩展属性
- listxattr: 列出扩展属性
- removexattr: 删除扩展属性
```

## 审计级别

### 级别定义

```
none (0)：
- 不记录任何审计
- 用于性能敏感场景

basic (1)：
- 记录修改操作
  - create, write, delete
  - mkdir, rmdir, rename
  - chmod, chown
- 不记录读取操作
- 适合大多数场景

standard (2)：
- 记录所有文件操作
- 不记录元数据查询（getattr, lookup）
- 平衡性能和完整性

full (3)：
- 记录所有操作
- 包括 getattr, lookup 等高频操作
- 用于深度分析和调试

debug (4)：
- 记录所有操作和详细参数
- 记录内部状态变化
- 仅用于开发调试
```

### 配置方式

```toml
[audit]
# 全局默认级别
level = "standard"

# 按路径配置
[[audit.rules]]
path = "/sensitive/*"
level = "full"

[[audit.rules]]
path = "/tmp/*"
level = "basic"

# 按命名空间配置
[[audit.namespace]]
name = "agent-production"
level = "full"

[[audit.namespace]]
name = "agent-dev"
level = "basic"
```

## 性能优化

### 异步写入

```
设计：
1. 操作完成后立即返回
2. 审计事件放入内存队列
3. 后台线程批量写入数据库

优势：
- 不阻塞文件操作
- 批量写入提高吞吐
- 降低数据库压力

风险缓解：
- 队列有界（防止内存溢出）
- 队列满时采取策略：
  - 丢弃（配置 drop_on_full）
  - 阻塞（配置 block_on_full）
  - 采样（配置 sample_on_full）
```

### 批量插入

```
策略：
- 累积 N 条记录或 T 秒后批量插入
- 使用 PostgreSQL COPY 或批量 INSERT
- 典型配置：1000 条或 1 秒

SQL 示例：
INSERT INTO audit_logs (operation, path, ...)
VALUES
  ('read', '/file1', ...),
  ('write', '/file2', ...),
  ...
```

### 采样策略

```
高频操作采样：
- read, getattr 等操作可能非常频繁
- 配置采样率（如 1%）
- 保留关键信息（首次、最后一次）

智能采样：
- 同文件的连续 read：只记录首次和最后一次
- 相同操作的重复：聚合为一条记录
- 错误操作：始终记录
```

### 分区表

```
时间分区：
- 按天或月分区
- 自动创建未来分区
- 定期清理历史分区

优势：
- 查询性能提升
- 便于归档历史数据
- 删除旧数据更高效
```

## 数据保留策略

### 分级保留

```
近期数据（最近 7 天）：
- 在线查询
- 完整索引
- 高性能存储

常规数据（7-90 天）：
- 在线查询
- 精简索引
- 标准存储

归档数据（90+ 天）：
- 归档到对象存储
- 仅支持批量查询
- 低成本存储
```

### 自动清理

```
配置示例：
[audit.retention]
recent_days = 7
standard_days = 90
archive_days = 365
archive_enabled = true

清理任务：
- 每天运行一次
- 移动过期数据到下一层
- 删除超过保留期的数据
```

## 查询接口

### 基础查询

```rust
// 按时间范围查询
audit.query()
    .time_range(start, end)
    .execute()

// 按 inode 查询
audit.query()
    .inode_id(12345)
    .execute()

// 按操作类型查询
audit.query()
    .operation("write")
    .execute()

// 按用户查询
audit.query()
    .uid(1000)
    .execute()

// 复合查询
audit.query()
    .path("/data/*")
    .operation_in(&["read", "write"])
    .success(true)
    .time_range(start, end)
    .limit(100)
    .execute()
```

### 聚合查询

```rust
// 操作统计
audit.aggregate()
    .group_by("operation")
    .count()
    .avg("duration_ms")
    .execute()

// 用户活动统计
audit.aggregate()
    .group_by("uid")
    .count()
    .sum("bytes_read")
    .sum("bytes_written")
    .execute()

// 时间序列
audit.aggregate()
    .time_bucket("1 hour")
    .count()
    .execute()
```

### 行为分析

```rust
// 文件访问模式
audit.analyze()
    .access_pattern(inode_id)
    .time_range(start, end)
    .execute()
    // 返回：顺序访问/随机访问/读写比例等

// Agent 行为分析
audit.analyze()
    .agent_behavior(namespace)
    .time_range(start, end)
    .execute()
    // 返回：操作分布、活跃时段、异常行为等

// 高频访问文件
audit.analyze()
    .frequent_files()
    .top(100)
    .time_range(start, end)
    .execute()
```

## 实时流式审计

### 设计

```
场景：实时监控 Agent 行为

实现：
1. 审计事件写入队列
2. 订阅者从队列读取
3. 支持过滤和转发

用途：
- 实时告警
- 行为监控
- 安全检测
```

### 接口

```rust
// 订阅审计流
let stream = audit.subscribe()
    .filter(|event| event.operation == "write")
    .subscribe()
    .await;

while let Some(event) = stream.next().await {
    // 处理事件
    process_event(event);
}
```

## 审计可视化

### 时间线视图

```
展示文件的完整操作历史：
- 创建时间
- 所有修改操作
- 访问记录
- 权限变更
```

### 关系图

```
展示文件间的关联：
- 同一会话操作的文件
- 父子关系
- 依赖关系
```

### 访问热度图

```
展示访问模式：
- 按时间的访问分布
- 按文件的访问频率
- 按用户的活动分布
```

## 安全和隐私

### 敏感信息处理

```
策略：
- 可配置的敏感路径列表
- 敏感路径的数据脱敏
- 支持加密存储审计日志

示例：
[audit.privacy]
sensitive_paths = [
    "/home/*/secrets/*",
    "/data/private/*"
]
mask_path = true          # 路径脱敏
mask_content = true       # 内容脱敏
```

### 访问控制

```
审计日志访问控制：
- 只有管理员可以查询所有审计
- 用户只能查询自己的审计
- 支持基于角色的访问控制（RBAC）
```

## 与其他组件集成

### 与 FUSE 层集成

```
集成点：
- 每个 FUSE 操作完成后调用审计
- 传递操作上下文（用户、路径、结果）
- 异步记录，不阻塞返回

代码模式：
fn fuse_operation(...) -> Result<T> {
    let start = Instant::now();
    let result = do_operation(...);
    
    // 异步记录审计
    audit::record_async(AuditEvent {
        operation: "write",
        path: path,
        success: result.is_ok(),
        duration_ms: start.elapsed().as_millis(),
        ...
    });
    
    result
}
```

### 与分层文件系统集成

```
审计数据用于分析：
- 统计文件访问频率
- 识别高频访问文件
- 分析层使用模式

查询示例：
SELECT inode_id, COUNT(*) as access_count
FROM audit_logs
WHERE operation IN ('read', 'write')
  AND created_at > NOW() - INTERVAL '30 days'
GROUP BY inode_id
ORDER BY access_count DESC;
```

### 与告警系统集成

```
异常检测规则：
- 短时间大量删除操作
- 异常的访问模式
- 权限异常变更
- 非预期的用户访问

触发告警：
- 写入告警队列
- 发送通知（邮件、Webhook）
- 执行自动化响应
```

## 性能指标

### 目标指标

```
写入性能：
- 峰值吞吐：50k events/s
- P99 延迟：< 10ms（异步）
- 队列深度：< 10k events

查询性能：
- 简单查询：< 100ms
- 聚合查询：< 1s
- 复杂分析：< 10s

存储：
- 每事件大小：~ 500 bytes
- 每天 1M 操作：~ 500MB/day
- 90 天保留：~ 45GB
```

## 测试策略

### 功能测试

```
- 各级别审计记录正确性
- 异步写入可靠性
- 查询接口正确性
- 数据保留策略
```

### 性能测试

```
- 高并发写入压力测试
- 队列满载测试
- 批量查询性能测试
- 长期运行稳定性
```

### 边界测试

```
- 队列溢出处理
- 数据库连接失败恢复
- 极大审计量场景
- 分区自动创建
```

## 运维工具

### CLI 工具

```bash
# 查询审计
tarbox audit query \
  --path "/data/*" \
  --operation write \
  --time-range "1h"

# 统计
tarbox audit stats \
  --group-by operation \
  --time-range "24h"

# 导出
tarbox audit export \
  --format json \
  --output audit.json \
  --time-range "7d"

# 清理
tarbox audit cleanup \
  --before "90d" \
  --dry-run
```

### 监控指标

```
Prometheus 指标：
- tarbox_audit_events_total: 总事件数
- tarbox_audit_queue_size: 队列大小
- tarbox_audit_write_latency: 写入延迟
- tarbox_audit_dropped_total: 丢弃事件数
```

## 文本文件审计

### 文本文件变化追踪

```
对于文本文件的修改操作，审计系统记录额外的细节信息：

标准审计字段：
- operation: write
- path: /app/config.yaml
- bytes_written: 1250
- success: true

文本文件扩展字段（text_changes）：
{
    "is_text_file": true,
    "lines_added": 5,
    "lines_deleted": 2,
    "lines_modified": 3,
    "old_line_count": 100,
    "new_line_count": 103,
    "change_summary": "+5 -2 ~3 lines in configuration section"
}

优势：
- 清晰了解文件变化幅度
- 便于识别大规模修改
- 支持代码审查和变更追踪
- 辅助异常行为检测
```

### 文本变化审计示例

```json
{
    "event_id": "uuid",
    "timestamp": "2026-01-15T10:30:00Z",
    "operation": "write",
    "path": "/data/config.yaml",
    "inode_id": 12345,
    "uid": 1000,
    "gid": 1000,
    "pid": 5678,
    "process_name": "agent-worker",
    "success": true,
    "duration_ms": 15,
    "bytes_written": 1250,
    "text_changes": {
        "is_text_file": true,
        "lines_added": 5,
        "lines_deleted": 2,
        "lines_modified": 3,
        "old_line_count": 100,
        "new_line_count": 103,
        "change_summary": "Modified database configuration section"
    },
    "metadata": {
        "layer_id": "layer-003",
        "parent_layer_id": "layer-002",
        "file_type": "yaml",
        "encoding": "UTF-8"
    }
}
```

### 查询文本文件变化

```rust
// 查询大规模修改
audit.query()
    .text_file_only()
    .filter("text_changes.lines_added > 100")
    .execute()

// 查询特定文件类型的修改
audit.query()
    .path("*.yaml")
    .text_file_only()
    .time_range(start, end)
    .execute()

// 统计代码变化
audit.aggregate()
    .text_file_only()
    .sum("text_changes.lines_added")
    .sum("text_changes.lines_deleted")
    .group_by("metadata.file_type")
    .execute()

// 结果示例：
// {
//     "py": { "lines_added": 1500, "lines_deleted": 800 },
//     "rs": { "lines_added": 2300, "lines_deleted": 1200 },
//     "yaml": { "lines_added": 200, "lines_deleted": 50 }
// }
```

### SQL 查询示例

```sql
-- 查询今天修改最多的文本文件
SELECT 
    path,
    SUM((metadata->'text_changes'->>'lines_added')::int) as total_added,
    SUM((metadata->'text_changes'->>'lines_deleted')::int) as total_deleted,
    COUNT(*) as modify_count
FROM audit_logs
WHERE operation = 'write'
  AND metadata->'text_changes'->>'is_text_file' = 'true'
  AND created_at > CURRENT_DATE
GROUP BY path
ORDER BY total_added + total_deleted DESC
LIMIT 10;

-- 查询某个 Agent 的代码产出
SELECT 
    namespace,
    SUM((metadata->'text_changes'->>'lines_added')::int) as lines_written,
    COUNT(DISTINCT path) as files_modified
FROM audit_logs
WHERE operation = 'write'
  AND metadata->'text_changes'->>'is_text_file' = 'true'
  AND created_at > NOW() - INTERVAL '7 days'
GROUP BY namespace
ORDER BY lines_written DESC;

-- 识别大规模重写（可能的异常行为）
SELECT 
    path,
    created_at,
    (metadata->'text_changes'->>'lines_deleted')::int as deleted,
    (metadata->'text_changes'->>'lines_added')::int as added
FROM audit_logs
WHERE operation = 'write'
  AND metadata->'text_changes'->>'is_text_file' = 'true'
  AND (metadata->'text_changes'->>'lines_deleted')::int > 1000
ORDER BY created_at DESC;
```

### 异常检测规则

```
基于文本文件变化的异常检测：

1. 大规模删除：
   - 单次操作删除超过 1000 行
   - 可能是误删除或恶意操作
   - 触发告警

2. 频繁重写：
   - 同一文件短时间内多次大幅修改
   - 可能是脚本错误或调试问题
   - 记录并通知

3. 异常文件类型：
   - 修改不应该被修改的文件（如 .so, .pyc）
   - 文本文件突然变为二进制文件
   - 触发安全审查

4. 产出异常：
   - Agent 长时间无代码产出（lines_added = 0）
   - 可能是卡死或工作异常
   - 监控告警

规则配置示例：
[audit.anomaly_detection]
enable_text_analysis = true

[[audit.anomaly_detection.rules]]
name = "massive_deletion"
condition = "text_changes.lines_deleted > 1000"
action = "alert"
severity = "high"

[[audit.anomaly_detection.rules]]
name = "frequent_rewrite"
condition = "same_file_modified_count > 10 in 1m"
action = "log"
severity = "medium"
```

### 可视化支持

```
文本变化可视化：

1. 时间线视图：
   - X 轴：时间
   - Y 轴：行数变化（+增加，-删除）
   - 展示文件的演化历史

2. 热力图：
   - 不同文件的修改频率
   - 颜色深度表示修改幅度
   - 识别活跃的文件

3. 产出统计：
   - Agent 每日代码产出统计
   - 不同文件类型的占比
   - 团队产出对比

4. 差异摘要：
   - 每次修改的 +/- 统计
   - 类似 GitHub 的 contribution graph
   - 按文件类型分组显示
```

### 性能考虑

```
文本变化计算开销：

1. 写入时计算：
   - 在文件写入时同步计算 diff
   - 增加约 5-10ms 延迟（取决于文件大小）
   - 只影响文本文件写操作

2. 优化策略：
   - 异步计算：写入后台计算
   - 采样：大文件只采样部分行
   - 缓存：缓存最近的文件版本
   - 阈值：超大文件跳过详细分析

3. 存储开销：
   - text_changes 字段约 100-200 bytes
   - 相对于整体审计记录，开销可接受
   - 可选：配置是否启用文本分析

配置示例：
[audit.text_analysis]
enabled = true
max_file_size = "1MB"       # 超过此大小跳过
async_compute = true         # 异步计算
include_summary = true       # 包含变化摘要
```


## 未来增强

### 智能分析

```
- 机器学习识别异常行为
- 自动生成访问模式报告
- 预测性分析（预测文件访问）
```

### 分布式追踪

```
- 集成 OpenTelemetry
- 跨服务追踪
- 分布式上下文传播
```

### 实时告警

```
- 复杂事件处理（CEP）
- 自定义告警规则
- 多种通知渠道
```

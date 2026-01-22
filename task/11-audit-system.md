# Task 11: 审计系统

## 状态

**📅 计划中**

## 目标

实现完整的审计系统，在 Task 06 (数据库层) 的基础上，增加：
- **异步批量写入**: 内存队列 + 后台批量插入
- **审计级别控制**: none/basic/standard/full/debug 五级
- **采样策略**: 高频操作智能采样
- **查询接口增强**: 行为分析、聚合查询、时间序列
- **实时流式审计**: 支持订阅和过滤
- **文本文件变化追踪**: 行级diff统计
- **FUSE层集成**: 自动记录所有文件操作

**注意**: Task 06 已实现审计日志的数据库表、基础CRUD和分区管理，本任务实现审计的业务逻辑和高级功能。

## 优先级

**P1 - 高优先级**

审计系统是 Tarbox 的核心特性之一，对 AI Agent 的行为追踪和分析至关重要。

## 依赖

- Task 06: 数据库层高级功能 ✅ (audit_logs表、分区管理)
- Task 05: FUSE 接口 ✅ (需要在 FUSE 层集成审计)
- Task 08: 分层文件系统 ✅ (需要审计层操作)

## 依赖的Spec

- **spec/03-audit-system.md** - 审计系统完整设计（核心）
- spec/02-fuse-interface.md - FUSE层集成点
- spec/04-layered-filesystem.md - 层操作审计
- spec/10-text-file-optimization.md - 文本文件变化追踪
- spec/07-performance.md - 性能优化策略

## 实现内容

### 1. 审计配置系统

- [ ] **审计级别定义** (`src/audit/level.rs`)
  - AuditLevel 枚举 (None, Basic, Standard, Full, Debug)
  - 每个级别包含的操作类型
  - 级别判断逻辑 (should_audit_operation)

- [ ] **审计配置** (`src/audit/config.rs`)
  - AuditConfig 结构体
  - 全局默认级别
  - 按路径配置规则 (path pattern -> level)
  - 按租户配置 (tenant_id -> level)
  - 采样率配置
  - 队列大小配置
  - 从 TOML 加载配置

- [ ] **配置文件支持** (`config/audit.toml`)
  ```toml
  [audit]
  level = "standard"
  queue_size = 10000
  batch_size = 1000
  batch_timeout_ms = 1000
  
  [[audit.rules]]
  path = "/sensitive/*"
  level = "full"
  
  [[audit.rules]]
  path = "/tmp/*"
  level = "basic"
  
  [audit.sampling]
  read_operations = 0.01  # 1% 采样
  getattr_operations = 0.001  # 0.1% 采样
  ```

### 2. 异步批量写入

- [ ] **审计事件队列** (`src/audit/queue.rs`)
  - AuditEvent 结构体（扩展 CreateAuditLogInput）
  - 有界队列 (tokio::sync::mpsc::channel)
  - 队列满时策略（Drop/Block/Sample）
  - 队列统计 (dropped_count, queued_count)

- [ ] **批量写入器** (`src/audit/writer.rs`)
  - AuditWriter 结构体
  - 后台 tokio 任务
  - 批量累积逻辑 (N条或T秒)
  - 使用 PostgreSQL COPY 或批量 INSERT
  - 错误重试机制
  - 优雅关闭 (flush pending)

- [ ] **审计服务** (`src/audit/service.rs`)
  - AuditService 结构体（全局单例）
  - record() 方法（发送到队列）
  - record_sync() 方法（同步写入，用于关键操作）
  - flush() 方法（强制刷新队列）
  - shutdown() 方法（优雅关闭）
  - 统计信息 (total_recorded, total_dropped)

### 3. 采样策略

- [ ] **采样器** (`src/audit/sampler.rs`)
  - Sampler trait
  - RandomSampler - 随机采样
  - RateLimiter - 限流采样
  - SmartSampler - 智能采样
    - 同文件连续read：只记录首次和最后一次
    - 错误操作：始终记录
    - 首次访问：始终记录

- [ ] **采样规则配置**
  - 按操作类型配置采样率
  - 按路径pattern配置采样率
  - 错误操作豁免采样

### 4. 文本文件变化追踪

- [ ] **文本变化计算** (`src/audit/text_changes.rs`)
  - TextChanges 结构体
    ```rust
    pub struct TextChanges {
        pub is_text_file: bool,
        pub lines_added: i32,
        pub lines_deleted: i32,
        pub lines_modified: i32,
        pub old_line_count: i32,
        pub new_line_count: i32,
        pub change_summary: Option<String>,
    }
    ```
  - compute_text_diff() 函数（使用 similar crate）
  - 异步计算支持
  - 大文件跳过阈值
  - 结果序列化为 JSON（存入 metadata 字段）

- [ ] **集成到文件写入**
  - FileSystem::write_file() 调用时计算 diff
  - 仅对文本文件计算
  - 配置开关控制是否启用

### 5. 查询接口增强

- [ ] **高级查询构建器** (`src/audit/query.rs`)
  - AuditQueryBuilder
  - 链式API (.time_range(), .operation(), .path())
  - 文本文件过滤 (.text_file_only())
  - 排序和分页
  - 执行返回 Vec\<AuditLog\>

- [ ] **聚合查询** (`src/audit/aggregate.rs`)
  - AggregateQuery
  - group_by() - 按字段分组
  - time_bucket() - 时间序列分桶
  - count(), sum(), avg(), min(), max()
  - 执行返回聚合结果

- [ ] **行为分析** (`src/audit/analysis.rs`)
  - access_pattern() - 文件访问模式分析
  - agent_behavior() - Agent 行为统计
  - frequent_files() - 高频访问文件
  - anomaly_detection() - 异常行为检测
    - 短时间大量删除
    - 大规模文本修改
    - 异常访问模式

### 6. 实时流式审计

- [ ] **审计流** (`src/audit/stream.rs`)
  - AuditStream 结构体
  - 基于 tokio::sync::broadcast
  - subscribe() 方法（返回 Stream）
  - filter() 方法（过滤条件）
  - 支持多个订阅者

- [ ] **流式API**
  ```rust
  let stream = audit.subscribe()
      .filter(|event| event.operation == "write")
      .await;
  
  while let Some(event) = stream.next().await {
      process_event(event);
  }
  ```

### 7. FUSE 层集成

- [ ] **FUSE 操作审计** (`src/fuse/audit.rs`)
  - audit_fuse_operation() 辅助函数
  - 在每个 FUSE 操作完成后调用
  - 记录操作类型、路径、结果、耗时
  - 异步发送到审计队列

- [ ] **集成点改造**
  - 修改 src/fuse/adapter.rs 中的所有操作
  - 添加计时和结果记录
  - 示例：
    ```rust
    fn read(&self, path: &str, ...) -> Result<Vec<u8>> {
        let start = Instant::now();
        let result = self.backend.read_file(path).await;
        
        audit::record_async(AuditEvent {
            operation: "read",
            path: path.to_string(),
            success: result.is_ok(),
            duration_ms: start.elapsed().as_millis() as i64,
            bytes_read: result.as_ref().map(|d| d.len()).unwrap_or(0) as i64,
            error_message: result.as_ref().err().map(|e| e.to_string()),
            ...
        });
        
        result
    }
    ```

### 8. CLI 工具集成

- [ ] **审计查询命令** (`src/cli/audit.rs`)
  - `tarbox audit query` - 查询审计日志
    - --path <pattern>
    - --operation <op>
    - --time-range <range>
    - --limit <n>
  - `tarbox audit stats` - 统计信息
    - --group-by <field>
    - --time-range <range>
  - `tarbox audit export` - 导出审计
    - --format json|csv
    - --output <file>
  - `tarbox audit cleanup` - 清理旧数据
    - --before <date>
    - --dry-run

- [ ] **实时监控命令**
  - `tarbox audit watch` - 实时观察审计事件
    - --filter <expr>
    - 类似 `tail -f`

### 9. 监控和指标

- [ ] **Prometheus 指标** (`src/audit/metrics.rs`)
  - tarbox_audit_events_total (counter) - 总事件数
  - tarbox_audit_queue_size (gauge) - 队列大小
  - tarbox_audit_write_latency (histogram) - 写入延迟
  - tarbox_audit_dropped_total (counter) - 丢弃事件数
  - tarbox_audit_batch_size (histogram) - 批量大小

- [ ] **健康检查**
  - 队列是否正常
  - 写入器是否运行
  - 丢弃率是否过高

### 10. 测试

- [ ] **单元测试**
  - 审计级别判断逻辑
  - 采样器逻辑
  - 文本变化计算
  - 查询构建器

- [ ] **集成测试** (`tests/audit_system_integration_test.rs`)
  - test_async_batch_write - 异步批量写入
  - test_queue_full_strategy - 队列满载策略
  - test_audit_levels - 审计级别控制
  - test_sampling - 采样策略
  - test_text_changes_tracking - 文本变化追踪
  - test_advanced_query - 高级查询
  - test_aggregate_stats - 聚合统计
  - test_real_time_stream - 实时流
  - test_fuse_integration - FUSE 集成
  - test_performance_high_load - 高负载性能测试

- [ ] **性能测试** (`benches/audit_benchmark.rs`)
  - 批量写入吞吐量（目标: >50k events/s）
  - 查询性能（简单查询 <100ms）
  - 队列性能（P99 延迟 <10ms）

## 架构要点

### 异步写入架构

```
┌─────────────┐
│ FUSE 操作   │
└──────┬──────┘
       │ audit::record_async()
       ▼
┌─────────────────┐
│ Bounded Queue   │ (10k events)
│ (mpsc channel)  │
└──────┬──────────┘
       │ background task
       ▼
┌─────────────────┐
│ Batch Accumulator│ (1000 events or 1s)
└──────┬──────────┘
       │ bulk insert
       ▼
┌─────────────────┐
│ PostgreSQL      │
│ (audit_logs)    │
└─────────────────┘
```

### 审计级别决策树

```
operation = "getattr"
  └─> level = none/basic? -> skip
  └─> level = standard? -> skip
  └─> level = full/debug? -> record

operation = "write"
  └─> level = none? -> skip
  └─> level >= basic? -> record

operation = "read"
  └─> level = none/basic? -> skip
  └─> level >= standard? -> record (with sampling)
```

### 文本变化集成

```rust
// In FileSystem::write_file()
if is_text_file(&path) && audit_config.enable_text_analysis {
    let old_content = self.read_file(&path).await.ok();
    let text_changes = compute_text_diff(old_content, new_content);
    
    audit_event.metadata.insert("text_changes", json!(text_changes));
}
```

## 性能目标

根据 spec/03-audit-system.md：

- **写入性能**:
  - 峰值吞吐: 50k events/s
  - P99 延迟: <10ms (异步)
  - 队列深度: <10k events

- **查询性能**:
  - 简单查询: <100ms
  - 聚合查询: <1s
  - 复杂分析: <10s

- **存储**:
  - 每事件大小: ~500 bytes
  - 每天 1M 操作: ~500MB/day
  - 90 天保留: ~45GB

## 验收标准

### 核心功能
- [ ] 审计级别配置系统实现
- [ ] 异步批量写入正常工作
- [ ] 队列满载策略正确执行
- [ ] 采样策略正确工作
- [ ] 文本变化追踪准确
- [ ] 高级查询接口完整
- [ ] 实时流式审计可用
- [ ] FUSE 层集成完成
- [ ] CLI 工具完整

### 质量标准
- [ ] 单元测试覆盖率 >55%
- [ ] 集成测试覆盖率 >25%
- [ ] 总覆盖率 >80%
- [ ] 性能测试达标
- [ ] cargo fmt 通过
- [ ] cargo clippy 无警告
- [ ] 所有测试通过

### 性能标准
- [ ] 异步写入吞吐 >50k events/s
- [ ] P99 延迟 <10ms
- [ ] 简单查询 <100ms
- [ ] 高负载下无丢失（在队列容量内）

## 文件清单

### 新增文件
```
src/audit/
├── mod.rs              - 模块导出
├── level.rs            - 审计级别
├── config.rs           - 审计配置
├── queue.rs            - 事件队列
├── writer.rs           - 批量写入器
├── service.rs          - 审计服务
├── sampler.rs          - 采样策略
├── text_changes.rs     - 文本变化计算
├── query.rs            - 高级查询
├── aggregate.rs        - 聚合查询
├── analysis.rs         - 行为分析
├── stream.rs           - 实时流
└── metrics.rs          - Prometheus指标

src/fuse/
└── audit.rs            - FUSE审计集成

src/cli/
└── audit.rs            - CLI审计命令

config/
└── audit.toml          - 审计配置文件

tests/
├── audit_system_integration_test.rs  - 系统集成测试
└── audit_performance_test.rs         - 性能测试

benches/
└── audit_benchmark.rs  - 性能基准
```

### 修改文件
- src/fuse/adapter.rs - 添加审计调用
- src/cli/main.rs - 添加审计子命令
- Cargo.toml - 添加依赖 (similar, prometheus-client)

## 技术栈

- **tokio** - 异步运行时、mpsc队列
- **similar** - 文本 diff 计算
- **prometheus-client** - 监控指标
- **sqlx** - 批量数据库操作
- **serde_json** - JSON序列化（text_changes）
- **clap** - CLI参数解析

## 未实现内容（推迟到未来）

以下功能不在本任务范围，推迟到未来需求明确时：

- **智能分析**: 机器学习识别异常行为
- **分布式追踪**: OpenTelemetry 集成
- **复杂告警**: CEP (复杂事件处理)
- **审计日志加密**: 敏感信息加密存储
- **审计可视化**: Web UI、时间线、关系图
- **多渠道通知**: 邮件、Webhook、Slack

## 后续任务

完成后可以开始：
- Task 12: Kubernetes CSI 驱动（需要审计 K8s 操作）
- Task 13: REST API（需要审计 API 调用）
- 性能调优和监控系统

## 参考资料

- spec/03-audit-system.md - 完整设计文档
- PostgreSQL 分区表最佳实践
- Tokio async patterns
- similar crate 文档（文本 diff）
- Prometheus 最佳实践

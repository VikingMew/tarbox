# Task 06: 数据库存储层高级功能

## 目标

实现 PostgreSQL 存储层，包括数据库连接管理、表结构初始化、基础 CRUD 操作。

## 优先级

**P0 - 最高优先级**

## 依赖

- Task 01: 项目初始化和基础设施搭建

## 子任务

### 2.1 数据库连接管理

- [ ] 实现连接池管理
  - 使用 sqlx 的连接池
  - 配置连接数、超时等参数
  - 实现健康检查
  - 实现连接重试机制

- [ ] 实现数据库初始化
  - 检查数据库连接
  - 验证 PostgreSQL 版本（>= 14）
  - 初始化扩展（如需要）

### 2.2 数据库迁移系统

- [ ] 创建迁移目录结构
  - `migrations/` - 迁移脚本目录
  - 使用 sqlx-cli 或自定义迁移工具

- [ ] 编写迁移脚本
  - `001_create_tenants.sql` - 租户表
  - `002_create_inodes.sql` - inode 表
  - `003_create_data_blocks.sql` - 数据块表
  - `004_create_audit_logs.sql` - 审计日志表（带分区）
  - `005_create_layers.sql` - 层表
  - `006_create_text_blocks.sql` - 文本块表
  - `007_create_text_metadata.sql` - 文本文件元数据表
  - `008_create_text_line_map.sql` - 文本行映射表
  - `009_create_indexes.sql` - 创建索引

- [ ] 实现迁移管理
  - 应用迁移
  - 回滚迁移
  - 查询迁移状态

### 2.3 租户管理

- [ ] 实现租户 CRUD 操作
  - `create_tenant()` - 创建租户
  - `get_tenant()` - 查询租户
  - `update_tenant()` - 更新租户
  - `delete_tenant()` - 删除租户
  - `list_tenants()` - 列出租户

- [ ] 实现租户配额管理
  - 检查配额
  - 更新使用量
  - 配额告警

### 2.4 Inode 操作

- [ ] 实现 inode CRUD 操作
  - `create_inode()` - 创建 inode
  - `get_inode()` - 查询 inode
  - `update_inode()` - 更新 inode
  - `delete_inode()` - 删除 inode
  - `get_inode_by_parent_and_name()` - 路径解析

- [ ] 实现 inode 批量操作
  - `batch_get_inodes()` - 批量查询
  - `list_children()` - 列出子节点

- [ ] 实现 inode 统计
  - 计算目录大小
  - 统计文件数量

### 2.5 数据块操作

- [ ] 实现二进制数据块操作
  - `create_block()` - 创建数据块
  - `get_block()` - 读取数据块
  - `delete_block()` - 删除数据块
  - `list_blocks()` - 列出文件的所有块

- [ ] 实现内容寻址
  - 基于 SHA-256 的去重
  - 引用计数管理

### 2.6 文本文件存储

- [ ] 实现文本块操作
  - `create_text_block()` - 创建文本块
  - `get_text_block()` - 读取文本块
  - `batch_get_text_blocks()` - 批量读取
  - 基于内容哈希去重

- [ ] 实现文本文件元数据操作
  - `create_text_metadata()` - 创建元数据
  - `get_text_metadata()` - 查询元数据
  - `update_text_metadata()` - 更新元数据

- [ ] 实现文本行映射操作
  - `create_line_map()` - 创建行映射
  - `get_line_map()` - 查询行映射
  - `batch_create_line_map()` - 批量创建

### 2.7 层管理

- [ ] 实现层 CRUD 操作
  - `create_layer()` - 创建层
  - `get_layer()` - 查询层
  - `update_layer()` - 更新层
  - `delete_layer()` - 删除层
  - `list_layers()` - 列出所有层
  - `get_layer_chain()` - 获取层链

- [ ] 实现层条目操作
  - `add_layer_entry()` - 添加层条目
  - `list_layer_entries()` - 列出层的所有条目

### 2.8 审计日志

- [ ] 实现审计日志写入
  - `create_audit_log()` - 创建审计记录
  - `batch_create_audit_logs()` - 批量创建
  - 异步批量插入

- [ ] 实现审计日志查询
  - `query_audit_logs()` - 条件查询
  - `aggregate_audit_stats()` - 聚合统计
  - 支持时间范围过滤
  - 支持多条件组合

- [ ] 实现审计日志分区管理
  - 自动创建分区
  - 清理过期分区

### 2.9 事务支持

- [ ] 实现事务封装
  - `begin_transaction()` - 开启事务
  - `commit_transaction()` - 提交事务
  - `rollback_transaction()` - 回滚事务

- [ ] 实现关键操作的事务保证
  - 文件创建（inode + 数据块）
  - 文件移动（更新多个 inode）
  - 层创建（层 + 层条目）

### 2.10 性能优化

- [ ] 实现预编译语句
  - 常用查询使用 prepared statements
  - 减少 SQL 解析开销

- [ ] 实现批量操作
  - 批量插入优化
  - 使用 COPY 命令（如适用）

- [ ] 实现查询优化
  - 分析慢查询
  - 添加必要的索引
  - 使用 EXPLAIN ANALYZE 验证

### 2.11 监控和健康检查

- [ ] 实现数据库健康检查
  - 连接池状态
  - 查询延迟
  - 错误率统计

- [ ] 实现 Prometheus 指标导出
  - 连接池指标
  - 查询延迟指标
  - 操作计数指标

### 2.12 测试

- [ ] 单元测试
  - 每个数据库操作的测试
  - 使用测试数据库
  - Mock 连接池（如需要）

- [ ] 集成测试
  - 完整的数据库操作流程
  - 事务一致性测试
  - 并发操作测试

- [ ] 性能测试
  - 批量插入性能
  - 查询性能基准
  - 连接池压力测试

## 验收标准

- [ ] 所有数据库表正确创建
- [ ] 所有 CRUD 操作实现并测试通过
- [ ] 事务正确工作
- [ ] 支持租户隔离
- [ ] 集成测试全部通过
- [ ] 性能满足目标要求
- [ ] 文档完整

## 预估时间

5-7 天

## 技术栈

- sqlx 0.8
- PostgreSQL 14+
- tokio 异步运行时

## 注意事项

- 所有查询必须包含 `tenant_id` 过滤
- 使用参数化查询防止 SQL 注入
- 大批量操作要考虑分批处理
- 注意处理数据库连接失败和重试
- 审计日志分区要提前创建
- 文本文件相关表要正确处理引用计数

## 后续任务

完成后可以开始：
- Task 03: 基础文件系统实现
- Task 04: FUSE 接口实现

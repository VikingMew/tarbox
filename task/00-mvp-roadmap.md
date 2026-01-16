# Task 00: MVP 开发路线图

## MVP 目标

实现一个最小可用版本，通过命令行工具创建租户空间并执行基础的 POSIX 文件操作。

## MVP 范围

### 包含功能
- ✅ 租户管理（创建、查询、删除）
- ✅ 基础目录操作（mkdir, ls, rmdir）
- ✅ 基础文件操作（touch, write, cat, rm）
- ✅ 简单的路径解析
- ✅ PostgreSQL 存储（最小 schema）
- ✅ CLI 工具

### 不包含功能
- ❌ FUSE 接口
- ❌ 分层文件系统
- ❌ 审计日志
- ❌ 文本文件优化
- ❌ 原生挂载
- ❌ 权限检查
- ❌ 符号链接和硬链接
- ❌ 缓存
- ❌ 并发控制
- ❌ Kubernetes 集成

## MVP 任务顺序

### Phase 1: 基础设施（已完成）
- **Task 01**: 项目初始化和基础设施搭建 ✅

### Phase 2: 存储层（3-4 天）
- **Task 02**: 数据库存储层（MVP）
  - 连接池管理
  - 最小 schema（tenants, inodes, data_blocks）
  - 基础 CRUD 操作
  - 事务支持

### Phase 3: 文件系统核心（3-4 天）
- **Task 03**: 基础文件系统实现（MVP）
  - 路径解析
  - 目录操作（create, list, remove）
  - 文件操作（create, write, read, delete）
  - 元数据操作（stat, chmod, chown）
  - 错误处理

### Phase 4: CLI 工具（2-3 天）
- **Task 04**: CLI 工具实现（MVP）
  - 租户管理命令
  - 文件系统操作命令
  - 输出格式化
  - 帮助系统

## MVP 验收标准

### 功能验收
```bash
# 1. 初始化数据库
tarbox init
# ✓ 所有表创建成功

# 2. 创建租户
tarbox tenant create test-agent
# ✓ 返回 tenant_id
# ✓ 自动创建根目录

# 3. 创建目录结构
tarbox --tenant test-agent mkdir /data
tarbox --tenant test-agent mkdir /data/logs
tarbox --tenant test-agent mkdir /data/models
# ✓ 目录创建成功

# 4. 列出目录
tarbox --tenant test-agent ls /
# ✓ 显示：data/
tarbox --tenant test-agent ls /data
# ✓ 显示：logs/, models/

# 5. 创建和写入文件
tarbox --tenant test-agent touch /data/config.txt
tarbox --tenant test-agent write /data/config.txt "key=value"
# ✓ 文件创建和写入成功

# 6. 读取文件
tarbox --tenant test-agent cat /data/config.txt
# ✓ 输出：key=value

# 7. 查看文件信息
tarbox --tenant test-agent stat /data/config.txt
# ✓ 显示大小、权限、时间戳

# 8. 删除文件
tarbox --tenant test-agent rm /data/config.txt
# ✓ 文件删除成功

# 9. 删除目录
tarbox --tenant test-agent rmdir /data/logs
# ✓ 空目录删除成功

# 10. 租户信息
tarbox tenant info test-agent
# ✓ 显示租户统计信息
```

### 质量验收
- [ ] 所有单元测试通过
- [ ] 所有集成测试通过
- [ ] 测试覆盖率 > 80%
- [ ] Clippy 无警告
- [ ] 代码格式正确

### 性能验收
- [ ] 创建 1000 个文件 < 10s
- [ ] 读取 1000 个文件 < 10s
- [ ] 单个文件读写延迟 < 100ms

## MVP 后续规划

### Phase 5: FUSE 接口（7-10 天）
- **Task 05**: FUSE 接口实现
  - 实现标准 POSIX 接口
  - 挂载点管理
  - 与文件系统核心集成

### Phase 6: 高级功能（按需）
- **Task 06**: 数据库层完善（审计、分层、文本优化）
- **Task 07**: 文件系统核心完善（权限、链接、缓存）
- **Task 08**: 分层文件系统（COW、检查点、历史）
- **Task 09**: 原生挂载支持
- **Task 10**: Kubernetes CSI 驱动

## 当前状态

- ✅ Task 01: 已完成
- ✅ Task 02: 已完成
- ✅ Task 03: 已完成
- ✅ Task 04: 已完成（MVP 完成！）

## 预计时间线

- Task 02: 3-4 天
- Task 03: 3-4 天  
- Task 04: 2-3 天

**总计**: 8-11 天完成 MVP

## 注意事项

1. **保持简单**: MVP 阶段不追求完美，聚焦核心功能
2. **快速迭代**: 尽快完成可工作的版本，然后迭代改进
3. **测试驱动**: 每个功能都要有测试
4. **文档同步**: 代码和文档保持同步
5. **遵循原则**: Linus 和 Carmack 的编程哲学

## 风险和挑战

1. **路径解析复杂度**: 需要正确处理各种边界情况
2. **事务管理**: 确保数据一致性
3. **错误处理**: 提供清晰的错误信息
4. **性能**: PostgreSQL 作为存储的性能开销

## 里程碑

- **M1**: Task 02 完成 - 数据库层可用
- **M2**: Task 03 完成 - 文件系统核心可用  
- **M3**: Task 04 完成 - MVP 交付，可以通过 CLI 完整使用

## 成功标准

MVP 成功的标志是：**任何开发者可以通过简单的 CLI 命令管理文件系统，无需了解内部实现细节**。

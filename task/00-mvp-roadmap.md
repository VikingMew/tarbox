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

### 功能验收 ✅ 已完成

**CLI 操作（已验证）：**
```bash
# 1. 初始化数据库 ✅
tarbox init

# 2. 创建租户 ✅
tarbox tenant create test-agent

# 3. 创建目录结构 ✅
tarbox --tenant test-agent mkdir /data
tarbox --tenant test-agent mkdir /data/logs

# 4. 列出目录 ✅
tarbox --tenant test-agent ls /
tarbox --tenant test-agent ls /data

# 5. 创建和写入文件 ✅
tarbox --tenant test-agent touch /data/config.txt
tarbox --tenant test-agent write /data/config.txt "key=value"

# 6. 读取文件 ✅
tarbox --tenant test-agent cat /data/config.txt

# 7. 查看文件信息 ✅
tarbox --tenant test-agent stat /data/config.txt

# 8. 删除文件 ✅
tarbox --tenant test-agent rm /data/config.txt

# 9. 删除目录 ✅
tarbox --tenant test-agent rmdir /data/logs

# 10. 租户管理 ✅
tarbox tenant info test-agent
tarbox tenant list
tarbox tenant delete test-agent
```

**FUSE 挂载（新增）：**
```bash
# 11. 挂载文件系统 ✅
tarbox --tenant test-agent mount /mnt/tarbox

# 12. 通过标准工具访问（实际挂载后可用）
ls /mnt/tarbox
cat /mnt/tarbox/data/config.txt
echo "hello" > /mnt/tarbox/data/test.txt

# 13. 卸载文件系统 ✅
tarbox umount /mnt/tarbox
```

### 质量验收
- [x] 所有单元测试通过（94 tests）
- [x] 所有 E2E 测试编写完成（63 tests，需要数据库）
- [x] Clippy 无警告
- [x] 代码格式正确（cargo fmt）
- [x] 编译成功（debug 和 release）

### 性能验收
- ⏳ 需要实际性能测试（Phase 3）

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

### ✅ Phase 1: MVP 核心（已完成）- 2026-01-18
- ✅ Task 01: 项目初始化和基础设施 (2026-01-15)
- ✅ Task 02: 数据库存储层（MVP）(2026-01-15)
- ✅ Task 03: 文件系统核心（MVP）(2026-01-15)
- ✅ Task 04: CLI 工具（MVP）(2026-01-17)
- ✅ Task 05: FUSE 接口实现 (2026-01-18)

**里程碑 M1 达成**: 可通过 CLI 完整管理文件系统
**里程碑 M2 达成**: 可作为真实文件系统挂载使用（FUSE）

### 📅 Phase 2: 高级功能（计划中）
- 📅 Task 06: 数据库层高级功能（审计、分层、文本优化）
- 📅 Task 07: 文件系统核心高级功能（权限、链接、缓存）
- 📅 Task 08: 分层文件系统（COW、检查点、历史）

### 🚀 Phase 3: 生产就绪（计划中）
- 📅 Task 09: CLI 工具高级功能（快照、审计查询等）
- 📅 Task 10: 性能优化和监控
- 📅 Task 11: 安全加固和配额管理

**里程碑 M3**: 生产环境就绪

### ☸️ Phase 4: 云原生集成（计划中）
- 📅 Task 12: Kubernetes CSI 驱动 (预计 4-5 周)
- 📅 Task 13: REST API (预计 2-3 周)
- 📅 Task 14: gRPC API (预计 2 周)
- 📅 Task 15: WASI 支持 (预计 8-12 周)

**里程碑 M4**: 云原生全面支持

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

MVP 成功的标志是：**任何开发者可以通过简单的 CLI 命令管理文件系统，或通过 FUSE 挂载使用标准 Unix 工具访问，无需了解内部实现细节**。

## ✅ MVP 已完成总结

### 完成时间
**2026-01-15 至 2026-01-18** (共 4 天)

### 已实现功能

#### 1. 数据库存储层
- PostgreSQL 连接池管理
- 租户、inode、数据块的完整 CRUD 操作
- 事务支持
- 内容哈希去重

#### 2. 文件系统核心
- 路径解析和验证
- 目录操作（创建、列出、删除）
- 文件操作（创建、读写、删除）
- 元数据操作（stat, chmod, chown）
- 错误处理和类型安全

#### 3. FUSE 接口
- FilesystemInterface 抽象层（90% 代码可被 CSI/WASI 复用）
- TarboxBackend 实现（完整 POSIX 操作）
- FuseAdapter（fuser trait 实现）
- 挂载管理（mount/unmount）
- 异步桥接（tokio → 同步 FUSE）

#### 4. CLI 工具
- 租户管理命令（create, list, info, delete）
- 文件系统操作命令（mkdir, ls, rmdir, touch, write, cat, rm, stat）
- FUSE 挂载命令（mount, umount）
- 数据库初始化（init）
- 完整帮助系统

### 代码统计
- 总源文件：20 个 Rust 文件
- 单元测试：94 tests (100% pass)
- E2E 测试：63 tests (需要数据库)
- 代码质量：通过 fmt 和 clippy 检查
- 编译状态：成功（debug 和 release）

### 技术架构
- 语言：Rust 1.92+ (Edition 2024)
- 数据库：PostgreSQL 14+
- FUSE：fuser 0.16
- 异步运行时：tokio
- CLI 框架：clap
- 测试框架：内置 + 实际数据库

### 未来扩展方向
- Phase 2: 审计系统、分层文件系统、文本优化
- Phase 3: 高级权限、缓存、性能优化
- Phase 4: Kubernetes CSI、REST/gRPC API、WASI 支持

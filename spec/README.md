# Tarbox 规范文档

## 文档结构

本目录包含 Tarbox 项目的详细设计规范，按照实现优先级组织。

## 优先级说明

### P0 - MVP 核心功能（必须实现）

这些规范是 Tarbox 的核心，必须在 MVP 阶段实现：

- **00-overview.md** - 项目概述和架构总览
- **01-database-schema.md** - PostgreSQL 数据库设计（核心）
- **02-fuse-interface.md** - FUSE 接口实现
- **09-multi-tenancy.md** - 多租户隔离设计

### P1 - 核心增强功能（MVP 后优先实现）

这些功能在 MVP 之后应该优先实现：

- **03-audit-system.md** - 审计日志系统
- **04-layered-filesystem.md** - 分层文件系统（COW）
- **08-filesystem-hooks.md** - 文件系统钩子（层控制）

### P2 - 高级功能（后续实现）

这些功能可以在基础功能稳定后实现：

- **05-kubernetes-csi.md** - Kubernetes CSI 驱动
- **06-api-design.md** - REST/gRPC API
- **07-performance.md** - 性能优化策略
- **10-text-file-optimization.md** - 文本文件优化（行级 diff）

### P3 - 可选优化（可延后或替代）

这些功能优先级较低，可以延后实现或使用替代方案：

- **11-dependencies.md** - 依赖管理
- **12-native-mounting.md** - 原生目录挂载（可用 bubblewrap 替代）

## 实现路线图

### 阶段 1: MVP 核心（当前）
- [x] Task 01: 项目初始化
- [x] Task 02: 数据库层（基于 spec 01）
- [x] Task 03: 文件系统核心（基于 spec 02 部分）
- [ ] Task 04: CLI 工具
- [ ] Task 05: FUSE 接口（基于 spec 02）

### 阶段 2: 核心增强
- [ ] 审计系统（spec 03）
- [ ] 分层文件系统（spec 04）
- [ ] 文件系统钩子（spec 08）

### 阶段 3: 云原生
- [ ] Kubernetes CSI（spec 05）
- [ ] API 服务（spec 06）

### 阶段 4: 高级优化
- [ ] 性能优化（spec 07）
- [ ] 文本文件优化（spec 10）

## 替代方案说明

### Native Mounting (spec 12)
**原计划**：在 Tarbox 内部实现原生目录挂载功能

**替代方案**：使用 **bubblewrap** 在容器层面实现
- bubblewrap 可以在启动容器时挂载宿主目录
- 无需在 Tarbox 内部实现复杂的挂载逻辑
- 更符合单一职责原则
- 性能相同甚至更好

**示例**：
```bash
# 使用 bubblewrap 挂载系统目录
bwrap \
  --ro-bind /usr /usr \
  --ro-bind /bin /bin \
  --bind /path/to/.venv /.venv \
  --bind /tarbox/mount /data \
  <command>
```

因此 spec 12 优先级降为 **P3**，可能不需要实现。

## 阅读顺序建议

1. **新手入门**：
   - 00-overview.md（了解整体架构）
   - 09-multi-tenancy.md（理解隔离模型）
   - 01-database-schema.md（理解数据模型）

2. **开发者**：
   - 按照优先级顺序阅读（P0 → P1 → P2）
   - 参考 task/ 目录中的实现任务

3. **运维人员**：
   - 05-kubernetes-csi.md（K8s 部署）
   - 07-performance.md（性能调优）
   - 03-audit-system.md（审计和合规）

## 文档维护

- 每个 spec 文件应保持设计文档的特性（描述"什么"和"为什么"）
- 不应包含具体的代码实现
- 实现细节应该在代码注释和 task/ 目录中
- 当设计发生变化时，及时更新相关 spec

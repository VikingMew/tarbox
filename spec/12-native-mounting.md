# Spec 12: 原生目录挂载

> **本规范已合并到 [spec/07-performance.md](07-performance.md)**
>
> 请查看 spec/07 中的"原生文件系统挂载"章节。

## 快速导航

- [完整文档：spec/07-performance.md](07-performance.md#原生文件系统挂载)

## 为什么合并？

原生目录挂载本质上是一种性能优化手段，通过绕过 PostgreSQL 直接访问原生文件系统来提升 I/O 性能。将其合并到性能优化规范中可以：

1. **统一性能策略**：与其他性能优化手段（缓存、批量操作、并发控制）放在一起便于整体考量
2. **减少重复**：性能考虑、缓存策略等内容原本会在两个文档中重复
3. **简化架构**：减少独立规范数量，降低维护成本

## 推荐替代方案

如 spec/07 中所述，推荐使用 **bubblewrap** 等容器工具在外部实现目录挂载：

```bash
bwrap \
  --ro-bind /usr /usr \
  --ro-bind /bin /bin \
  --bind /host/venv /.venv \
  --bind /tarbox/tenant/data /data \
  /bin/bash
```

**理由**：
- 单一职责原则 - Tarbox 专注于文件系统本身
- 性能相同或更好 - bubblewrap 直接在内核层面操作
- 减少复杂度 - 无需在 Tarbox 中维护挂载逻辑
- 更灵活 - 容器编排层可以自由控制挂载策略

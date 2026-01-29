# Spec 12: 原生目录挂载 (已废弃)

> **⚠️ 本 Spec 已废弃**
>
> **替代方案**: 参见 **[Spec 18: 文件系统组合](./18-filesystem-composition.md)**
>
> 原 bubblewrap 方案无法满足跨 Tenant Layer 组合的需求，新方案在 Tarbox 内部实现文件系统组合功能。

---

## 历史记录（以下内容已废弃）

### 原决策

Tarbox **不实现**原生目录挂载功能。所有原生目录挂载通过容器编排工具（Docker, Kubernetes）使用 bubblewrap 完成。

### 原因

1. **单一职责**: Tarbox 专注于文件系统，容器挂载由容器运行时负责
2. **降低开发量**: 无需在 Tarbox 中维护挂载逻辑、权限检查、路径匹配
3. **性能更好**: bubblewrap 直接在内核层面操作，零开销
4. **更灵活**: 编排层可根据需求自由配置挂载策略

### 废弃原因（2026-01-29）

bubblewrap 方案存在以下局限：

1. **无法跨 Layer 组合**：只能绑定宿主机目录，无法从 Tarbox 的不同 Layer 组合文件系统
2. **无法共享 Layer**：不同 Tenant 无法共享同一个只读 Layer（如预训练模型）
3. **运行时限制**：需要容器支持，裸机部署困难
4. **动态性差**：挂载配置在容器启动时固定，运行时无法调整

新方案（Spec 18）在 Tarbox 内部实现文件系统组合，支持：
- 宿主机目录挂载
- 跨 Tenant Layer 挂载
- 共享只读 Layer
- 写时复制（COW）
- 运行时动态配置

## 实现方式

### Docker 示例

```bash
docker run \
  --security-opt apparmor=unconfined \
  tarbox/runtime:latest \
  bwrap \
    --bind /tarbox/mount/{tenant_id} / \
    --ro-bind /usr /usr \
    --ro-bind /bin /bin \
    --bind /host/venvs/{tenant_id} /.venv \
    /bin/bash
```

### Kubernetes 示例

```yaml
apiVersion: v1
kind: Pod
spec:
  containers:
  - name: agent
    command:
    - bwrap
    - --bind
    - /tarbox/mount/my-tenant
    - /
    - --ro-bind
    - /usr
    - /usr
    - /bin/python
    - /workspace/main.py
```

## Tarbox 视角

- Tarbox 通过 FUSE 提供挂载点：`/tarbox/mount/{tenant_id}`
- bubblewrap 在容器内将此挂载点绑定到 `/`
- 其他系统目录（`/usr`, `/bin`, `/venvs`）由 bubblewrap 处理
- **Tarbox 完全无感知，只提供标准 POSIX 文件系统**

## 相关文档

- CLAUDE.md - 架构说明（Native directory mounting 部分）
- spec/07-performance.md - 性能优化（不包含内部实现）

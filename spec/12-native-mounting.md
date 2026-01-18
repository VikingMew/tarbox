# Spec 12: 原生目录挂载 (已废弃)

> **⚠️ 本功能不在 Tarbox 中实现**
>
> **推荐方案**: 使用 **bubblewrap** 在容器层实现目录挂载

## 决策

Tarbox **不实现**原生目录挂载功能。所有原生目录挂载通过容器编排工具（Docker, Kubernetes）使用 bubblewrap 完成。

## 原因

1. **单一职责**: Tarbox 专注于文件系统，容器挂载由容器运行时负责
2. **降低开发量**: 无需在 Tarbox 中维护挂载逻辑、权限检查、路径匹配
3. **性能更好**: bubblewrap 直接在内核层面操作，零开销
4. **更灵活**: 编排层可根据需求自由配置挂载策略

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

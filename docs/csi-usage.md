# Kubernetes CSI 使用指南

本文档介绍如何在 Kubernetes 中使用 Tarbox CSI (Container Storage Interface) 驱动。

---

## ☸️ 什么是 CSI？

CSI (Container Storage Interface) 是 Kubernetes 的存储插件标准，允许第三方存储系统集成到 K8s 中提供持久化卷。

## Tarbox CSI 的优势

- **分层持久化**: 每个 PVC 支持快照和版本历史
- **多租户隔离**: K8s namespace 自动映射到 Tarbox tenant
- **文本优化存储**: Git-like 行级 diff 存储，节省空间
- **审计日志**: 所有文件操作自动记录，满足合规要求
- **内容去重**: 跨 PVC 的块级去重，共享相同数据

---

## 架构概览

```
Kubernetes Pod
       ↓
PersistentVolumeClaim (PVC)
       ↓
CSI Driver (tarbox.csi.io)
       ↓
Tarbox Filesystem
       ↓
PostgreSQL (Multi-tenant Storage)
```

### CSI 组件

#### 1. Identity Service
**位置**: `src/csi/identity.rs`

提供 CSI 驱动基本信息：
- 驱动名称: `tarbox.csi.io`
- 支持的能力: Controller + Node Service
- 版本信息和健康检查

#### 2. Controller Service
**位置**: `src/csi/controller.rs`

集中式卷管理（运行在控制平面）：
- `CreateVolume` - 创建 PV（创建租户 + 初始层）
- `DeleteVolume` - 删除 PV（删除租户）
- `CreateSnapshot` - 创建快照（创建新层）
- `DeleteSnapshot` - 删除快照（删除层）
- `ListVolumes` - 列出所有卷
- `ControllerGetCapabilities` - 能力查询

#### 3. Node Service
**位置**: `src/csi/node.rs`

节点本地操作（运行在每个 K8s 节点）：
- `NodePublishVolume` - 挂载卷到 Pod
- `NodeUnpublishVolume` - 卸载卷
- `NodeGetCapabilities` - 节点能力查询
- `NodeGetInfo` - 节点信息

---

## 部署到 Kubernetes

### 前置要求

- Kubernetes 1.20+
- PostgreSQL 16+ (集群外部或内部)
- Snapshot CRD（如需快照功能）

### 使用 Helm 部署（推荐）

```bash
# 1. 克隆仓库
git clone https://github.com/vikingmew/tarbox.git
cd tarbox

# 2. 配置数据库连接
# 编辑 charts/tarbox-csi/values.yaml
cat <<EOF > my-values.yaml
database:
  url: "postgres://user:password@postgres-host:5432/tarbox"

# 可选：配置镜像
image:
  repository: your-registry/tarbox
  tag: latest
EOF

# 3. 安装 CSI 驱动
helm install tarbox-csi ./charts/tarbox-csi \
  --namespace kube-system \
  --values my-values.yaml

# 4. 验证安装
kubectl get csidrivers
kubectl get pods -n kube-system -l app.kubernetes.io/name=tarbox-csi
```

### 手动部署

```bash
# 1. 创建命名空间
kubectl create namespace tarbox-system

# 2. 创建数据库连接 Secret
kubectl create secret generic tarbox-db \
  --from-literal=DATABASE_URL="postgres://user:pass@host/tarbox" \
  -n tarbox-system

# 3. 部署 CSI 组件
kubectl apply -f deploy/csi/csidriver.yaml
kubectl apply -f deploy/csi/controller-rbac.yaml
kubectl apply -f deploy/csi/controller-deployment.yaml
kubectl apply -f deploy/csi/node-rbac.yaml
kubectl apply -f deploy/csi/node-daemonset.yaml

# 4. 验证
kubectl get pods -n tarbox-system
kubectl get csidrivers tarbox.csi.io
```

---

## 基本使用

### 创建 StorageClass

```yaml
apiVersion: storage.k8s.io/v1
kind: StorageClass
metadata:
  name: tarbox
provisioner: tarbox.csi.io
parameters:
  # Tarbox 特定参数（当前为空，未来可扩展）
  fsType: "tarbox"
volumeBindingMode: Immediate
allowVolumeExpansion: false  # 当前不支持扩容
reclaimPolicy: Delete
```

### 创建 PersistentVolumeClaim

```yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: ai-workspace
  namespace: default
spec:
  accessModes:
    - ReadWriteOnce  # 仅支持 RWO
  storageClassName: tarbox
  resources:
    requests:
      storage: 10Gi
```

### 在 Pod 中使用卷

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: ai-agent
  namespace: default
spec:
  containers:
  - name: agent
    image: my-ai-agent:latest
    volumeMounts:
    - name: workspace
      mountPath: /workspace
    command:
    - sh
    - -c
    - |
      # 文件会持久化到 Tarbox
      echo "Hello from pod" > /workspace/hello.txt
      cat /workspace/hello.txt
      sleep 3600
  volumes:
  - name: workspace
    persistentVolumeClaim:
      claimName: ai-workspace
```

部署并验证：
```bash
kubectl apply -f pod.yaml
kubectl exec ai-agent -- cat /workspace/hello.txt
# 输出: Hello from pod
```

---

## 快照管理

### 创建 VolumeSnapshotClass

```yaml
apiVersion: snapshot.storage.k8s.io/v1
kind: VolumeSnapshotClass
metadata:
  name: tarbox-snapshot
driver: tarbox.csi.io
deletionPolicy: Delete
```

### 创建快照（保存检查点）

```yaml
apiVersion: snapshot.storage.k8s.io/v1
kind: VolumeSnapshot
metadata:
  name: ai-workspace-checkpoint-1
  namespace: default
spec:
  volumeSnapshotClassName: tarbox-snapshot
  source:
    persistentVolumeClaimName: ai-workspace
```

查看快照状态：
```bash
kubectl get volumesnapshot
kubectl describe volumesnapshot ai-workspace-checkpoint-1
```

### 从快照恢复

```yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: ai-workspace-restored
  namespace: default
spec:
  dataSource:
    name: ai-workspace-checkpoint-1
    kind: VolumeSnapshot
    apiGroup: snapshot.storage.k8s.io
  accessModes:
    - ReadWriteOnce
  storageClassName: tarbox
  resources:
    requests:
      storage: 10Gi
```

---

## 高级用法

### 多租户隔离

Tarbox CSI 使用以下策略映射 K8s 资源到租户：

```
Kubernetes Namespace → Tarbox Tenant
PVC Name → Volume ID (metadata)
VolumeSnapshot → Tarbox Layer
```

示例：
```bash
# namespace: team-a → tenant_id: <uuid-for-team-a>
# PVC: workspace-001 → volume_id: pvc-workspace-001
# Snapshot: checkpoint-1 → layer_id: <uuid-for-layer>
```

### 在 Deployment 中使用

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: ai-workers
  namespace: default
spec:
  replicas: 1  # 注意: RWO 模式只支持单 Pod
  selector:
    matchLabels:
      app: ai-worker
  template:
    metadata:
      labels:
        app: ai-worker
    spec:
      containers:
      - name: worker
        image: ai-worker:v1
        volumeMounts:
        - name: workspace
          mountPath: /workspace
      volumes:
      - name: workspace
        persistentVolumeClaim:
          claimName: ai-workspace
```

### StatefulSet 示例

```yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: ai-agents
  namespace: default
spec:
  serviceName: ai-agents
  replicas: 3
  selector:
    matchLabels:
      app: ai-agent
  template:
    metadata:
      labels:
        app: ai-agent
    spec:
      containers:
      - name: agent
        image: ai-agent:v1
        volumeMounts:
        - name: workspace
          mountPath: /workspace
  volumeClaimTemplates:
  - metadata:
      name: workspace
    spec:
      accessModes: ["ReadWriteOnce"]
      storageClassName: tarbox
      resources:
        requests:
          storage: 5Gi
```

---

## 监控和运维

### 查看 CSI 组件状态

```bash
# 控制器 Pod
kubectl get pods -n kube-system -l app=tarbox-csi-controller

# 节点 DaemonSet
kubectl get pods -n kube-system -l app=tarbox-csi-node

# CSI 驱动注册
kubectl get csidrivers tarbox.csi.io
```

### 查看日志

```bash
# 控制器日志
kubectl logs -n kube-system -l app=tarbox-csi-controller -c tarbox-plugin

# 节点插件日志
kubectl logs -n kube-system -l app=tarbox-csi-node -c tarbox-plugin

# 查看最近错误
kubectl logs -n kube-system -l app=tarbox-csi-controller --tail=50 | grep -i error
```

### Prometheus 监控

CSI 驱动导出以下指标（如启用 `--metrics-addr`）：

```bash
# 在 controller/node deployment 中启用 metrics
--metrics-addr=:9090

# Prometheus 抓取配置
kubectl port-forward -n kube-system deploy/tarbox-csi-controller 9090:9090
curl http://localhost:9090/metrics
```

可用指标：
- `tarbox_csi_operations_total{method="CreateVolume"}` - 操作计数
- `tarbox_csi_operation_duration_seconds{method="CreateVolume"}` - 操作延迟
- `tarbox_csi_operation_errors_total{method="CreateVolume"}` - 操作错误
- `tarbox_volume_count{namespace="default"}` - 各命名空间卷数量
- `tarbox_volume_capacity{volume_id="..."}` - 卷容量（字节）
- `tarbox_volume_used{volume_id="..."}` - 卷已用空间（字节）
- `tarbox_mount_count{volume_id="..."}` - 活跃挂载数
- `tarbox_mount_duration_seconds` - 挂载操作延迟
- `tarbox_snapshot_count` - 总快照数
- `tarbox_snapshot_size{snapshot_id="..."}` - 快照大小（字节）

### 事件查看

```bash
# 查看 PVC 事件
kubectl describe pvc ai-workspace

# 查看卷挂载事件
kubectl describe pod ai-agent

# 查看快照事件
kubectl describe volumesnapshot ai-workspace-checkpoint-1
```

---

## 故障排查

### Pod 无法挂载卷

**现象**: Pod 卡在 `ContainerCreating` 状态

**排查步骤**:
```bash
# 1. 查看 Pod 事件
kubectl describe pod <pod-name>

# 2. 检查 CSI 节点插件
kubectl get pods -n kube-system -l app=tarbox-csi-node
kubectl logs -n kube-system <tarbox-csi-node-pod> -c tarbox-plugin

# 3. 检查 PVC 状态
kubectl get pvc
kubectl describe pvc <pvc-name>

# 4. 验证数据库连接
kubectl exec -n kube-system deploy/tarbox-csi-controller -- \
  tarbox tenant list
```

### 卷创建失败

**现象**: PVC 卡在 `Pending` 状态

**排查步骤**:
```bash
# 1. 检查 StorageClass
kubectl get storageclass tarbox
kubectl describe storageclass tarbox

# 2. 检查控制器日志
kubectl logs -n kube-system -l app=tarbox-csi-controller -c tarbox-plugin --tail=100

# 3. 验证数据库可访问性
kubectl exec -n kube-system deploy/tarbox-csi-controller -- \
  psql $DATABASE_URL -c "SELECT 1"
```

### 快照失败

**现象**: VolumeSnapshot 状态为 `Failed`

**排查步骤**:
```bash
# 1. 查看快照事件
kubectl describe volumesnapshot <snapshot-name>

# 2. 检查 snapshot-controller
kubectl get pods -n kube-system -l app=snapshot-controller

# 3. 验证 VolumeSnapshotClass
kubectl get volumesnapshotclass tarbox-snapshot
```

### 性能问题

**现象**: 卷 I/O 延迟高

**优化建议**:
```bash
# 1. 检查数据库性能
# 在 PostgreSQL 中执行
EXPLAIN ANALYZE SELECT * FROM inodes WHERE tenant_id = '...';

# 2. 调整 Tarbox 缓存配置（如未启用）
# 在 deployment 中添加环境变量（通过配置文件或环境变量）
# 注意：当前缓存配置需要通过 config.toml 文件设置
# 默认值: max_entries=10000, ttl_seconds=300

# 3. 监控数据库连接数
kubectl exec -n kube-system deploy/tarbox-csi-controller -- \
  psql $DATABASE_URL -c "SELECT count(*) FROM pg_stat_activity;"
```

---

## CSI 合规性测试

Tarbox 使用 [csi-sanity](https://github.com/kubernetes-csi/csi-test) 进行 CSI 合规性测试。

### 本地测试

```bash
# 1. 启动 Tarbox CSI 服务器
DATABASE_URL="postgres://localhost/tarbox" \
  cargo run --release -- csi \
  --mode=all \
  --endpoint=unix:///tmp/csi.sock \
  --node-id=test-node

# 2. 在另一个终端运行测试
./scripts/csi-sanity-test.sh
```

### CI 集成

CSI 合规性测试已集成到 E2E 测试流程：
```bash
# 查看 .github/workflows/e2e.yml
# 每次 push 会自动运行 csi-sanity 测试
```

---

## 限制和注意事项

### 当前支持的功能
- ✅ Dynamic provisioning (动态供应)
- ✅ Volume snapshots (卷快照)
- ✅ Snapshot restore (快照恢复)
- ✅ Volume deletion (卷删除)
- ✅ ReadWriteOnce (RWO) 访问模式

### 当前限制
- ❌ 不支持 ReadWriteMany (RWX) 访问模式
- ❌ 不支持 ReadOnlyMany (ROX) 访问模式
- ❌ 不支持卷扩容 (`allowVolumeExpansion: false`)
- ❌ 不支持卷克隆（计划未来支持）
- ❌ 不支持原始块卷（Block volume）

### 设计决策
- **RWO 限制**: Tarbox 的层系统设计为单写入者模型
- **无扩容**: 存储配额由租户级别控制，不支持运行时扩展
- **快照为只读**: 符合 CSI 规范，快照用于恢复而非直接挂载

---

## 卸载

```bash
# 1. 删除所有使用 Tarbox 的 PVC 和 Pod
kubectl delete pvc --all -n <namespace>

# 2. 删除快照
kubectl delete volumesnapshot --all -n <namespace>

# 3. 卸载 Helm release
helm uninstall tarbox-csi -n kube-system

# 或手动删除资源
kubectl delete -f deploy/csi/

# 4. 删除 CSIDriver
kubectl delete csidriver tarbox.csi.io

# 5. 清理数据库（可选，会删除所有数据）
# psql $DATABASE_URL -c "DROP SCHEMA public CASCADE; CREATE SCHEMA public;"
```

---

## 相关文档

- [架构设计 - Kubernetes CSI](../spec/07-kubernetes-csi.md)
- [开发任务 - CSI 驱动实现](../task/13-kubernetes-csi-driver.md)
- [开发任务 - CSI 测试](../task/18-csi-sanity-testing.md)
- [Helm Chart 配置](../charts/tarbox-csi/README.md)

---

## 外部资源

- [Kubernetes CSI 文档](https://kubernetes-csi.github.io/docs/)
- [CSI 规范](https://github.com/container-storage-interface/spec)
- [CSI 驱动开发指南](https://kubernetes-csi.github.io/docs/developing.html)

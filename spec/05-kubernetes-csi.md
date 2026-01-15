# Kubernetes CSI 驱动设计

## 概述

CSI（Container Storage Interface）是 Kubernetes 的标准存储接口。Tarbox 通过实现 CSI 驱动，使得可以在 Kubernetes 中作为持久卷使用。

## 设计目标

### 功能目标

- **动态供应**：自动创建和删除卷
- **多租户隔离**：不同命名空间/PVC 独立存储
- **自动租户管理**：从 PVC 自动创建租户
- **ReadWriteMany**：支持多个 Pod 同时挂载
- **快照支持**：卷快照和恢复
- **卷扩展**：在线扩容

### 非功能目标

- **高可用**：驱动本身无单点故障
- **性能**：最小化额外开销
- **易部署**：标准 Helm Chart 部署
- **可观测**：完整的监控和日志

## 多租户集成

### PVC 到租户的自动映射

**命名规则**：
- 租户名称格式：`<namespace>-<pvc-name>`
- 确保全局唯一性
- 符合 DNS 命名规范

**示例**：
```yaml
# PVC 定义
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: agent-storage
  namespace: ai-agents
spec:
  storageClassName: tarbox
  resources:
    requests:
      storage: 10Gi

# 自动创建租户
# 租户名称：ai-agents-agent-storage
# 租户 ID：自动生成的 UUID
```

### 租户生命周期管理

**CreateVolume 时**：
1. 提取 namespace 和 PVC name
2. 生成租户名称：`{namespace}-{pvc-name}`
3. 检查租户是否已存在
4. 如果不存在，创建租户记录
5. 初始化租户的根 inode 和基础层
6. 设置配额（从 PVC capacity）
7. 返回 volume_id（实际上是 tenant_id）

**DeleteVolume 时**：
1. 从 volume_id 获取 tenant_id
2. 标记租户为 'deleted'
3. 拒绝新的访问
4. 等待所有活跃挂载卸载
5. 异步清理租户数据
6. 删除租户记录

### 跨命名空间隔离

**Kubernetes 原生隔离**：
- PVC 只能在同一 namespace 内使用
- ServiceAccount 权限限制访问范围
- RBAC 控制谁可以创建/删除 PVC

**Tarbox 额外保证**：
- 即使绕过 Kubernetes RBAC
- 数据库层面通过 tenant_id 完全隔离
- 不同 namespace 的 PVC 无法互相访问

**示例**：
```
namespace: team-a
  PVC: data-storage -> 租户: team-a-data-storage

namespace: team-b  
  PVC: data-storage -> 租户: team-b-data-storage

虽然 PVC 名称相同，但租户完全不同，数据完全隔离
```

### 配额管理

**从 PVC 设置配额**：
```yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: agent-storage
spec:
  resources:
    requests:
      storage: 10Gi  # 自动设置为租户的 quota_bytes

# 结果：租户的 quota_bytes = 10 * 1024^3
```

**配额超限处理**：
- 写入时检查配额
- 超限返回 ENOSPC
- Pod 内应用收到 "No space left on device" 错误
- 可以通过扩容 PVC 来增加配额

## CSI 架构

### 组件划分

```
┌─────────────────────────────────────────┐
│         Kubernetes Control Plane        │
│  ┌──────────────────────────────────┐   │
│  │   External Provisioner           │   │
│  │   External Attacher              │   │
│  │   External Snapshotter           │   │
│  │   External Resizer               │   │
│  └────────────┬─────────────────────┘   │
└───────────────┼─────────────────────────┘
                │ gRPC
┌───────────────▼─────────────────────────┐
│      Tarbox CSI Controller              │
│  ┌──────────────────────────────────┐   │
│  │  Identity Service                │   │
│  │  Controller Service              │   │
│  │    - CreateVolume                │   │
│  │    - DeleteVolume                │   │
│  │    - ControllerPublishVolume     │   │
│  │    - CreateSnapshot              │   │
│  │    - ExpandVolume                │   │
│  └──────────────────────────────────┘   │
└─────────────────────────────────────────┘

┌─────────────────────────────────────────┐
│      Tarbox CSI Node (DaemonSet)        │
│  ┌──────────────────────────────────┐   │
│  │  Identity Service                │   │
│  │  Node Service                    │   │
│  │    - NodeStageVolume             │   │
│  │    - NodePublishVolume           │   │
│  │    - NodeUnpublishVolume         │   │
│  │    - NodeUnstageVolume           │   │
│  │    - NodeGetVolumeStats          │   │
│  └──────────────────────────────────┘   │
│  ┌──────────────────────────────────┐   │
│  │    FUSE Mount Manager            │   │
│  └──────────────────────────────────┘   │
└─────────────────────────────────────────┘
```

### 部署模式

```
Controller 部署：
- Deployment（2-3 副本）
- 高可用（Leader Election）
- 负责卷的创建、删除、快照等

Node 部署：
- DaemonSet（每个节点一个）
- 负责卷的挂载和卸载
- 运行 FUSE 文件系统
```

## CSI 接口实现

### Identity Service

```protobuf
service Identity {
    rpc GetPluginInfo(GetPluginInfoRequest)
        returns (GetPluginInfoResponse) {}
    
    rpc GetPluginCapabilities(GetPluginCapabilitiesRequest)
        returns (GetPluginCapabilitiesResponse) {}
    
    rpc Probe(ProbeRequest)
        returns (ProbeResponse) {}
}
```

实现要点：
```
GetPluginInfo:
- name: tarbox.csi.io
- vendor_version: 0.1.0

GetPluginCapabilities:
- CONTROLLER_SERVICE
- VOLUME_ACCESSIBILITY_CONSTRAINTS
- EXPAND_VOLUME

Probe:
- 检查数据库连接
- 检查 FUSE 可用性
- 返回健康状态
```

### Controller Service

#### CreateVolume

```protobuf
rpc CreateVolume(CreateVolumeRequest)
    returns (CreateVolumeResponse) {}
```

流程：
```
1. 解析请求参数
   - capacity_range: 容量需求
   - parameters: 自定义参数
   - secrets: 数据库连接信息
   - volume_name: PVC 名称
   - 从 parameters 或 topology 提取 namespace

2. 生成租户名称
   - tenant_name = "{namespace}-{pvc-name}"
   - 生成 tenant_id (UUID)

3. 创建租户（如果不存在）
   - INSERT INTO tenants (tenant_id, tenant_name, quota_bytes, ...)
   - 创建租户的根 inode (tenant_id, inode_id=1)
   - 创建基础层 (base layer)
   - 初始化配额计数器

4. 返回卷信息
   - volume_id: tenant_id（作为卷的唯一标识）
   - capacity_bytes: 配额大小
   - volume_context: {"tenant_id": "...", "tenant_name": "..."}
```

参数支持：
```yaml
parameters:
  # 数据库配置
  databaseURL: "postgresql://..."
  
  # Layer 配置
  autoCheckpoint: "false"
  checkpointInterval: "1h"
  
  # 审计配置
  auditLevel: "standard"
  auditRetentionDays: "90"
  
  # 性能配置
  cacheSize: "1Gi"
  blockSize: "4096"
  
  # 命名空间
  namespace: "agent-001"
  
  # 原生挂载配置（TOML 格式）
  nativeMounts: |
    [[native_mounts]]
    path = "/bin"
    source = "/bin"
    mode = "ro"
    shared = true
    
    [[native_mounts]]
    path = "/usr"
    source = "/usr"
    mode = "ro"
    shared = true
    
    [[native_mounts]]
    path = "/.venv"
    source = "/var/tarbox/venvs/{tenant_id}"
    mode = "rw"
    shared = false
```

原生挂载处理：
```
CreateVolume 时：
1. 解析 nativeMounts 参数（TOML 格式）
2. 对于每个 [[native_mounts]] 条目：
   - 如果 shared=false，插入 native_mounts 表（关联 tenant_id）
   - 如果 shared=true，检查是否已存在全局挂载
     - 不存在则创建全局挂载（tenant_id=NULL）
     - 已存在则跳过
3. 挂载时 FUSE 会自动加载这些配置

DeleteVolume 时：
1. 删除该租户的专属挂载（shared=false 且 tenant_id 匹配）
2. 保留共享挂载（shared=true）
```

#### DeleteVolume

```protobuf
rpc DeleteVolume(DeleteVolumeRequest)
    returns (DeleteVolumeResponse) {}
```

流程：
```
1. 验证卷存在
2. 检查是否正在使用
3. 标记删除（软删除）
4. 异步清理数据
   - 删除所有 inode
   - 删除所有数据块
   - 清理审计日志
5. 返回成功
```

#### CreateSnapshot

```protobuf
rpc CreateSnapshot(CreateSnapshotRequest)
    returns (CreateSnapshotResponse) {}
```

流程：
```
1. 创建快照记录
2. 复制卷的元数据树
   - 采用 COW（写时复制）
   - 只复制元数据，不复制数据
3. 返回快照 ID
```

#### ControllerExpandVolume

```protobuf
rpc ControllerExpandVolume(ControllerExpandVolumeRequest)
    returns (ControllerExpandVolumeResponse) {}
```

流程：
```
1. 验证新容量 > 当前容量
2. 更新配额限制
3. 返回新容量
4. 不需要文件系统扩展（动态分配）
```

### Node Service

#### NodeStageVolume

```protobuf
rpc NodeStageVolume(NodeStageVolumeRequest)
    returns (NodeStageVolumeResponse) {}
```

流程：
```
1. 准备暂存目录
   staging_target_path: /var/lib/kubelet/plugins/.../staging

2. 从 volume_id 获取租户信息
   - volume_id 即 tenant_id
   - 从 volume_context 获取 tenant_name
   - 验证租户存在且状态为 active

3. 挂载 Tarbox FUSE（指定租户）
   - 启动 FUSE 进程
   - 传递 --tenant-id 参数
   - 挂载到暂存目录
   - FUSE 进程绑定到该租户，只能访问该租户数据

4. 验证挂载成功
```

#### NodePublishVolume

```protobuf
rpc NodePublishVolume(NodePublishVolumeRequest)
    returns (NodePublishVolumeResponse) {}
```

流程：
```
1. 从暂存目录绑定挂载到目标
   target_path: /var/lib/kubelet/pods/.../volumes/...

2. 应用挂载选项
   - read_only: 只读挂载
   - volume_mount_group: 设置组 ID

3. 返回成功
```

#### NodeUnpublishVolume

```protobuf
rpc NodeUnpublishVolume(NodeUnpublishVolumeRequest)
    returns (NodeUnpublishVolumeResponse) {}
```

流程：
```
1. 卸载目标路径
2. 清理挂载点
3. 返回成功
```

#### NodeUnstageVolume

```protobuf
rpc NodeUnstageVolume(NodeUnstageVolumeRequest)
    returns (NodeUnstageVolumeResponse) {}
```

流程：
```
1. 卸载 FUSE 文件系统
2. 停止 FUSE 进程
3. 清理暂存目录
4. 返回成功
```

#### NodeGetVolumeStats

```protobuf
rpc NodeGetVolumeStats(NodeGetVolumeStatsRequest)
    returns (NodeGetVolumeStatsResponse) {}
```

流程：
```
1. 查询卷的使用统计
   - used_bytes: 已用空间
   - available_bytes: 可用空间
   - used_inodes: 已用 inode 数
   - available_inodes: 可用 inode 数

2. 返回统计信息
```

## StorageClass 设计

### 基础 StorageClass

```yaml
apiVersion: storage.k8s.io/v1
kind: StorageClass
metadata:
  name: tarbox
provisioner: tarbox.csi.io
parameters:
  databaseURL: "postgresql://tarbox:password@postgres:5432/tarbox"
  auditLevel: "standard"
  autoCheckpoint: "false"
reclaimPolicy: Delete
allowVolumeExpansion: true
volumeBindingMode: Immediate
```

### 高性能 StorageClass

```yaml
apiVersion: storage.k8s.io/v1
kind: StorageClass
metadata:
  name: tarbox-performance
provisioner: tarbox.csi.io
parameters:
  databaseURL: "postgresql://..."
  auditLevel: "basic"           # 降低审计级别
  cacheSize: "4Gi"              # 增大缓存
  blockSize: "8192"             # 更大的块
  autoCheckpoint: "false"       # 禁用自动检查点
reclaimPolicy: Delete
allowVolumeExpansion: true
```

### 审计优化 StorageClass

```yaml
apiVersion: storage.k8s.io/v1
kind: StorageClass
metadata:
  name: tarbox-audit
provisioner: tarbox.csi.io
parameters:
  databaseURL: "postgresql://..."
  auditLevel: "full"            # 完整审计
  auditRetentionDays: "365"     # 长期保留
  autoCheckpoint: "true"        # 启用自动检查点
  checkpointInterval: "1h"      # 每小时创建检查点
reclaimPolicy: Retain           # 保留数据
allowVolumeExpansion: true
```

### 原生挂载 StorageClass

```yaml
apiVersion: storage.k8s.io/v1
kind: StorageClass
metadata:
  name: tarbox-native
provisioner: tarbox.csi.io
parameters:
  databaseURL: "postgresql://..."
  # 原生挂载配置（TOML 格式）
  nativeMounts: |
    [[native_mounts]]
    path = "/bin"
    source = "/bin"
    mode = "ro"
    shared = true
    
    [[native_mounts]]
    path = "/usr"
    source = "/usr"
    mode = "ro"
    shared = true
    
    [[native_mounts]]
    path = "/.venv"
    source = "/var/tarbox/venvs/{tenant_id}"
    mode = "rw"
    shared = false
reclaimPolicy: Delete
allowVolumeExpansion: true
```

## PVC 和 Pod 使用

### 创建 PVC

```yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: agent-storage
  namespace: ai-agents
spec:
  accessModes:
    - ReadWriteMany          # 多 Pod 共享
  storageClassName: tarbox
  resources:
    requests:
      storage: 10Gi
```

### 在 Pod 中使用

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: ai-agent
  namespace: ai-agents
spec:
  containers:
  - name: agent
    image: ai-agent:latest
    volumeMounts:
    - name: data
      mountPath: /workspace
  volumes:
  - name: data
    persistentVolumeClaim:
      claimName: agent-storage
```

### StatefulSet 使用

```yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: agent-workers
spec:
  serviceName: agent
  replicas: 3
  selector:
    matchLabels:
      app: agent
  template:
    metadata:
      labels:
        app: agent
    spec:
      containers:
      - name: agent
        image: ai-agent:latest
        volumeMounts:
        - name: data
          mountPath: /data
  volumeClaimTemplates:
  - metadata:
      name: data
    spec:
      accessModes: ["ReadWriteOnce"]
      storageClassName: tarbox
      resources:
        requests:
          storage: 5Gi
```

## 快照功能

### VolumeSnapshotClass

```yaml
apiVersion: snapshot.storage.k8s.io/v1
kind: VolumeSnapshotClass
metadata:
  name: tarbox-snapshot
driver: tarbox.csi.io
deletionPolicy: Delete
parameters:
  compression: "zstd"
```

### 创建快照

```yaml
apiVersion: snapshot.storage.k8s.io/v1
kind: VolumeSnapshot
metadata:
  name: agent-storage-snapshot-20260114
  namespace: ai-agents
spec:
  volumeSnapshotClassName: tarbox-snapshot
  source:
    persistentVolumeClaimName: agent-storage
```

### 从快照恢复

```yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: agent-storage-restore
  namespace: ai-agents
spec:
  accessModes:
    - ReadWriteMany
  storageClassName: tarbox
  dataSource:
    name: agent-storage-snapshot-20260114
    kind: VolumeSnapshot
    apiGroup: snapshot.storage.k8s.io
  resources:
    requests:
      storage: 10Gi
```

## 卷克隆

```yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: agent-storage-clone
  namespace: ai-agents
spec:
  accessModes:
    - ReadWriteMany
  storageClassName: tarbox
  dataSource:
    name: agent-storage
    kind: PersistentVolumeClaim
  resources:
    requests:
      storage: 10Gi
```

## 多租户隔离

### 命名空间隔离

```
设计：
- 每个 PVC 对应一个独立的命名空间
- 命名空间是根 inode 的隔离
- 不同命名空间的数据完全隔离

命名规则：
- k8s-{namespace}-{pvc-name}
- 例如：k8s-ai-agents-agent-storage
```

### 配额管理

```
每个 PVC 配额：
- 最大容量：PVC 请求的 storage
- 最大 inode 数：根据容量计算
- 最大文件大小：可配置

超配额行为：
- 写入返回 ENOSPC（空间不足）
- 审计记录配额超限事件
```

### RBAC 集成

```
Kubernetes RBAC：
- 只有命名空间内的 Pod 可以访问 PVC
- 通过 ServiceAccount 进行身份验证
- 支持细粒度权限控制

Tarbox 内部：
- 映射 K8s 身份到文件系统权限
- 支持 Pod 级别的访问控制
```

## 高可用设计

### Controller 高可用

```
部署：
- 多副本 Deployment（3 副本）
- Leader Election（基于 Kubernetes）
- 只有 Leader 处理请求

故障转移：
- Leader 失败时自动选举新 Leader
- 切换时间 < 30 秒
- 无状态设计，快速恢复
```

### Node 高可用

```
部署：
- DaemonSet（每节点一个）
- 节点故障时 Pod 自动调度到其他节点
- 新节点自动重新挂载卷

数据可用性：
- 数据存储在 PostgreSQL
- PostgreSQL 高可用（主从复制）
- 节点故障不影响数据
```

## 监控和告警

### Prometheus 指标

```
# CSI 操作指标
tarbox_csi_operations_total{method="CreateVolume|DeleteVolume|..."}
tarbox_csi_operation_duration_seconds{method="..."}
tarbox_csi_operation_errors_total{method="..."}

# 卷统计
tarbox_volume_count{namespace="..."}
tarbox_volume_capacity_bytes{volume_id="..."}
tarbox_volume_used_bytes{volume_id="..."}

# 挂载统计
tarbox_mount_count{node="..."}
tarbox_mount_duration_seconds

# 快照统计
tarbox_snapshot_count
tarbox_snapshot_size_bytes{snapshot_id="..."}
```

### 日志

```
结构化日志：
- JSON 格式
- 包含请求 ID、卷 ID 等上下文
- 不同级别（INFO/WARN/ERROR）

日志内容：
- CSI 操作记录
- 挂载/卸载事件
- 错误和异常
- 性能统计
```

### 告警规则

```yaml
groups:
- name: tarbox-csi
  rules:
  - alert: TarboxCSIHighErrorRate
    expr: rate(tarbox_csi_operation_errors_total[5m]) > 0.1
    for: 5m
    annotations:
      summary: "Tarbox CSI error rate is high"
  
  - alert: TarboxVolumeAlmostFull
    expr: tarbox_volume_used_bytes / tarbox_volume_capacity_bytes > 0.9
    for: 10m
    annotations:
      summary: "Tarbox volume {{ $labels.volume_id }} is almost full"
  
  - alert: TarboxCSIControllerDown
    expr: up{job="tarbox-csi-controller"} == 0
    for: 1m
    annotations:
      summary: "Tarbox CSI controller is down"
```

## 部署

### Helm Chart 结构

```
tarbox-csi/
├── Chart.yaml
├── values.yaml
├── templates/
│   ├── controller/
│   │   ├── deployment.yaml
│   │   ├── service.yaml
│   │   └── rbac.yaml
│   ├── node/
│   │   ├── daemonset.yaml
│   │   └── rbac.yaml
│   ├── storageclass.yaml
│   ├── csidriver.yaml
│   └── servicemonitor.yaml
└── crds/
    └── ...
```

### 安装命令

```bash
# 添加 Helm 仓库
helm repo add tarbox https://tarbox.io/charts
helm repo update

# 安装
helm install tarbox-csi tarbox/tarbox-csi \
  --namespace tarbox-system \
  --create-namespace \
  --set database.url="postgresql://..." \
  --set controller.replicas=3
```

### values.yaml 示例

```yaml
image:
  repository: tarbox/csi-driver
  tag: v0.1.0
  pullPolicy: IfNotPresent

controller:
  replicas: 3
  resources:
    requests:
      cpu: 100m
      memory: 128Mi
    limits:
      cpu: 1000m
      memory: 1Gi

node:
  resources:
    requests:
      cpu: 100m
      memory: 256Mi
    limits:
      cpu: 1000m
      memory: 2Gi
  
  # 挂载主机路径（FUSE 需要）
  hostPath:
    enabled: true
    path: /var/lib/tarbox

database:
  url: "postgresql://tarbox:password@postgres:5432/tarbox"
  # 或使用 Secret
  urlSecret:
    name: tarbox-db-secret
    key: url

storageClass:
  create: true
  name: tarbox
  reclaimPolicy: Delete
  allowVolumeExpansion: true

monitoring:
  enabled: true
  serviceMonitor:
    enabled: true
    interval: 30s
```

## 测试策略

### 单元测试

```
- CSI 接口实现测试
- 参数解析测试
- 错误处理测试
```

### 集成测试

```
- 在真实 K8s 集群测试
- 测试 PVC 创建、挂载、使用、删除全流程
- 测试快照和克隆
- 测试卷扩展
```

### E2E 测试

```
- 使用 kubernetes-csi/csi-test 工具
- 测试 CSI 规范合规性
- 测试并发场景
- 测试故障恢复
```

## 未来增强

### 卷迁移

```
- 支持在线迁移卷到不同数据库
- 支持跨集群卷迁移
```

### 卷加密

```
- 支持静态加密
- 与 KMS 集成
```

### 性能优化

```
- 本地缓存（节点级别）
- 智能预取
- 读写分离（只读副本）
```

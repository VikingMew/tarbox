# Task 13: Kubernetes CSI 驱动

## 状态

**📅 计划中**

## 目标

实现完整的 Kubernetes CSI (Container Storage Interface) 驱动，使 Tarbox 可以作为 Kubernetes 持久卷使用。基于 spec/14 的 FilesystemInterface 抽象层，通过适配器模式实现 CSI 协议，复用 TarboxBackend 的核心逻辑。

**核心特性**：
- **动态供应**: 自动创建和删除 PV
- **多租户隔离**: PVC → 租户自动映射
- **ReadWriteMany**: 多 Pod 共享卷
- **快照和克隆**: 基于 Layer 机制
- **在线扩容**: 动态调整配额
- **原生挂载**: 支持主机目录挂载（系统目录、venv等）
- **高可用**: Controller 多副本 + Leader Election

## 优先级

**P2 - 云原生集成**

## 依赖

- Task 05: FUSE 接口 ✅ (FilesystemInterface 抽象层)
- Task 08: 分层文件系统 ✅ (快照支持)
- Task 06: 数据库层高级 ✅ (原生挂载表)

## 依赖的Spec

- **spec/05-kubernetes-csi.md** - CSI 驱动设计（核心）
- **spec/14-filesystem-interface.md** - 文件系统抽象层（核心）
- spec/04-layered-filesystem.md - 快照和克隆支持
- spec/12-native-mounting.md - 原生挂载配置
- spec/09-multi-tenancy.md - 租户隔离

## 实现内容

### 1. CSI 核心服务

- [ ] **Identity Service** (`src/csi/identity.rs`)
  - GetPluginInfo - 返回插件名称和版本
  - GetPluginCapabilities - 声明支持的能力
  - Probe - 健康检查
  ```rust
  pub struct IdentityService {
      name: String,        // "tarbox.csi.io"
      version: String,     // "0.1.0"
  }
  ```

- [ ] **Controller Service** (`src/csi/controller.rs`)
  - CreateVolume - 创建卷（自动创建租户）
  - DeleteVolume - 删除卷（清理租户）
  - ControllerPublishVolume - 发布卷到节点
  - ControllerUnpublishVolume - 从节点取消发布
  - ValidateVolumeCapabilities - 验证卷能力
  - ListVolumes - 列出所有卷
  - GetCapacity - 获取容量信息
  - ControllerGetCapabilities - 声明控制器能力
  - CreateSnapshot - 创建快照（基于 Layer）
  - DeleteSnapshot - 删除快照
  - ListSnapshots - 列出快照
  - ControllerExpandVolume - 在线扩容

- [ ] **Node Service** (`src/csi/node.rs`)
  - NodeStageVolume - 暂存卷（FUSE 挂载）
  - NodeUnstageVolume - 取消暂存
  - NodePublishVolume - 发布卷到 Pod
  - NodeUnpublishVolume - 从 Pod 取消发布
  - NodeGetVolumeStats - 获取卷统计
  - NodeExpandVolume - 节点侧扩容（可选）
  - NodeGetCapabilities - 声明节点能力
  - NodeGetInfo - 获取节点信息

### 2. 多租户集成

- [ ] **PVC 到租户映射** (`src/csi/tenant_mapping.rs`)
  - 租户命名规则: `{namespace}-{pvc-name}`
  - 自动创建租户记录
  - 租户 ID 作为 volume_id
  - 配额管理（从 PVC capacity）
  - 租户生命周期管理
  ```rust
  pub struct TenantMapper {
      tenant_ops: Arc<TenantOperations>,
      layer_ops: Arc<LayerOperations>,
  }
  
  impl TenantMapper {
      async fn create_tenant_from_pvc(
          &self,
          namespace: &str,
          pvc_name: &str,
          capacity_bytes: i64,
      ) -> Result<Tenant>;
      
      async fn delete_tenant_for_volume(
          &self,
          volume_id: &str,
      ) -> Result<()>;
  }
  ```

- [ ] **跨命名空间隔离**
  - Kubernetes RBAC 集成
  - 租户级别数据隔离
  - ServiceAccount 映射

### 3. 快照和克隆

- [ ] **快照管理** (`src/csi/snapshot.rs`)
  - 基于 Layer 机制实现快照
  - CreateSnapshot → 创建新 Layer
  - DeleteSnapshot → 删除 Layer
  - ListSnapshots → 列出租户的 Layer 历史
  - 快照元数据管理
  ```rust
  pub struct SnapshotManager {
      layer_ops: Arc<LayerOperations>,
  }
  
  impl SnapshotManager {
      async fn create_snapshot(
          &self,
          tenant_id: Uuid,
          snapshot_name: &str,
      ) -> Result<Snapshot>;
      
      async fn restore_from_snapshot(
          &self,
          tenant_id: Uuid,
          snapshot_id: Uuid,
      ) -> Result<()>;
  }
  ```

- [ ] **卷克隆**
  - 从现有卷克隆（复制 Layer 链）
  - COW 优化（共享数据块）
  - 独立租户空间

### 4. 原生挂载支持

- [ ] **原生挂载配置** (`src/csi/native_mounts.rs`)
  - 从 StorageClass parameters 解析原生挂载配置（TOML 格式）
  - 创建卷时写入 native_mounts 表
  - 支持共享挂载（所有租户）和专属挂载（单个租户）
  - 路径模板变量替换（{tenant_id}, {namespace}, {pvc_name}）
  ```rust
  pub struct NativeMountConfig {
      pub mounts: Vec<NativeMountEntry>,
  }
  
  pub struct NativeMountEntry {
      pub path: String,           // 虚拟路径 e.g., "/bin"
      pub source: String,         // 主机路径 e.g., "/bin" 或 "/var/tarbox/venvs/{tenant_id}"
      pub mode: String,           // "ro" or "rw"
      pub shared: bool,           // true = 所有租户共享, false = 租户专属
      pub priority: i32,
  }
  
  impl NativeMountConfig {
      fn from_toml(toml_str: &str) -> Result<Self>;
      
      async fn apply_to_tenant(
          &self,
          tenant_id: Uuid,
          native_mount_ops: &NativeMountOperations,
      ) -> Result<()>;
  }
  ```

- [ ] **示例配置**
  ```toml
  # 系统目录共享挂载
  [[native_mounts]]
  path = "/bin"
  source = "/bin"
  mode = "ro"
  shared = true
  priority = 100
  
  [[native_mounts]]
  path = "/usr"
  source = "/usr"
  mode = "ro"
  shared = true
  priority = 100
  
  # 租户专属 venv
  [[native_mounts]]
  path = "/.venv"
  source = "/var/tarbox/venvs/{tenant_id}"
  mode = "rw"
  shared = false
  priority = 200
  
  # 共享数据集
  [[native_mounts]]
  path = "/data/models"
  source = "/mnt/shared/models"
  mode = "ro"
  shared = true
  priority = 150
  ```

### 5. FUSE 挂载管理

- [ ] **挂载管理器** (`src/csi/mount_manager.rs`)
  - NodeStageVolume 时启动 FUSE 进程
  - 进程生命周期管理
  - 挂载点清理
  - 错误恢复
  ```rust
  pub struct MountManager {
      fs: Arc<FileSystem>,
      active_mounts: Arc<Mutex<HashMap<String, MountHandle>>>,
  }
  
  struct MountHandle {
      tenant_id: Uuid,
      mount_path: PathBuf,
      fuse_process: Option<Child>,
  }
  ```

- [ ] **挂载选项**
  - Read-only 模式
  - User/Group ID 映射
  - 权限控制

### 6. gRPC 服务器

- [ ] **CSI gRPC 服务** (`src/csi/server.rs`)
  - 基于 tonic 实现
  - Unix domain socket 通信
  - 请求日志和追踪
  - 错误处理和重试
  ```rust
  pub struct CsiServer {
      identity: IdentityService,
      controller: Option<ControllerService>,
      node: Option<NodeService>,
  }
  
  impl CsiServer {
      pub async fn serve_controller(addr: String) -> Result<()>;
      pub async fn serve_node(addr: String) -> Result<()>;
  }
  ```

- [ ] **gRPC 端点**
  - Controller: unix:///csi/controller.sock
  - Node: unix:///csi/node.sock

### 7. Kubernetes 资源

- [ ] **CSIDriver** (`deploy/csi/csidriver.yaml`)
  ```yaml
  apiVersion: storage.k8s.io/v1
  kind: CSIDriver
  metadata:
    name: tarbox.csi.io
  spec:
    attachRequired: false
    podInfoOnMount: true
    volumeLifecycleModes:
      - Persistent
      - Ephemeral
  ```

- [ ] **StorageClass** (`deploy/csi/storageclass.yaml`)
  ```yaml
  apiVersion: storage.k8s.io/v1
  kind: StorageClass
  metadata:
    name: tarbox
  provisioner: tarbox.csi.io
  parameters:
    databaseURL: "postgresql://..."
    auditLevel: "standard"
    autoCheckpoint: "false"
    # 原生挂载配置
    nativeMounts: |
      [[native_mounts]]
      path = "/bin"
      source = "/bin"
      mode = "ro"
      shared = true
  reclaimPolicy: Delete
  allowVolumeExpansion: true
  volumeBindingMode: Immediate
  ```

- [ ] **VolumeSnapshotClass** (`deploy/csi/snapshotclass.yaml`)
  ```yaml
  apiVersion: snapshot.storage.k8s.io/v1
  kind: VolumeSnapshotClass
  metadata:
    name: tarbox-snapshot
  driver: tarbox.csi.io
  deletionPolicy: Delete
  ```

### 8. Controller 部署

- [ ] **Controller Deployment** (`deploy/csi/controller-deployment.yaml`)
  - 多副本部署（3 副本）
  - Leader Election
  - RBAC 配置
  - 资源限制
  ```yaml
  apiVersion: apps/v1
  kind: Deployment
  metadata:
    name: tarbox-csi-controller
  spec:
    replicas: 3
    selector:
      matchLabels:
        app: tarbox-csi-controller
    template:
      spec:
        serviceAccountName: tarbox-csi-controller
        containers:
        - name: csi-provisioner
          image: k8s.gcr.io/sig-storage/csi-provisioner:v3.4.0
        - name: csi-attacher
          image: k8s.gcr.io/sig-storage/csi-attacher:v4.2.0
        - name: csi-snapshotter
          image: k8s.gcr.io/sig-storage/csi-snapshotter:v6.2.0
        - name: csi-resizer
          image: k8s.gcr.io/sig-storage/csi-resizer:v1.7.0
        - name: tarbox-controller
          image: tarbox/csi-driver:v0.1.0
          args:
            - --mode=controller
            - --endpoint=unix:///csi/csi.sock
  ```

- [ ] **Leader Election 配置**
  - 基于 Kubernetes Lease
  - 故障转移 < 30 秒

### 9. Node 部署

- [ ] **Node DaemonSet** (`deploy/csi/node-daemonset.yaml`)
  - 每节点一个 Pod
  - 特权模式（FUSE 需要）
  - 主机路径挂载
  ```yaml
  apiVersion: apps/v1
  kind: DaemonSet
  metadata:
    name: tarbox-csi-node
  spec:
    selector:
      matchLabels:
        app: tarbox-csi-node
    template:
      spec:
        serviceAccountName: tarbox-csi-node
        hostNetwork: true
        containers:
        - name: node-driver-registrar
          image: k8s.gcr.io/sig-storage/csi-node-driver-registrar:v2.7.0
        - name: tarbox-node
          image: tarbox/csi-driver:v0.1.0
          args:
            - --mode=node
            - --endpoint=unix:///csi/csi.sock
          securityContext:
            privileged: true
          volumeMounts:
            - name: plugin-dir
              mountPath: /csi
            - name: pods-mount-dir
              mountPath: /var/lib/kubelet/pods
              mountPropagation: Bidirectional
            - name: fuse-device
              mountPath: /dev/fuse
  ```

### 10. Helm Chart

- [ ] **Chart 结构** (`charts/tarbox-csi/`)
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
  │   ├── csidriver.yaml
  │   ├── storageclass.yaml
  │   ├── snapshotclass.yaml
  │   └── servicemonitor.yaml (可选)
  ```

- [ ] **values.yaml 配置**
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
  
  database:
    url: "postgresql://..."
  
  storageClass:
    create: true
    name: tarbox
    reclaimPolicy: Delete
    allowVolumeExpansion: true
    parameters:
      auditLevel: "standard"
      nativeMounts: |
        [[native_mounts]]
        path = "/bin"
        source = "/bin"
        mode = "ro"
        shared = true
  
  monitoring:
    enabled: true
  ```

### 11. 监控和可观测性

- [ ] **Prometheus 指标** (`src/csi/metrics.rs`)
  ```rust
  // CSI 操作指标
  tarbox_csi_operations_total{method="CreateVolume|DeleteVolume|..."}
  tarbox_csi_operation_duration_seconds{method="..."}
  tarbox_csi_operation_errors_total{method="..."}
  
  // 卷统计
  tarbox_volume_count{namespace="..."}
  tarbox_volume_capacity_bytes{volume_id="..."}
  tarbox_volume_used_bytes{volume_id="..."}
  
  // 挂载统计
  tarbox_mount_count{node="..."}
  tarbox_mount_duration_seconds
  
  // 快照统计
  tarbox_snapshot_count
  tarbox_snapshot_size_bytes{snapshot_id="..."}
  ```

- [ ] **结构化日志**
  - JSON 格式
  - 请求 ID 追踪
  - 卷 ID、租户 ID 上下文

- [ ] **告警规则** (`deploy/monitoring/alerts.yaml`)
  - CSI 操作失败率
  - 卷配额告警
  - Controller 不可用告警

### 12. 测试

- [ ] **单元测试**
  - CSI 接口实现测试
  - 租户映射逻辑测试
  - 快照管理测试
  - 原生挂载配置解析测试

- [ ] **集成测试** (`tests/csi_integration_test.rs`)
  - test_create_delete_volume - 卷生命周期
  - test_pvc_tenant_mapping - PVC 到租户映射
  - test_volume_snapshot - 快照创建和恢复
  - test_volume_clone - 卷克隆
  - test_volume_expansion - 在线扩容
  - test_native_mounts_config - 原生挂载配置
  - test_shared_vs_exclusive_mounts - 共享和专属挂载
  - test_mount_unmount - 挂载和卸载
  - test_multi_pod_access - ReadWriteMany 测试

- [ ] **E2E 测试** (`tests/csi_e2e_test.sh`)
  - 使用 csi-sanity 工具
  - 在真实 K8s 集群测试
  - 测试故障恢复

- [ ] **性能测试**
  - 大量卷创建性能
  - 并发挂载性能
  - 快照性能

## 架构要点

### CSI 适配器模式

```rust
// CSI Adapter 实现 FilesystemInterface
pub struct CsiAdapter {
    backend: Arc<TarboxBackend>,
    tenant_mapper: Arc<TenantMapper>,
    mount_manager: Arc<MountManager>,
}

// 复用 90% 代码
impl FilesystemInterface for CsiAdapter {
    async fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        self.backend.read_file(path).await
    }
    // ... 其他方法直接转发到 backend
}
```

### 租户自动创建流程

```
CreateVolume Request
  ↓
解析 namespace + pvc_name
  ↓
生成 tenant_name = "{namespace}-{pvc-name}"
  ↓
检查租户是否存在
  ↓
不存在 → 创建租户记录
         → 创建根 inode
         → 创建 base layer
         → 设置配额
         → 解析并应用原生挂载配置
  ↓
返回 volume_id (tenant_id)
```

### 原生挂载配置流程

```
StorageClass.parameters.nativeMounts (TOML)
  ↓
CreateVolume 时解析
  ↓
对每个 [[native_mounts]] 条目:
  ├─ shared = true  → 创建全局挂载（tenant_id=NULL）
  └─ shared = false → 创建租户专属挂载（tenant_id=当前租户）
  ↓
路径模板替换 {tenant_id} 等变量
  ↓
写入 native_mounts 表
  ↓
NodeStageVolume 时 FUSE 自动加载配置
```

## 验收标准

### 核心功能
- [ ] CSI Identity/Controller/Node 服务完整实现
- [ ] PVC 自动创建租户
- [ ] 卷的创建、删除、挂载、卸载正常工作
- [ ] ReadWriteMany 模式正常工作
- [ ] 快照创建和恢复正常工作
- [ ] 卷克隆正常工作
- [ ] 在线扩容正常工作
- [ ] 原生挂载配置正常工作（共享和专属）
- [ ] 路径模板变量替换正常工作

### 质量标准
- [ ] 单元测试覆盖率 >55%
- [ ] 集成测试覆盖率 >25%
- [ ] 总覆盖率 >80%
- [ ] csi-sanity 测试通过
- [ ] cargo fmt 通过
- [ ] cargo clippy 无警告

### 部署标准
- [ ] Helm Chart 可以成功安装
- [ ] Controller 高可用（3 副本 + Leader Election）
- [ ] Node DaemonSet 正常运行
- [ ] StorageClass 可以创建 PVC
- [ ] Pod 可以正常使用 PVC

### 性能标准
- [ ] CreateVolume < 5s
- [ ] NodeStageVolume < 10s
- [ ] 支持 100+ 并发卷
- [ ] Leader 故障转移 < 30s

## 文件清单

### 新增文件
```
src/csi/
├── mod.rs                  - 模块导出
├── identity.rs             - Identity Service
├── controller.rs           - Controller Service
├── node.rs                 - Node Service
├── server.rs               - gRPC 服务器
├── tenant_mapping.rs       - PVC → 租户映射
├── snapshot.rs             - 快照管理
├── mount_manager.rs        - FUSE 挂载管理
├── native_mounts.rs        - 原生挂载配置
└── metrics.rs              - Prometheus 指标

deploy/csi/
├── csidriver.yaml
├── storageclass.yaml
├── snapshotclass.yaml
├── controller-deployment.yaml
├── controller-rbac.yaml
├── node-daemonset.yaml
└── node-rbac.yaml

charts/tarbox-csi/
├── Chart.yaml
├── values.yaml
└── templates/...

tests/
├── csi_integration_test.rs
└── csi_e2e_test.sh

deploy/monitoring/
└── alerts.yaml
```

### 修改文件
- Cargo.toml - 添加 tonic, prost 依赖
- src/main.rs - 添加 CSI 模式启动

## 技术栈

- **tonic** - gRPC 框架
- **prost** - Protocol Buffers
- **tokio** - 异步运行时
- **k8s-openapi** - Kubernetes API 类型
- **kube** - Kubernetes 客户端（用于 Leader Election）
- **prometheus-client** - 监控指标

## 后续任务

完成后可以开始：
- Task 14: REST API（与 CSI 共享部分代码）
- 生产环境验证和优化

## 参考资料

- [CSI Specification](https://github.com/container-storage-interface/spec)
- [Kubernetes CSI Developer Guide](https://kubernetes-csi.github.io/docs/)
- [csi-sanity](https://github.com/kubernetes-csi/csi-test/tree/master/cmd/csi-sanity)
- spec/05-kubernetes-csi.md - 完整设计文档
- spec/14-filesystem-interface.md - 抽象层设计

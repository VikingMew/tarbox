# API 设计规范

## 概述

Tarbox 提供多层次的 API 接口，包括内部 Rust API、HTTP REST API、gRPC API 和 CLI 工具。

## API 分层

```
┌─────────────────────────────────────┐
│     用户接口层                       │
│  ├─ CLI (tarbox)                   │
│  ├─ Web Dashboard                  │
│  └─ K8s CRD                        │
└──────────────┬──────────────────────┘
               │
┌──────────────▼──────────────────────┐
│     外部 API 层                      │
│  ├─ REST API (HTTP)                │
│  ├─ gRPC API                       │
│  └─ CSI gRPC                       │
└──────────────┬──────────────────────┘
               │
┌──────────────▼──────────────────────┐
│     内部 API 层                      │
│  ├─ FileSystem API                 │
│  ├─ Layer API                      │
│  ├─ Audit API                      │
│  └─ Storage API                    │
└─────────────────────────────────────┘
```

## 内部 Rust API

### FileSystem API

```rust
pub trait FileSystem {
    // 基础操作
    async fn create(&self, path: &Path, mode: Mode) -> Result<Inode>;
    async fn open(&self, path: &Path, flags: OpenFlags) -> Result<FileHandle>;
    async fn read(&self, fh: FileHandle, offset: u64, size: usize) -> Result<Vec<u8>>;
    async fn write(&self, fh: FileHandle, offset: u64, data: &[u8]) -> Result<usize>;
    async fn close(&self, fh: FileHandle) -> Result<()>;
    async fn unlink(&self, path: &Path) -> Result<()>;
    
    // 目录操作
    async fn mkdir(&self, path: &Path, mode: Mode) -> Result<()>;
    async fn rmdir(&self, path: &Path) -> Result<()>;
    async fn readdir(&self, path: &Path) -> Result<Vec<DirEntry>>;
    
    // 元数据操作
    async fn stat(&self, path: &Path) -> Result<FileStat>;
    async fn chmod(&self, path: &Path, mode: Mode) -> Result<()>;
    async fn chown(&self, path: &Path, uid: u32, gid: u32) -> Result<()>;
    async fn utime(&self, path: &Path, atime: Time, mtime: Time) -> Result<()>;
    
    // 链接操作
    async fn symlink(&self, target: &Path, link: &Path) -> Result<()>;
    async fn readlink(&self, path: &Path) -> Result<PathBuf>;
    async fn hardlink(&self, target: &Path, link: &Path) -> Result<()>;
    
    // 扩展属性
    async fn setxattr(&self, path: &Path, name: &str, value: &[u8]) -> Result<()>;
    async fn getxattr(&self, path: &Path, name: &str) -> Result<Vec<u8>>;
    async fn listxattr(&self, path: &Path) -> Result<Vec<String>>;
    async fn removexattr(&self, path: &Path, name: &str) -> Result<()>;
}
```

### Tenant API

```rust
pub trait TenantManager {
    // Tenant 管理
    async fn create_tenant(&self, input: CreateTenantInput) -> Result<Tenant>;
    async fn get_tenant(&self, tenant_id: Uuid) -> Result<Option<Tenant>>;
    async fn list_tenants(&self, filter: TenantFilter) -> Result<Vec<Tenant>>;
    async fn update_tenant(&self, tenant_id: Uuid, input: UpdateTenantInput) -> Result<Tenant>;
    async fn delete_tenant(&self, tenant_id: Uuid, force: bool) -> Result<()>;
    
    // Tenant 状态管理
    async fn suspend_tenant(&self, tenant_id: Uuid, reason: &str) -> Result<()>;
    async fn resume_tenant(&self, tenant_id: Uuid) -> Result<()>;
    async fn get_tenant_status(&self, tenant_id: Uuid) -> Result<TenantStatus>;
    
    // Tenant 统计
    async fn get_tenant_stats(&self, tenant_id: Uuid) -> Result<TenantStats>;
    async fn get_storage_usage(&self, tenant_id: Uuid) -> Result<StorageUsage>;
    async fn get_operation_stats(&self, tenant_id: Uuid, time_range: Duration) -> Result<OperationStats>;
    
    // 数据管理
    async fn cleanup_tenant_data(&self, tenant_id: Uuid, options: CleanupOptions) -> Result<CleanupResult>;
    async fn export_tenant_data(&self, tenant_id: Uuid, options: ExportOptions) -> Result<ExportJob>;
    async fn import_tenant_data(&self, tenant_id: Uuid, options: ImportOptions) -> Result<ImportJob>;
}

// 数据结构
pub struct CreateTenantInput {
    pub tenant_name: String,
    pub display_name: String,
    pub storage_quota_bytes: i64,
    pub metadata: Option<serde_json::Value>,
}

pub struct UpdateTenantInput {
    pub display_name: Option<String>,
    pub storage_quota_bytes: Option<i64>,
    pub metadata: Option<serde_json::Value>,
}

pub struct TenantFilter {
    pub status: Option<TenantStatus>,
    pub limit: i32,
    pub offset: i32,
}

pub struct TenantStats {
    pub storage: StorageUsage,
    pub layers: LayerStats,
    pub operations: OperationStats,
    pub last_access_at: Option<DateTime<Utc>>,
}

pub struct StorageUsage {
    pub used_bytes: i64,
    pub quota_bytes: i64,
    pub usage_percentage: f64,
    pub file_count: i64,
    pub directory_count: i64,
}

pub struct CleanupOptions {
    pub remove_deleted_files: bool,
    pub vacuum_blocks: bool,
    pub compress_layers: bool,
}
```

### Composition API

```rust
/// 文件系统组合管理（参见 Spec 18）
#[async_trait]
pub trait CompositionManager {
    /// 获取 Tenant 的组合配置
    async fn get_composition(&self, tenant_id: Uuid) -> Result<TenantComposition>;
    
    /// 添加挂载
    async fn add_mount(&self, tenant_id: Uuid, entry: CreateMountEntry) -> Result<MountEntry>;
    
    /// 更新挂载
    async fn update_mount(&self, mount_entry_id: Uuid, update: UpdateMountEntry) -> Result<MountEntry>;
    
    /// 删除挂载
    async fn remove_mount(&self, mount_entry_id: Uuid) -> Result<()>;
    
    /// 启用/禁用挂载
    async fn set_mount_enabled(&self, mount_entry_id: Uuid, enabled: bool) -> Result<()>;
    
    /// 解析路径（返回实际数据源）
    async fn resolve_path(&self, tenant_id: Uuid, path: &Path) -> Result<ResolvedPath>;
}

/// Layer 发布管理（参见 Spec 18）
#[async_trait]
pub trait LayerPublisher {
    /// 发布 Layer（允许其他 tenant 只读访问）
    async fn publish_layer(&self, input: PublishLayerInput) -> Result<()>;
    
    /// 取消发布
    async fn unpublish_layer(&self, layer_id: Uuid) -> Result<()>;
    
    /// 获取已发布的 Layer 列表
    async fn list_published_layers(&self, filter: PublishedLayerFilter) -> Result<Vec<PublishedLayer>>;
    
    /// 检查访问权限
    async fn check_layer_access(&self, layer_id: Uuid, accessor_tenant_id: Uuid) -> Result<bool>;
}

// 挂载源类型
pub enum MountSource {
    /// 宿主机目录
    Host { path: PathBuf },
    
    /// Tarbox Layer（可以是其他租户已发布的层）
    Layer {
        tenant_id: Uuid,
        layer_id: Uuid,
        subpath: Option<PathBuf>,
    },
    
    /// 当前 Tenant 的工作层（owner 可写）
    WorkingLayer,
}

// 挂载模式
pub enum MountMode {
    ReadOnly,      // 只读
    ReadWrite,     // 读写（仅限 Host 或 WorkingLayer）
    CopyOnWrite,   // 写时复制（读取来自源，写入到 WorkingLayer）
}

// 挂载条目
pub struct MountEntry {
    pub mount_entry_id: Uuid,
    pub tenant_id: Uuid,
    pub virtual_path: PathBuf,
    pub source: MountSource,
    pub mode: MountMode,
    pub is_file: bool,      // true=文件挂载, false=目录挂载
    pub enabled: bool,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// 约束：同一 Tenant 的挂载路径不可嵌套、不可冲突

// Tenant 组合配置
pub struct TenantComposition {
    pub tenant_id: Uuid,
    pub working_layer_id: Uuid,
    pub mounts: Vec<MountEntry>,
}

// 挂载点级别的 Layer 链
// 每个 WorkingLayer 类型的挂载点有独立的 layer 链
pub struct Layer {
    pub layer_id: Uuid,
    pub mount_entry_id: Uuid,        // 所属挂载点
    pub tenant_id: Uuid,
    pub parent_layer_id: Option<Uuid>,
    pub name: Option<String>,        // snapshot 名称
    pub is_working: bool,            // 是否是当前工作层
    pub created_at: DateTime<Utc>,
}

// 发布某个挂载点的 Layer
pub struct PublishInput {
    pub mount_entry_id: Uuid,        // 要发布的挂载点
    pub publish_name: String,        // 全局唯一的发布名称
    pub description: Option<String>,
    pub target: PublishTarget,
    pub scope: PublishScope,
}

pub enum PublishTarget {
    /// 发布特定的 snapshot（内容固定）
    Layer(Uuid),
    
    /// 发布当前 working layer（实时跟随）
    WorkingLayer,
}

pub struct PublishedMount {
    pub publish_id: Uuid,
    pub mount_entry_id: Uuid,        // 发布的挂载点
    pub tenant_id: Uuid,             // Owner
    pub target_type: String,         // "layer" 或 "working_layer"
    pub layer_id: Option<Uuid>,      // 如果是特定 layer
    pub publish_name: String,
    pub description: Option<String>,
    pub scope: PublishScope,
    pub created_at: DateTime<Utc>,
}

pub enum PublishScope {
    Public,                          // 所有租户可见
    AllowList(Vec<Uuid>),           // 仅指定租户可见
}
```

### Layer API

```rust
pub trait LayerManager {
    // 层管理
    async fn create_checkpoint(&self, name: &str, description: &str) -> Result<Layer>;
    async fn list_layers(&self) -> Result<Vec<Layer>>;
    async fn get_layer(&self, layer_id: Uuid) -> Result<Layer>;
    async fn delete_layer(&self, layer_id: Uuid) -> Result<()>;
    
    // 层操作
    async fn switch_layer(&self, layer_id: Uuid) -> Result<()>;
    async fn get_current_layer(&self) -> Result<Layer>;
    async fn get_layer_changes(&self, layer_id: Uuid) -> Result<LayerChanges>;
    
    // 高级操作
    async fn squash_layers(&self, layer_ids: &[Uuid], name: &str) -> Result<Layer>;
    async fn create_branch(&self, from_layer: Uuid, name: &str) -> Result<Layer>;
    async fn merge_layers(&self, source: Uuid, target: Uuid) -> Result<()>;
    
    // 查询
    async fn get_layer_tree(&self) -> Result<LayerTree>;
    async fn get_file_history(&self, path: &Path) -> Result<Vec<LayerEntry>>;
}
```

### Audit API

```rust
pub trait AuditManager {
    // 记录审计
    async fn record(&self, event: AuditEvent) -> Result<()>;
    async fn record_batch(&self, events: Vec<AuditEvent>) -> Result<()>;
    
    // 查询审计
    async fn query(&self, filter: AuditFilter) -> Result<Vec<AuditEvent>>;
    async fn aggregate(&self, query: AggregateQuery) -> Result<AggregateResult>;
    
    // 流式订阅
    async fn subscribe(&self, filter: AuditFilter) -> Result<AuditStream>;
    
    // 管理
    async fn set_audit_level(&self, level: AuditLevel) -> Result<()>;
    async fn get_audit_level(&self) -> Result<AuditLevel>;
    async fn cleanup_old_logs(&self, before: DateTime) -> Result<u64>;
}
```

### Storage API

```rust
pub trait StorageBackend {
    // Inode 操作
    async fn create_inode(&self, inode: &InodeData) -> Result<InodeId>;
    async fn get_inode(&self, inode_id: InodeId) -> Result<InodeData>;
    async fn update_inode(&self, inode_id: InodeId, data: &InodeData) -> Result<()>;
    async fn delete_inode(&self, inode_id: InodeId) -> Result<()>;
    
    // 数据块操作
    async fn write_block(&self, block: &BlockData) -> Result<BlockId>;
    async fn read_block(&self, block_id: BlockId) -> Result<BlockData>;
    async fn delete_block(&self, block_id: BlockId) -> Result<()>;
    
    // 批量操作
    async fn batch_write_blocks(&self, blocks: Vec<BlockData>) -> Result<Vec<BlockId>>;
    async fn batch_read_blocks(&self, block_ids: Vec<BlockId>) -> Result<Vec<BlockData>>;
    
    // 事务
    async fn begin_transaction(&self) -> Result<Transaction>;
    async fn commit_transaction(&self, tx: Transaction) -> Result<()>;
    async fn rollback_transaction(&self, tx: Transaction) -> Result<()>;
}
```

## REST API

### 基础信息

```
Base URL: http://tarbox-server:8080/api/v1
Authentication: Bearer Token
Content-Type: application/json
```

### 文件系统操作

```http
# 获取文件信息
GET /fs/stat?path=/data/file.txt
Response: {
    "inode_id": 12345,
    "type": "file",
    "size": 1048576,
    "mode": 420,  # 0644
    "uid": 1000,
    "gid": 1000,
    "atime": "2026-01-14T12:00:00Z",
    "mtime": "2026-01-14T12:00:00Z",
    "ctime": "2026-01-14T12:00:00Z"
}

# 列出目录
GET /fs/list?path=/data&recursive=false
Response: {
    "entries": [
        {
            "name": "file1.txt",
            "type": "file",
            "size": 1024
        },
        {
            "name": "subdir",
            "type": "directory",
            "size": 4096
        }
    ]
}

# 读取文件内容
GET /fs/read?path=/data/file.txt&offset=0&length=1024
Response: {
    "data": "base64-encoded-content",
    "bytes_read": 1024
}

# 写入文件
POST /fs/write
{
    "path": "/data/file.txt",
    "offset": 0,
    "data": "base64-encoded-content",
    "create_if_not_exists": true
}
Response: {
    "bytes_written": 1024
}

# 创建目录
POST /fs/mkdir
{
    "path": "/data/newdir",
    "mode": 493,  # 0755
    "recursive": true
}

# 删除文件
DELETE /fs/remove?path=/data/file.txt

# 重命名/移动
POST /fs/rename
{
    "from": "/data/old.txt",
    "to": "/data/new.txt"
}

# 复制文件
POST /fs/copy
{
    "from": "/data/source.txt",
    "to": "/data/dest.txt"
}
```

### Tenant 管理操作

```http
# 列出所有 Tenant
GET /tenants
Query Parameters:
  - limit: 限制返回数量 (默认100)
  - offset: 偏移量 (默认0)
  - status: 过滤状态 (active/suspended/deleted)
Response: {
    "tenants": [
        {
            "tenant_id": "uuid",
            "tenant_name": "tenant-001",
            "display_name": "AI Agent 001",
            "status": "active",
            "current_layer_id": "layer-uuid",
            "storage_quota_bytes": 107374182400,
            "storage_used_bytes": 10737418240,
            "file_count": 12345,
            "created_at": "2026-01-14T12:00:00Z",
            "updated_at": "2026-01-14T12:00:00Z",
            "metadata": {
                "owner": "user@example.com",
                "project": "ml-training"
            }
        }
    ],
    "total": 50,
    "limit": 100,
    "offset": 0
}

# 获取 Tenant 详情
GET /tenants/{tenant_id}
Response: {
    "tenant_id": "uuid",
    "tenant_name": "tenant-001",
    "display_name": "AI Agent 001",
    "status": "active",
    "current_layer_id": "layer-uuid",
    "storage_quota_bytes": 107374182400,
    "storage_used_bytes": 10737418240,
    "file_count": 12345,
    "layer_count": 5,
    "created_at": "2026-01-14T12:00:00Z",
    "updated_at": "2026-01-14T12:00:00Z",
    "last_access_at": "2026-01-15T10:30:00Z",
    "metadata": {
        "owner": "user@example.com",
        "project": "ml-training",
        "environment": "production"
    }
}

# 创建 Tenant
POST /tenants
{
    "tenant_name": "tenant-001",
    "display_name": "AI Agent 001",
    "storage_quota_bytes": 107374182400,
    "metadata": {
        "owner": "user@example.com",
        "project": "ml-training"
    }
}
Response: {
    "tenant_id": "new-uuid",
    "tenant_name": "tenant-001",
    "display_name": "AI Agent 001",
    "status": "active",
    "current_layer_id": "base-layer-uuid",
    "storage_quota_bytes": 107374182400,
    "storage_used_bytes": 0,
    "file_count": 0,
    "layer_count": 1,
    "created_at": "2026-01-15T10:00:00Z",
    "updated_at": "2026-01-15T10:00:00Z"
}

# 更新 Tenant
PUT /tenants/{tenant_id}
{
    "display_name": "AI Agent 001 (Updated)",
    "storage_quota_bytes": 214748364800,
    "metadata": {
        "owner": "user@example.com",
        "project": "ml-training",
        "environment": "production"
    }
}
Response: {
    "tenant_id": "uuid",
    "tenant_name": "tenant-001",
    "display_name": "AI Agent 001 (Updated)",
    "storage_quota_bytes": 214748364800,
    "updated_at": "2026-01-15T11:00:00Z"
}

# 暂停 Tenant
POST /tenants/{tenant_id}/suspend
{
    "reason": "Quota exceeded"
}
Response: {
    "tenant_id": "uuid",
    "status": "suspended",
    "suspended_at": "2026-01-15T11:30:00Z"
}

# 恢复 Tenant
POST /tenants/{tenant_id}/resume
Response: {
    "tenant_id": "uuid",
    "status": "active",
    "resumed_at": "2026-01-15T12:00:00Z"
}

# 删除 Tenant
DELETE /tenants/{tenant_id}?force=false
Query Parameters:
  - force: 强制删除（忽略数据保留策略）
Response: {
    "tenant_id": "uuid",
    "status": "deleted",
    "deleted_at": "2026-01-15T12:30:00Z",
    "data_retention_until": "2026-02-15T12:30:00Z"
}

# 获取 Tenant 统计信息
GET /tenants/{tenant_id}/stats
Response: {
    "tenant_id": "uuid",
    "storage": {
        "used_bytes": 10737418240,
        "quota_bytes": 107374182400,
        "usage_percentage": 10.0,
        "file_count": 12345,
        "directory_count": 567
    },
    "layers": {
        "total_count": 5,
        "active_layer_id": "layer-uuid",
        "base_layer_size": 1073741824,
        "all_layers_size": 5368709120
    },
    "operations": {
        "read_ops_24h": 100000,
        "write_ops_24h": 50000,
        "delete_ops_24h": 1000
    },
    "last_access_at": "2026-01-15T10:30:00Z"
}

# 获取 Tenant 的所有层
GET /tenants/{tenant_id}/layers
Response: {
    "tenant_id": "uuid",
    "layers": [
        {
            "layer_id": "uuid",
            "name": "checkpoint-001",
            "parent_layer_id": "parent-uuid",
            "is_readonly": true,
            "file_count": 1234,
            "total_size": 1048576,
            "created_at": "2026-01-14T12:00:00Z"
        }
    ]
}

# 清理 Tenant 数据（清理已删除文件、未使用块等）
POST /tenants/{tenant_id}/cleanup
{
    "remove_deleted_files": true,
    "vacuum_blocks": true,
    "compress_layers": false
}
Response: {
    "tenant_id": "uuid",
    "cleanup_started_at": "2026-01-15T13:00:00Z",
    "estimated_space_freed": 1073741824,
    "job_id": "cleanup-job-uuid"
}

# 导出 Tenant 数据
POST /tenants/{tenant_id}/export
{
    "format": "tar.gz",
    "include_history": true,
    "layer_id": "specific-layer-uuid"
}
Response: {
    "tenant_id": "uuid",
    "export_job_id": "export-job-uuid",
    "status": "pending",
    "created_at": "2026-01-15T14:00:00Z"
}

# 获取导出任务状态
GET /tenants/{tenant_id}/export/{export_job_id}
Response: {
    "export_job_id": "export-job-uuid",
    "status": "completed",
    "download_url": "https://tarbox-server/downloads/export-uuid.tar.gz",
    "size_bytes": 10737418240,
    "expires_at": "2026-01-16T14:00:00Z"
}

# 导入 Tenant 数据
POST /tenants/{tenant_id}/import
{
    "source_url": "https://storage.example.com/backup.tar.gz",
    "merge_strategy": "replace",
    "create_checkpoint": true
}
Response: {
    "tenant_id": "uuid",
    "import_job_id": "import-job-uuid",
    "status": "pending",
    "created_at": "2026-01-15T15:00:00Z"
}
```

### 文件系统组合操作（Composition）

> 参见 [Spec 18: 文件系统组合](./18-filesystem-composition.md)

```http
# 批量设置挂载配置（混合挂载）
# 这是配置混合挂载的主要方式，会替换该 Tenant 的所有挂载配置
PUT /tenants/{tenant_id}/mounts
Content-Type: application/json
{
    "mounts": [
        {
            "virtual_path": "/workspace",
            "is_file": false,
            "source_type": "working_layer",
            "mode": "rw"
        },
        {
            "virtual_path": "/models",
            "is_file": false,
            "source_type": "layer",
            "source_tenant_id": "model-hub-uuid",
            "source_layer_id": "bert-base-layer-uuid",
            "mode": "ro"
        },
        {
            "virtual_path": "/data",
            "is_file": false,
            "published_layer_name": "imagenet-v1",
            "source_subpath": "/train",
            "mode": "cow"
        },
        {
            "virtual_path": "/usr",
            "is_file": false,
            "source_type": "host",
            "host_path": "/usr",
            "mode": "ro"
        },
        {
            "virtual_path": "/bin",
            "is_file": false,
            "source_type": "host",
            "host_path": "/bin",
            "mode": "ro"
        },
        {
            "virtual_path": "/config.yaml",
            "is_file": true,
            "source_type": "host",
            "host_path": "/etc/app/config.yaml",
            "mode": "ro"
        }
    ]
}
Response: {
    "tenant_id": "uuid",
    "mounts_created": 6,
    "mounts": [...]
}

# 导入挂载配置（使用简化的 source 格式）
# name 用于 API 引用（snapshot/publish），path 是实际挂载路径
POST /tenants/{tenant_id}/mounts/import
Content-Type: application/json
{
    "mounts": [
        {
            "name": "workspace",
            "path": "/workspace",
            "source": "working_layer",
            "mode": "rw"
        },
        {
            "name": "models",
            "path": "/models",
            "source": "layer:model-hub:bert-base-v1",
            "mode": "ro"
        },
        {
            "name": "data",
            "path": "/data",
            "source": "published:imagenet-v1",
            "subpath": "/train",
            "mode": "cow"
        },
        {
            "name": "usr",
            "path": "/usr",
            "source": "host:/usr",
            "mode": "ro"
        },
        {
            "name": "bin",
            "path": "/bin",
            "source": "host:/bin",
            "mode": "ro"
        },
        {
            "name": "config",
            "path": "/config.yaml",
            "file": true,
            "source": "host:/etc/app/config.yaml",
            "mode": "ro"
        }
    ]
}
Response: {
    "tenant_id": "uuid",
    "mounts_created": 6,
    "mounts": [...]
}

# 导出挂载配置（JSON 格式，可转换为 TOML 供 CLI 使用）
GET /tenants/{tenant_id}/mounts/export
Response: {
    "tenant_id": "uuid",
    "tenant_name": "my-tenant",
    "exported_at": "2026-01-29T12:00:00Z",
    "mounts": [
        {
            "name": "workspace",
            "path": "/workspace",
            "source": "working_layer",
            "mode": "rw"
        },
        {
            "name": "models",
            "path": "/models",
            "source": "layer:model-hub:bert-base-v1",
            "mode": "ro"
        },
        {
            "name": "data",
            "path": "/data",
            "source": "published:imagenet-v1",
            "subpath": "/train",
            "mode": "cow"
        },
        {
            "name": "usr",
            "path": "/usr",
            "source": "host:/usr",
            "mode": "ro"
        },
        {
            "name": "config",
            "path": "/config.yaml",
            "file": true,
            "source": "host:/etc/app/config.yaml",
            "mode": "ro"
        }
    ]
}

# 获取 Tenant 的挂载配置
# 注意：挂载路径不可嵌套、不可冲突
# name 用于 API 引用（snapshot/publish），virtual_path 是实际挂载路径
GET /tenants/{tenant_id}/mounts
Response: {
    "tenant_id": "uuid",
    "working_layer_id": "uuid",
    "mounts": [
        {
            "mount_entry_id": "uuid",
            "name": "workspace",
            "virtual_path": "/workspace",
            "is_file": false,
            "source_type": "working_layer",
            "mode": "rw",
            "enabled": true
        },
        {
            "mount_entry_id": "uuid",
            "name": "models",
            "virtual_path": "/models",
            "is_file": false,
            "source_type": "layer",
            "source_tenant_id": "shared-models-tenant-uuid",
            "source_layer_id": "layer-uuid",
            "source_subpath": null,
            "mode": "ro",
            "enabled": true
        },
        {
            "mount_entry_id": "uuid",
            "name": "usr",
            "virtual_path": "/usr",
            "is_file": false,
            "source_type": "host",
            "host_path": "/usr",
            "mode": "ro",
            "enabled": true
        },
        {
            "mount_entry_id": "uuid",
            "name": "config",
            "virtual_path": "/config.yaml",
            "is_file": true,
            "source_type": "host",
            "host_path": "/etc/app/config.yaml",
            "mode": "ro",
            "enabled": true
        }
    ]
}

# 添加目录挂载
# name 用于 API 引用（snapshot/publish），virtual_path 是实际挂载路径
POST /tenants/{tenant_id}/mounts
{
    "name": "data",
    "virtual_path": "/data",
    "is_file": false,
    "source_type": "layer",
    "source_tenant_id": "data-tenant-uuid",
    "source_layer_id": "dataset-layer-uuid",
    "source_subpath": "/datasets/imagenet",
    "mode": "cow"
}
Response: {
    "mount_entry_id": "new-uuid",
    "name": "data",
    "virtual_path": "/data",
    "is_file": false,
    ...
}

# 添加单文件挂载
POST /tenants/{tenant_id}/mounts
{
    "name": "settings",
    "virtual_path": "/settings.json",
    "is_file": true,
    "source_type": "host",
    "host_path": "/etc/app/settings.json",
    "mode": "ro"
}
Response: {
    "mount_entry_id": "new-uuid",
    "name": "settings",
    "virtual_path": "/settings.json",
    "is_file": true,
    ...
}

# 使用已发布的 Layer 名称添加挂载
POST /tenants/{tenant_id}/mounts
{
    "name": "models",
    "virtual_path": "/models",
    "is_file": false,
    "published_layer_name": "pretrained-bert-base",
    "mode": "ro"
}

# 添加宿主机目录挂载
POST /tenants/{tenant_id}/mounts
{
    "name": "usr",
    "virtual_path": "/usr",
    "is_file": false,
    "source_type": "host",
    "host_path": "/usr",
    "mode": "ro"
}

# 更新挂载
PUT /tenants/{tenant_id}/mounts/{mount_entry_id}
{
    "mode": "cow",
    "enabled": true
}
Response: {
    "mount_entry_id": "uuid",
    "virtual_path": "/data",
    "mode": "cow",
    "enabled": true,
    "updated_at": "2026-01-29T11:00:00Z"
}

# 删除挂载
DELETE /tenants/{tenant_id}/mounts/{mount_entry_id}

# 启用挂载
POST /tenants/{tenant_id}/mounts/{mount_entry_id}/enable

# 禁用挂载
POST /tenants/{tenant_id}/mounts/{mount_entry_id}/disable

# Snapshot 特定挂载点（使用挂载点名称，不是路径）
POST /tenants/{tenant_id}/mounts/{mount_name}/snapshot
{
    "name": "memory-v1",
    "description": "Memory after task 1"
}
Response: {
    "layer_id": "new-layer-uuid",
    "mount_entry_id": "mount-uuid",
    "mount_name": "memory",
    "name": "memory-v1",
    "parent_layer_id": "previous-working-uuid",
    "created_at": "2026-01-29T12:00:00Z"
}

# Snapshot 多个挂载点
POST /tenants/{tenant_id}/snapshot
{
    "mounts": ["memory", "workspace"],
    "name": "checkpoint-1",
    "skip_unchanged": true
}
Response: {
    "snapshots": [
        {"mount_name": "memory", "layer_id": "new-layer-uuid"},
        {"mount_name": "workspace", "skipped": true, "reason": "no changes"}
    ]
}

# 发布挂载点的特定 Layer（内容固定）
POST /tenants/{tenant_id}/mounts/{mount_name}/publish
{
    "publish_name": "bert-base-v1",
    "description": "BERT base model weights",
    "target": "layer",
    "layer_id": "layer-uuid",
    "scope": "public"
}
Response: {
    "publish_id": "publish-uuid",
    "mount_entry_id": "mount-uuid",
    "mount_name": "models",
    "tenant_id": "model-hub-uuid",
    "target_type": "layer",
    "layer_id": "layer-uuid",
    "publish_name": "bert-base-v1",
    "description": "BERT base model weights",
    "scope": "public",
    "published_at": "2026-01-29T12:00:00Z"
}

# 发布挂载点的 Working Layer（实时跟随）
POST /tenants/{tenant_id}/mounts/{mount_name}/publish
{
    "publish_name": "agent1-memory",
    "description": "Shared memory for agents",
    "target": "working_layer",
    "scope": "public"
}
Response: {
    "publish_id": "publish-uuid",
    "mount_entry_id": "mount-uuid",
    "mount_name": "memory",
    "tenant_id": "agent-001-uuid",
    "target_type": "working_layer",
    "layer_id": null,
    "publish_name": "agent1-memory",
    "description": "Shared memory for agents",
    "scope": "public",
    "published_at": "2026-01-29T12:00:00Z"
}

# 发布（仅指定 tenant 可访问）
POST /tenants/{tenant_id}/mounts/{mount_name}/publish
{
    "publish_name": "private-model-v1",
    "description": "Private fine-tuned model",
    "target": "working_layer",
    "scope": "allow_list",
    "allowed_tenants": ["tenant-a-uuid", "tenant-b-uuid"]
}

# 取消发布
DELETE /tenants/{tenant_id}/mounts/{mount_name}/publish

# 获取 Tenant 已发布的挂载列表
GET /tenants/{tenant_id}/published-mounts
Response: {
    "tenant_id": "uuid",
    "published_mounts": [
        {
            "publish_id": "publish-uuid",
            "mount_entry_id": "mount-uuid",
            "mount_name": "memory",
            "publish_name": "agent1-memory",
            "target_type": "working_layer",
            "layer_id": null,
            "scope": "public",
            "published_at": "2026-01-29T12:00:00Z"
        }
    ]
}

# 更新发布信息
PUT /tenants/{tenant_id}/layers/{layer_id}/publish
{
    "description": "Updated description",
    "scope": "public"
}

# 获取已发布的 Layer 列表
GET /published-layers
Query Parameters:
  - scope: public|all (可选)
  - owner_tenant_id: 过滤特定 owner (可选)
Response: {
    "published_layers": [
        {
            "layer_id": "layer-uuid",
            "tenant_id": "model-hub-uuid",
            "publish_name": "pretrained-bert-base",
            "description": "BERT base model weights",
            "scope": "public",
            "layer_info": {
                "name": "bert-base-v1",
                "file_count": 15,
                "total_size": 440000000
            },
            "published_at": "2026-01-15T10:00:00Z"
        },
        {
            "layer_id": "layer-uuid",
            "tenant_id": "data-hub-uuid",
            "publish_name": "imagenet-dataset-v1",
            "description": "ImageNet training dataset",
            "scope": "public",
            "published_at": "2026-01-10T08:00:00Z"
        }
    ]
}

# 获取已发布 Layer 详情
GET /published-layers/{publish_name}
Response: {
    "layer_id": "layer-uuid",
    "tenant_id": "model-hub-uuid",
    "publish_name": "pretrained-bert-base",
    "description": "BERT base model weights",
    "scope": "public",
    "allowed_tenants": null,
    "layer_info": {
        "name": "bert-base-v1",
        "file_count": 15,
        "total_size": 440000000,
        "created_at": "2026-01-15T09:00:00Z"
    },
    "usage_count": 42,
    "published_at": "2026-01-15T10:00:00Z"
}

# 解析路径（调试用）
GET /tenants/{tenant_id}/resolve?path=/data/models/bert.bin
Response: {
    "path": "/data/models/bert.bin",
    "resolved": {
        "mount_entry_id": "uuid",
        "mount_name": "data",
        "virtual_path": "/data/models",
        "source_type": "layer",
        "source_tenant_id": "model-hub-uuid",
        "source_layer_id": "layer-uuid",
        "relative_path": "bert.bin",
        "mode": "ro"
    }
}
```

### 层管理操作

```http
# 列出所有层
GET /layers
Response: {
    "layers": [
        {
            "layer_id": "uuid",
            "name": "checkpoint-001",
            "parent_layer_id": "parent-uuid",
            "is_readonly": true,
            "file_count": 1234,
            "added_files": 100,
            "modified_files": 50,
            "deleted_files": 10,
            "total_size": 1048576,
            "created_at": "2026-01-14T12:00:00Z"
        }
    ]
}

# 获取层详情
GET /layers/{layer_id}

# 创建检查点
POST /layers/checkpoint
{
    "name": "checkpoint-001",
    "description": "After data preprocessing"
}
Response: {
    "layer_id": "new-uuid",
    "name": "checkpoint-001",
    ...
}

# 切换层
POST /layers/switch
{
    "layer_id": "target-uuid",
    "create_branch_if_readonly": true
}

# 获取当前层
GET /layers/current

# 获取层的变化
GET /layers/{layer_id}/changes
Response: {
    "added": [
        {"path": "/data/file1.txt", "size": 1024}
    ],
    "modified": [
        {"path": "/data/file2.txt", "old_size": 2048, "new_size": 3072}
    ],
    "deleted": [
        {"path": "/data/file3.txt"}
    ]
}

# 合并层
POST /layers/squash
{
    "layer_ids": ["uuid1", "uuid2", "uuid3"],
    "new_name": "squashed-layer",
    "new_description": "Merged layers"
}

# 创建分支
POST /layers/{layer_id}/branch
{
    "branch_name": "experiment-2",
    "description": "New experiment branch"
}

# 获取层树
GET /layers/tree
Response: {
    "root": {
        "layer_id": "base-uuid",
        "name": "base",
        "children": [
            {
                "layer_id": "child1-uuid",
                "name": "checkpoint-001",
                "children": [...]
            }
        ]
    }
}

# 获取文件历史
GET /layers/file-history?path=/data/file.txt
Response: {
    "history": [
        {
            "layer_id": "uuid",
            "layer_name": "checkpoint-003",
            "operation": "modify",
            "timestamp": "2026-01-14T12:00:00Z"
        },
        {
            "layer_id": "uuid2",
            "layer_name": "checkpoint-001",
            "operation": "add",
            "timestamp": "2026-01-13T10:00:00Z"
        }
    ]
}

# 删除层
DELETE /layers/{layer_id}?force=false
```

### 审计查询

```http
# 查询审计日志
GET /audit/query?
    start_time=2026-01-14T00:00:00Z&
    end_time=2026-01-14T23:59:59Z&
    operation=write&
    path=/data/*&
    limit=100
Response: {
    "events": [
        {
            "event_id": "uuid",
            "timestamp": "2026-01-14T12:00:00Z",
            "operation": "write",
            "path": "/data/file.txt",
            "uid": 1000,
            "gid": 1000,
            "pid": 12345,
            "success": true,
            "bytes_written": 1024,
            "duration_ms": 5
        }
    ],
    "total": 1234,
    "has_more": true
}

# 审计统计
GET /audit/stats?
    start_time=2026-01-14T00:00:00Z&
    end_time=2026-01-14T23:59:59Z&
    group_by=operation
Response: {
    "stats": [
        {
            "operation": "read",
            "count": 10000,
            "avg_duration_ms": 2.5,
            "total_bytes": 10485760
        },
        {
            "operation": "write",
            "count": 5000,
            "avg_duration_ms": 5.0,
            "total_bytes": 5242880
        }
    ]
}

# 用户活动
GET /audit/user-activity?uid=1000&time_range=24h
Response: {
    "uid": 1000,
    "operations": 1234,
    "files_accessed": 567,
    "bytes_read": 10485760,
    "bytes_written": 5242880,
    "top_files": [
        {"path": "/data/file.txt", "access_count": 100}
    ]
}
```

### 系统管理

```http
# 获取系统状态
GET /system/status
Response: {
    "version": "0.1.0",
    "uptime_seconds": 86400,
    "database": {
        "connected": true,
        "pool_size": 20,
        "active_connections": 5
    },
    "cache": {
        "size_bytes": 1073741824,
        "used_bytes": 536870912,
        "hit_rate": 0.95
    },
    "layers": {
        "total": 10,
        "active": "uuid"
    }
}

# 获取性能指标
GET /system/metrics
Response: {
    "operations_per_second": 1000,
    "avg_latency_ms": 2.5,
    "cache_hit_rate": 0.95,
    "storage_used_bytes": 10737418240,
    "storage_available_bytes": 107374182400
}

# 刷新缓存
POST /system/cache/flush

# 健康检查
GET /health
Response: {
    "status": "healthy",
    "checks": {
        "database": "ok",
        "fuse": "ok",
        "cache": "ok"
    }
}
```

## gRPC API

### 服务定义

```protobuf
syntax = "proto3";

package tarbox.api.v1;

service TarboxService {
    // 文件系统操作
    rpc Stat(StatRequest) returns (StatResponse);
    rpc List(ListRequest) returns (ListResponse);
    rpc Read(ReadRequest) returns (stream ReadResponse);
    rpc Write(stream WriteRequest) returns (WriteResponse);
    rpc Create(CreateRequest) returns (CreateResponse);
    rpc Delete(DeleteRequest) returns (DeleteResponse);
    
    // Tenant 操作
    rpc ListTenants(ListTenantsRequest) returns (ListTenantsResponse);
    rpc GetTenant(GetTenantRequest) returns (GetTenantResponse);
    rpc CreateTenant(CreateTenantRequest) returns (CreateTenantResponse);
    rpc UpdateTenant(UpdateTenantRequest) returns (UpdateTenantResponse);
    rpc DeleteTenant(DeleteTenantRequest) returns (DeleteTenantResponse);
    rpc SuspendTenant(SuspendTenantRequest) returns (SuspendTenantResponse);
    rpc ResumeTenant(ResumeTenantRequest) returns (ResumeTenantResponse);
    rpc GetTenantStats(GetTenantStatsRequest) returns (GetTenantStatsResponse);
    rpc CleanupTenantData(CleanupTenantDataRequest) returns (CleanupTenantDataResponse);
    rpc ExportTenantData(ExportTenantDataRequest) returns (ExportTenantDataResponse);
    rpc ImportTenantData(ImportTenantDataRequest) returns (ImportTenantDataResponse);
    
    // 层操作
    rpc ListLayers(ListLayersRequest) returns (ListLayersResponse);
    rpc CreateCheckpoint(CreateCheckpointRequest) returns (CreateCheckpointResponse);
    rpc SwitchLayer(SwitchLayerRequest) returns (SwitchLayerResponse);
    rpc GetLayerChanges(GetLayerChangesRequest) returns (GetLayerChangesResponse);
    
    // 审计
    rpc QueryAudit(QueryAuditRequest) returns (stream AuditEvent);
    rpc GetAuditStats(GetAuditStatsRequest) returns (GetAuditStatsResponse);
}

message StatRequest {
    string path = 1;
}

message StatResponse {
    uint64 inode_id = 1;
    string type = 2;  // file, directory, symlink
    uint64 size = 3;
    uint32 mode = 4;
    uint32 uid = 5;
    uint32 gid = 6;
    int64 atime = 7;
    int64 mtime = 8;
    int64 ctime = 9;
}

message ReadRequest {
    string path = 1;
    uint64 offset = 2;
    uint32 length = 3;
}

message ReadResponse {
    bytes data = 1;
    uint32 bytes_read = 2;
}

message WriteRequest {
    string path = 1;
    uint64 offset = 2;
    bytes data = 3;
    bool create_if_not_exists = 4;
}

message WriteResponse {
    uint32 bytes_written = 1;
}

// Tenant 消息定义
message ListTenantsRequest {
    int32 limit = 1;
    int32 offset = 2;
    string status = 3;  // active, suspended, deleted
}

message ListTenantsResponse {
    repeated TenantInfo tenants = 1;
    int32 total = 2;
}

message TenantInfo {
    string tenant_id = 1;
    string tenant_name = 2;
    string display_name = 3;
    string status = 4;
    string current_layer_id = 5;
    int64 storage_quota_bytes = 6;
    int64 storage_used_bytes = 7;
    int64 file_count = 8;
    int64 created_at = 9;
    int64 updated_at = 10;
    string metadata_json = 11;
}

message GetTenantRequest {
    string tenant_id = 1;
}

message GetTenantResponse {
    TenantInfo tenant = 1;
    int64 layer_count = 2;
    int64 last_access_at = 3;
}

message CreateTenantRequest {
    string tenant_name = 1;
    string display_name = 2;
    int64 storage_quota_bytes = 3;
    string metadata_json = 4;
}

message CreateTenantResponse {
    TenantInfo tenant = 1;
}

message UpdateTenantRequest {
    string tenant_id = 1;
    string display_name = 2;
    int64 storage_quota_bytes = 3;
    string metadata_json = 4;
}

message UpdateTenantResponse {
    TenantInfo tenant = 1;
}

message DeleteTenantRequest {
    string tenant_id = 1;
    bool force = 2;
}

message DeleteTenantResponse {
    string tenant_id = 1;
    string status = 2;
    int64 deleted_at = 3;
    int64 data_retention_until = 4;
}

message SuspendTenantRequest {
    string tenant_id = 1;
    string reason = 2;
}

message SuspendTenantResponse {
    string tenant_id = 1;
    string status = 2;
    int64 suspended_at = 3;
}

message ResumeTenantRequest {
    string tenant_id = 1;
}

message ResumeTenantResponse {
    string tenant_id = 1;
    string status = 2;
    int64 resumed_at = 3;
}

message GetTenantStatsRequest {
    string tenant_id = 1;
}

message GetTenantStatsResponse {
    string tenant_id = 1;
    StorageStats storage = 2;
    LayerStats layers = 3;
    OperationStats operations = 4;
    int64 last_access_at = 5;
}

message StorageStats {
    int64 used_bytes = 1;
    int64 quota_bytes = 2;
    double usage_percentage = 3;
    int64 file_count = 4;
    int64 directory_count = 5;
}

message LayerStats {
    int32 total_count = 1;
    string active_layer_id = 2;
    int64 base_layer_size = 3;
    int64 all_layers_size = 4;
}

message OperationStats {
    int64 read_ops_24h = 1;
    int64 write_ops_24h = 2;
    int64 delete_ops_24h = 3;
}

message CleanupTenantDataRequest {
    string tenant_id = 1;
    bool remove_deleted_files = 2;
    bool vacuum_blocks = 3;
    bool compress_layers = 4;
}

message CleanupTenantDataResponse {
    string tenant_id = 1;
    int64 cleanup_started_at = 2;
    int64 estimated_space_freed = 3;
    string job_id = 4;
}

message ExportTenantDataRequest {
    string tenant_id = 1;
    string format = 2;  // tar.gz, zip
    bool include_history = 3;
    string layer_id = 4;
}

message ExportTenantDataResponse {
    string tenant_id = 1;
    string export_job_id = 2;
    string status = 3;
    int64 created_at = 4;
}

message ImportTenantDataRequest {
    string tenant_id = 1;
    string source_url = 2;
    string merge_strategy = 3;  // replace, merge, skip
    bool create_checkpoint = 4;
}

message ImportTenantDataResponse {
    string tenant_id = 1;
    string import_job_id = 2;
    string status = 3;
    int64 created_at = 4;
}

// ... 其他消息定义
```

## CLI 工具

### 命令结构

```bash
tarbox [global-options] <command> [command-options] [arguments]

Global Options:
  --config <file>      配置文件路径
  --database <url>     数据库连接 URL
  --mount-point <path> 挂载点路径
  --verbose, -v        详细输出
  --quiet, -q          静默模式
  --json               JSON 输出
```

### 文件系统命令

```bash
# 挂载文件系统
tarbox mount [options]
  --mount-point <path>     挂载点 (必需)
  --database <url>         数据库 URL
  --cache-size <size>      缓存大小
  --audit-level <level>    审计级别
  --namespace <ns>         命名空间
  --readonly               只读挂载
  --foreground             前台运行

# 卸载
tarbox umount <mount-point>

# 文件操作
tarbox ls <path>
  --long, -l              详细列表
  --recursive, -R         递归列表
  --layer <id>            指定层

tarbox cat <path>
tarbox cp <source> <dest>
tarbox mv <source> <dest>
tarbox rm <path> [--recursive]
tarbox mkdir <path> [--parents]
```

### 文件系统组合命令（Mount）

> 混合挂载通过配置文件完成，不支持命令行逐条添加

```bash
# 从配置文件应用挂载（主要方式）
tarbox mount apply --tenant <tenant-id> --config mounts.toml

# 配置文件示例 (mounts.toml):
# ----------------------------------------
# [[mounts]]
# name = "workspace"
# path = "/workspace"
# source = "working_layer"
# mode = "rw"
#
# [[mounts]]
# name = "models"
# path = "/models"
# source = "layer:model-hub:bert-base-v1"
# mode = "ro"
#
# [[mounts]]
# name = "data"
# path = "/data"
# source = "published:imagenet-v1"
# subpath = "/train"
# mode = "cow"
#
# [[mounts]]
# name = "usr"
# path = "/usr"
# source = "host:/usr"
# mode = "ro"
#
# [[mounts]]
# name = "bin"
# path = "/bin"
# source = "host:/bin"
# mode = "ro"
#
# [[mounts]]
# name = "config"
# path = "/config.yaml"
# file = true
# source = "host:/etc/app/config.yaml"
# mode = "ro"
# ----------------------------------------

# 查看当前挂载配置
tarbox mount list --tenant <tenant-id>
  --json                               JSON 输出
  --toml                               TOML 输出（可直接保存为配置文件）

示例输出：
NAME        PATH            TYPE  SOURCE                          MODE  ENABLED
workspace   /workspace      dir   working_layer                   rw    true
models      /models         dir   layer:model-hub:bert-base       ro    true
usr         /usr            dir   host:/usr                       ro    true
config      /config.yaml    file  host:/etc/app/config.yaml       ro    true

# 导出当前配置到文件
tarbox mount export --tenant <tenant-id> --output mounts.toml

# 验证配置文件（不实际应用）
tarbox mount validate --config mounts.toml

# 清空所有挂载
tarbox mount clear --tenant <tenant-id>

# 删除单个挂载（使用挂载点名称）
tarbox mount remove --tenant <tenant-id> --mount <mount-name>

示例：
tarbox mount remove --tenant uuid-123 --mount models

# 启用/禁用挂载
tarbox mount enable --tenant <tenant-id> --mount <mount-name>
tarbox mount disable --tenant <tenant-id> --mount <mount-name>

# 更新挂载配置
tarbox mount update --tenant <tenant-id> --mount <mount-name> [options]
  --mode <ro|rw|cow>                   更新挂载模式
  --enabled <true|false>               启用/禁用

示例：
tarbox mount update --tenant uuid-123 --mount data --mode cow

# 解析路径（调试用）
tarbox mount resolve --tenant <tenant-id> /models/bert/config.json

示例输出：
Path:         /models/bert/config.json
Mount Name:   models
Mount Path:   /models (dir)
Source:       layer:model-hub:bert-base-v1
Relative:     bert/config.json
Mode:         ro
```

#### 配置文件格式

```toml
# mounts.toml - Tarbox 混合挂载配置
# 注意：name 用于 API 引用（如 snapshot/publish），path 是实际挂载路径

# 工作层（可写）
[[mounts]]
name = "workspace"                       # API 引用名称（用于 snapshot/publish 命令）
path = "/workspace"                      # 实际挂载路径
source = "working_layer"
mode = "rw"

# 从其他 Tenant 的 Layer 挂载（只读）
[[mounts]]
name = "models"
path = "/models"
source = "layer:model-hub:bert-base-v1"  # 格式: layer:<tenant>:<layer>
mode = "ro"

# 从已发布的 Layer 挂载（写时复制）
[[mounts]]
name = "data"
path = "/data"
source = "published:imagenet-v1"         # 格式: published:<publish-name>
subpath = "/train"                       # 可选：只挂载子目录
mode = "cow"

# 从宿主机挂载目录
[[mounts]]
name = "usr"
path = "/usr"
source = "host:/usr"                     # 格式: host:<host-path>
mode = "ro"

[[mounts]]
name = "bin"
path = "/bin"
source = "host:/bin"
mode = "ro"

[[mounts]]
name = "lib"
path = "/lib"
source = "host:/lib"
mode = "ro"

# 从宿主机挂载单个文件
[[mounts]]
name = "config"
path = "/config.yaml"
file = true                              # 标记为文件挂载
source = "host:/etc/app/config.yaml"
mode = "ro"

# 可选字段
[[mounts]]
name = "cache"
path = "/cache"
source = "host:/var/cache/app"
mode = "rw"
enabled = false                          # 默认禁用
```

#### 字段说明

| 字段 | 必需 | 说明 | 示例 |
|------|------|------|------|
| `name` | 是 | API 引用名称（用于 snapshot/publish 命令） | `name = "models"` |
| `path` | 是 | 实际挂载路径 | `path = "/models"` |
| `source` | 是 | 数据来源（见下表） | `source = "working_layer"` |
| `mode` | 是 | 挂载模式 (ro/rw/cow) | `mode = "ro"` |
| `file` | 否 | 是否为单文件挂载 | `file = true` |
| `subpath` | 否 | 只挂载源的子目录 | `subpath = "/train"` |
| `enabled` | 否 | 是否启用（默认 true） | `enabled = false` |

#### Source 格式说明

| 格式 | 说明 | 示例 |
|------|------|------|
| `working_layer` | 当前 Tenant 的工作层 | `source = "working_layer"` |
| `host:<path>` | 宿主机目录/文件 | `source = "host:/usr"` |
| `layer:<tenant>:<layer>` | 指定 Tenant 的 Layer | `source = "layer:model-hub:bert-v1"` |
| `published:<name>` | 已发布的 Layer（按发布名称） | `source = "published:imagenet-v1"` |

#### Mode 说明

| Mode | 说明 |
|------|------|
| `ro` | 只读，写入返回 EROFS |
| `rw` | 读写，直接写入源（仅 host 和 working_layer） |
| `cow` | 写时复制，读取来自源，写入到 working_layer |

### 文件系统组合操作 HTTP/CLI 对照表

| 操作 | HTTP API | CLI |
|------|----------|-----|
| 批量设置挂载 | `PUT /tenants/{id}/mounts` | `tarbox mount apply --config` |
| 导入挂载配置 | `POST /tenants/{id}/mounts/import` | `tarbox mount apply --config` |
| 导出挂载配置 | `GET /tenants/{id}/mounts/export` | `tarbox mount export --output` |
| 列出挂载配置 | `GET /tenants/{id}/mounts` | `tarbox mount list` |
| 添加单个挂载 | `POST /tenants/{id}/mounts` | (使用配置文件) |
| 更新挂载 | `PUT /tenants/{id}/mounts/{entry_id}` | `tarbox mount update --mount` |
| 删除挂载 | `DELETE /tenants/{id}/mounts/{entry_id}` | `tarbox mount remove --mount` |
| 启用挂载 | `POST /tenants/{id}/mounts/{entry_id}/enable` | `tarbox mount enable --mount` |
| 禁用挂载 | `POST /tenants/{id}/mounts/{entry_id}/disable` | `tarbox mount disable --mount` |
| 清空所有挂载 | (多次 DELETE) | `tarbox mount clear` |
| 验证配置 | - | `tarbox mount validate --config` |
| 解析路径 | `GET /tenants/{id}/resolve?path=` | `tarbox mount resolve` |
| Snapshot 单个挂载 | `POST /tenants/{id}/mounts/{name}/snapshot` | `tarbox snapshot --mount` |
| Snapshot 多个挂载 | `POST /tenants/{id}/snapshot` | `tarbox snapshot --mount x --mount y` |
| 发布挂载 | `POST /tenants/{id}/mounts/{name}/publish` | `tarbox publish --mount` |
| 取消发布 | `DELETE /tenants/{id}/mounts/{name}/publish` | `tarbox unpublish --mount` |
| 列出已发布 (全局) | `GET /published-layers` | `tarbox layer list-published` |
| 列出已发布 (Tenant) | `GET /tenants/{id}/published-mounts` | `tarbox publish list --tenant` |
| 查看发布详情 | `GET /published-layers/{name}` | `tarbox layer publish-info` |
| 更新发布信息 | `PUT /tenants/{id}/layers/{id}/publish` | `tarbox layer publish-update` |
| 添加授权租户 | (PUT 更新 scope) | `tarbox layer publish-allow` |
| 移除授权租户 | (PUT 更新 scope) | `tarbox layer publish-revoke` |

### Layer 发布命令

```bash
# 列出已发布的 Layer
tarbox layer list-published
  --scope <public|all>                 过滤范围
  --owner <tenant-id>                  过滤所有者
  --json                               JSON 输出

示例输出：
PUBLISH_NAME            OWNER         SCOPE         SIZE      PUBLISHED
pretrained-bert-base    model-hub     public        420MB     2026-01-15
imagenet-dataset-v1     data-hub      public        140GB     2026-01-10
private-model-v1        my-tenant     allow_list    850MB     2026-01-20

# Snapshot 特定挂载点（使用挂载点名称，不是路径）
tarbox snapshot --tenant <tenant-id> --mount memory --name "memory-v1"

# Snapshot 多个挂载点
tarbox snapshot --tenant <tenant-id> --mount memory --mount workspace --name "checkpoint-1"

# Snapshot 所有 WorkingLayer 挂载点（跳过无变化的）
tarbox snapshot --tenant <tenant-id> --all --name "full-checkpoint" --skip-unchanged

# 发布挂载点的特定 Layer（内容固定）
# 注意：--mount 使用挂载点名称（如 "models"），不是路径（如 "/models"）
tarbox publish \
    --tenant <tenant-id> \
    --mount models \
    --layer <layer-id-or-name> \
    --name "my-model-v1" \
    --description "My fine-tuned model" \
    --scope public

# 发布挂载点的 Working Layer（实时共享）
tarbox publish \
    --tenant <tenant-id> \
    --mount memory \
    --target working_layer \
    --name "agent1-memory" \
    --description "Shared memory for agents" \
    --scope public

# 发布（仅指定 tenant 可访问）
tarbox publish \
    --tenant <tenant-id> \
    --mount data \
    --target working_layer \
    --name "private-dataset" \
    --scope allow_list \
    --allow tenant-a,tenant-b,tenant-c

# 查看已发布 Layer 详情
tarbox layer publish-info my-model-v1

# 更新发布信息
tarbox layer publish-update my-model-v1 \
    --description "Updated description" \
    --scope public

# 添加授权租户（allow_list scope）
tarbox layer publish-allow my-model-v1 tenant-d

# 移除授权租户
tarbox layer publish-revoke my-model-v1 tenant-d

# 取消发布（使用挂载点名称）
tarbox unpublish --tenant <tenant-id> --mount <mount-name>

示例：
tarbox unpublish --tenant uuid-123 --mount memory

# 列出 Tenant 已发布的挂载
tarbox publish list --tenant <tenant-id>
  --json                               JSON 输出

示例输出：
MOUNT       PUBLISH_NAME      TARGET          SCOPE         PUBLISHED
memory      agent1-memory     working_layer   public        2026-01-29
models      bert-base-v1      layer           allow_list    2026-01-28
```

### Tenant 管理命令

```bash
# 列出所有 Tenant
tarbox tenant list
  --status <active|suspended|deleted>  过滤状态
  --limit <n>                          限制数量
  --json                               JSON 输出

# 获取 Tenant 详情
tarbox tenant get <tenant-id>
  --json                               JSON 输出

# 创建 Tenant
tarbox tenant create <tenant-name>
  --display-name <name>                显示名称
  --quota <size>                       存储配额 (如: 100GB, 1TB)
  --metadata <json>                    元数据 JSON

示例：
tarbox tenant create ai-agent-001 \
  --display-name "AI Agent 001" \
  --quota 100GB \
  --metadata '{"owner":"user@example.com","project":"ml-training"}'

# 更新 Tenant
tarbox tenant update <tenant-id>
  --display-name <name>                更新显示名称
  --quota <size>                       更新存储配额
  --metadata <json>                    更新元数据

示例：
tarbox tenant update uuid-123 --quota 200GB

# 删除 Tenant
tarbox tenant delete <tenant-id>
  --force                              强制删除（跳过保留期）

# 暂停 Tenant
tarbox tenant suspend <tenant-id>
  --reason <text>                      暂停原因

示例：
tarbox tenant suspend uuid-123 --reason "Quota exceeded"

# 恢复 Tenant
tarbox tenant resume <tenant-id>

# 获取 Tenant 统计信息
tarbox tenant stats <tenant-id>
  --json                               JSON 输出

示例输出：
Storage:
  Used: 10.0 GB / 100.0 GB (10.0%)
  Files: 12,345
  Directories: 567

Layers:
  Total: 5 layers
  Active: checkpoint-003
  Base size: 1.0 GB
  All layers: 5.0 GB

Operations (24h):
  Reads: 100,000
  Writes: 50,000
  Deletes: 1,000

Last Access: 2026-01-15 10:30:00

# 获取 Tenant 的所有层
tarbox tenant layers <tenant-id>
  --tree                               树形显示
  --json                               JSON 输出

# 清理 Tenant 数据
tarbox tenant cleanup <tenant-id>
  --remove-deleted-files               清理已删除文件
  --vacuum-blocks                      清理未使用块
  --compress-layers                    压缩层
  --dry-run                            仅显示将清理的内容

示例：
tarbox tenant cleanup uuid-123 --remove-deleted-files --vacuum-blocks

# 导出 Tenant 数据
tarbox tenant export <tenant-id>
  --output <file>                      输出文件路径
  --format <tar.gz|zip>                导出格式
  --include-history                    包含历史层
  --layer <layer-id>                   导出特定层

示例：
tarbox tenant export uuid-123 \
  --output /backup/tenant-backup.tar.gz \
  --include-history

# 导入 Tenant 数据
tarbox tenant import <tenant-id>
  --input <file>                       输入文件路径
  --merge-strategy <replace|merge|skip> 合并策略
  --create-checkpoint                  导入后创建检查点

示例：
tarbox tenant import uuid-123 \
  --input /backup/tenant-backup.tar.gz \
  --merge-strategy replace \
  --create-checkpoint

# 切换到 Tenant 上下文（用于后续命令）
tarbox tenant use <tenant-id>
# 之后的文件系统命令会在该 tenant 下执行

# 显示当前使用的 Tenant
tarbox tenant current
```

### 层管理命令

```bash
# 列出层
tarbox layer list
  --tree                  树形显示
  --json                  JSON 输出

# 创建检查点
tarbox layer checkpoint <name>
  --description <desc>    描述

# 切换层
tarbox layer switch <layer-id-or-name>
  --branch                创建新分支

# 查看当前层
tarbox layer current

# 查看层变化
tarbox layer diff <layer-id>
  --summary               仅显示摘要
  --detailed              显示详细差异

# 查看文件历史
tarbox layer history <path>

# 层树可视化
tarbox layer tree

# 创建分支
tarbox layer branch <name>
  --from <layer-id>       从指定层创建

# 合并层
tarbox layer squash <layer-ids...>
  --name <name>           新层名称

# 删除层
tarbox layer delete <layer-id>
  --force                 强制删除
```

### 审计命令

```bash
# 查询审计日志
tarbox audit query
  --path <pattern>        路径模式
  --operation <op>        操作类型
  --uid <uid>             用户 ID
  --time-range <range>    时间范围 (1h, 24h, 7d)
  --limit <n>             限制结果数量

# 审计统计
tarbox audit stats
  --group-by <field>      分组字段
  --time-range <range>    时间范围

# 用户活动
tarbox audit user <uid>
  --time-range <range>

# 导出审计
tarbox audit export
  --format <json|csv>     输出格式
  --output <file>         输出文件
```

### 系统管理命令

```bash
# 初始化数据库
tarbox init
  --database <url>
  --force                 强制重新初始化

# 系统状态
tarbox status
  --watch                 持续监控

# 性能统计
tarbox stats
  --interval <seconds>    刷新间隔

# 缓存管理
tarbox cache flush
tarbox cache stats

# 数据库维护
tarbox maintenance vacuum
tarbox maintenance analyze
tarbox maintenance cleanup
  --before <date>         清理指定日期前的数据
```

## 错误处理

### 错误码设计

```rust
pub enum ErrorCode {
    // 通用错误 (1xxx)
    Unknown = 1000,
    InvalidArgument = 1001,
    NotFound = 1002,
    AlreadyExists = 1003,
    PermissionDenied = 1004,
    
    // 文件系统错误 (2xxx)
    FileNotFound = 2001,
    FileAlreadyExists = 2002,
    NotADirectory = 2003,
    IsADirectory = 2004,
    DirectoryNotEmpty = 2005,
    NoSpaceLeft = 2006,
    
    // 层错误 (3xxx)
    LayerNotFound = 3001,
    LayerIsReadonly = 3002,
    LayerHasChildren = 3003,
    InvalidLayerChain = 3004,
    
    // Tenant 错误 (6xxx)
    TenantNotFound = 6001,
    TenantAlreadyExists = 6002,
    TenantSuspended = 6003,
    TenantQuotaExceeded = 6004,
    TenantNameInvalid = 6005,
    TenantHasActiveConnections = 6006,
    
    // 挂载/组合错误 (7xxx)
    MountNotFound = 7001,
    MountAlreadyExists = 7002,
    MountPathConflict = 7003,
    MountSourceNotFound = 7004,
    MountAccessDenied = 7005,
    LayerNotPublished = 7006,
    PublishedLayerAccessDenied = 7007,
    HostPathNotAllowed = 7008,
    InvalidMountMode = 7009,
    CircularMountDependency = 7010,
    
    // 数据库错误 (4xxx)
    DatabaseError = 4001,
    ConnectionFailed = 4002,
    TransactionFailed = 4003,
    
    // 系统错误 (5xxx)
    InternalError = 5001,
    NotImplemented = 5002,
}
```

### 错误响应格式

```json
{
    "error": {
        "code": 2001,
        "message": "File not found",
        "details": {
            "path": "/data/nonexistent.txt"
        },
        "request_id": "uuid"
    }
}
```

## API 版本管理

```
版本策略：
- URL 路径版本：/api/v1, /api/v2
- 向后兼容：保持旧版本至少 6 个月
- 废弃通知：响应头 X-API-Deprecated: true
- 文档：为每个版本维护独立文档
```

## 认证和授权

### 认证方式

```
1. Bearer Token
   Header: Authorization: Bearer <token>

2. API Key
   Header: X-API-Key: <key>

3. mTLS (gRPC)
   Client Certificate

4. Kubernetes ServiceAccount (CSI)
   自动注入的 Token
```

### 授权模型

```
基于角色的访问控制 (RBAC)：

Roles:
- admin: 完全控制
- operator: 文件系统操作 + 层管理
- reader: 只读访问
- auditor: 审计查询

Permissions:
- fs:read
- fs:write
- fs:delete
- layer:create
- layer:switch
- layer:delete
- audit:query
- system:manage
```

## 速率限制

```
限制策略：
- 每个 Token: 1000 req/min
- 每个 IP: 100 req/min (未认证)
- 写操作: 100 req/min
- 查询操作: 1000 req/min

响应头：
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 999
X-RateLimit-Reset: 1610640000
```

## 文本文件 API

### Rust API

```
文本文件差异和历史查询接口：

trait TextFileManager {
    // 文件差异对比
    async fn diff_file(
        path: Path,
        layer_a: LayerId,
        layer_b: LayerId
    ) -> Result<TextFileDiff>;
    
    // 文件历史
    async fn file_history(
        path: Path
    ) -> Result<Vec<FileVersion>>;
    
    // 查看历史版本内容
    async fn read_file_at_layer(
        path: Path,
        layer_id: LayerId
    ) -> Result<String>;
}

数据结构：
- TextFileDiff: 包含行级差异信息
- FileVersion: 包含层ID、时间、大小、行数等
- LineChange: 表示行的增删改
```

### REST API

```
文本文件差异接口：

GET /api/v1/files/diff
Query Parameters:
  - path: 文件路径
  - layer_a: 源层ID
  - layer_b: 目标层ID
  - format: 输出格式（unified/json/side-by-side）

Response:
{
  "path": "/data/config.yaml",
  "layer_a": {"id": "layer-001", "name": "checkpoint-1"},
  "layer_b": {"id": "layer-002", "name": "checkpoint-2"},
  "changes": [
    {
      "type": "modified",
      "old_line_num": 50,
      "new_line_num": 50,
      "old_content": "port: 5432",
      "new_content": "port: 5433"
    },
    {
      "type": "added",
      "new_line_num": 51,
      "new_content": "pool_size: 20"
    }
  ],
  "summary": {
    "lines_added": 3,
    "lines_deleted": 1,
    "lines_modified": 2
  }
}

文件历史接口：

GET /api/v1/files/history
Query Parameters:
  - path: 文件路径
  - limit: 限制返回数量
  
Response:
{
  "path": "/data/config.yaml",
  "versions": [
    {
      "layer_id": "layer-003",
      "layer_name": "checkpoint-3",
      "timestamp": "2026-01-15T10:30:00Z",
      "size": 1250,
      "lines": 103,
      "changes": "+5 -2 ~3"
    },
    {
      "layer_id": "layer-002",
      "layer_name": "checkpoint-2",
      "timestamp": "2026-01-14T15:20:00Z",
      "size": 1200,
      "lines": 100,
      "changes": "+2 -1 ~1"
    }
  ]
}

查看历史版本内容：

GET /api/v1/files/content
Query Parameters:
  - path: 文件路径
  - layer_id: 层ID（可选，默认当前层）
  
Response:
  返回文件的文本内容
```

### gRPC API

```
service TextFileService {
    // 文件差异
    rpc DiffFile(DiffFileRequest) returns (DiffFileResponse);
    
    // 文件历史
    rpc GetFileHistory(FileHistoryRequest) returns (FileHistoryResponse);
    
    // 历史版本内容
    rpc ReadFileAtLayer(ReadFileAtLayerRequest) returns (ReadFileAtLayerResponse);
}

消息定义：

message DiffFileRequest {
    string path = 1;
    string layer_a = 2;
    string layer_b = 3;
    DiffFormat format = 4;
}

message DiffFileResponse {
    string path = 1;
    LayerInfo layer_a = 2;
    LayerInfo layer_b = 3;
    repeated LineChange changes = 4;
    DiffSummary summary = 5;
}

message LineChange {
    ChangeType type = 1;  // ADDED, DELETED, MODIFIED
    int32 old_line_num = 2;
    int32 new_line_num = 3;
    string old_content = 4;
    string new_content = 5;
}

message FileHistoryRequest {
    string path = 1;
    int32 limit = 2;
}

message FileHistoryResponse {
    string path = 1;
    repeated FileVersion versions = 2;
}

message FileVersion {
    string layer_id = 1;
    string layer_name = 2;
    int64 timestamp = 3;
    int64 size = 4;
    int32 lines = 5;
    string changes_summary = 6;
}
```

### CLI 工具

```
文本文件差异命令：

tarbox diff <layer-a> <layer-b> <path>
  --format <unified|json|side-by-side>  输出格式
  --color                                彩色输出
  --output <file>                        输出到文件

示例：
tarbox diff checkpoint-1 checkpoint-2 /data/config.yaml
tarbox diff layer-001 layer-002 /app/*.py --format json

输出示例（unified 格式）：
--- /data/config.yaml (checkpoint-1)
+++ /data/config.yaml (checkpoint-2)
@@ -48,7 +48,8 @@
 database:
   host: localhost
-  port: 5432
+  port: 5433
+  pool_size: 20
   database: tarbox

文件历史命令：

tarbox history <path>
  --limit <n>         限制显示数量
  --verbose           显示详细信息
  
示例：
tarbox history /data/config.yaml

输出示例：
Layer: checkpoint-3 (2026-01-15 10:30:00)
  Size: 1.25KB, Lines: 103 (+5 -2 ~3)
  
Layer: checkpoint-2 (2026-01-14 15:20:00)
  Size: 1.20KB, Lines: 100 (+2 -1 ~1)
  
Layer: checkpoint-1 (2026-01-13 09:00:00)
  Size: 1.18KB, Lines: 98 (initial)

查看历史版本：

tarbox cat <path> --layer <layer-id>

示例：
tarbox cat /data/config.yaml --layer checkpoint-1
```

## 测试

### API 测试套件

```
- 单元测试：测试每个 API 函数
- 集成测试：测试 API 端到端流程
- 负载测试：测试高并发场景
- 文本文件 diff 测试：测试各种差异场景
- 历史查询测试：测试跨层查询性能
- 兼容性测试：测试跨版本兼容性
```

### 测试工具

```bash
# REST API 测试
curl -X GET http://localhost:8080/api/v1/layers

# gRPC 测试
grpcurl -plaintext localhost:9090 tarbox.api.v1.TarboxService/ListLayers

# CLI 测试
tarbox layer list --json | jq
```

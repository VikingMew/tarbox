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

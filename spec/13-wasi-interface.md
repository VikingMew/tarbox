# Spec 13: WASI Adapter 技术设计

**优先级**: P2 (高级功能)  
**状态**: 部分实现  
**依赖**: spec/01 (数据库), spec/14 (文件系统接口)  
**相关**: spec/17 (tarbox-wasi crate 规划)

## 概述

本规范描述 Tarbox WASI 适配器的技术设计。WASI 适配器是一个**文件系统后端组件**，让外部 WASI 运行时（如 Wasmtime、WasmEdge）可以使用 Tarbox 作为文件系统实现。

**关键定位**：
- Tarbox **不是** WASI 运行时，**不执行** Wasm 代码
- Tarbox 提供 **WASI 兼容的文件系统适配器**
- 外部运行时通过适配器将文件操作路由到 Tarbox → PostgreSQL

## 架构定位

```
┌─────────────────────────────────────┐
│  AI Agent (Wasm 模块)               │
└─────────────────────────────────────┘
                 ↓
┌─────────────────────────────────────┐
│  WASI 运行时 (Wasmtime/WasmEdge)    │  ← 他们执行 Wasm
└─────────────────────────────────────┘
                 ↓ WASI filesystem 调用
┌─────────────────────────────────────┐
│  Tarbox WasiAdapter (本规范)        │  ← 我们提供的适配器
│  - 文件描述符管理                   │
│  - WASI 错误码映射                  │
│  - PostgreSQL 存储                  │
└─────────────────────────────────────┘
                 ↓
┌─────────────────────────────────────┐
│  Tarbox FileSystem                  │
└─────────────────────────────────────┘
                 ↓
┌─────────────────────────────────────┐
│  PostgreSQL (多租户存储)            │
└─────────────────────────────────────┘
```

## 设计目标

1. **文件系统后端**: 为 WASI 运行时提供持久化文件系统
2. **PostgreSQL 核心**: 保留 Tarbox 的核心能力（多租户、分层、审计）
3. **标准 fd API**: 提供 POSIX-like 的文件描述符操作
4. **可集成性**: 易于集成到各种 WASI 运行时
5. **双模式支持**: Direct（直连 PostgreSQL）和 HTTP（通过 API）

## 核心组件

### 1. WasiAdapter

**位置**: `src/wasi/adapter.rs`

适配器主结构，桥接 WASI 调用到 Tarbox 文件系统：

```rust
pub struct WasiAdapter<'a> {
    /// 底层文件系统实现
    fs: Arc<FileSystem<'a>>,
    /// 租户 ID（多租户隔离）
    tenant_id: Uuid,
    /// 文件描述符表
    fd_table: Arc<Mutex<FdTable>>,
    /// 配置
    config: WasiConfig,
}
```

**核心方法**：

| 方法 | 说明 |
|------|------|
| `fd_open(path, flags)` | 打开文件，返回 fd |
| `fd_read(fd, buf)` | 从 fd 读取数据 |
| `fd_write(fd, buf)` | 向 fd 写入数据 |
| `fd_close(fd)` | 关闭 fd |
| `fd_seek(fd, offset, whence)` | 改变文件位置 |
| `fd_filestat_get(fd)` | 获取文件元数据 |
| `fd_readdir(fd)` | 读取目录内容 |
| `path_open(dirfd, path, flags)` | 相对路径打开 |
| `path_create_directory(path)` | 创建目录 |
| `path_remove_directory(path)` | 删除目录 |
| `path_unlink_file(path)` | 删除文件 |
| `path_rename(old, new)` | 重命名 |

### 2. FdTable

**位置**: `src/wasi/fd_table.rs`

管理文件描述符的分配和状态：

```rust
pub struct FdTable {
    /// 下一个可用 fd
    next_fd: u32,
    /// fd -> FileDescriptor 映射
    descriptors: HashMap<u32, FileDescriptor>,
}

pub struct FileDescriptor {
    /// 对应的 inode ID
    pub inode_id: i64,
    /// 文件路径
    pub path: String,
    /// 打开标志
    pub flags: OpenFlags,
    /// 当前读写位置
    pub position: u64,
    /// 是否是目录
    pub is_directory: bool,
}
```

**OpenFlags 定义**：

```rust
bitflags! {
    pub struct OpenFlags: u32 {
        const READ    = 0b0001;  // 可读
        const WRITE   = 0b0010;  // 可写
        const CREATE  = 0b0100;  // 不存在则创建
        const TRUNC   = 0b1000;  // 截断
        const APPEND  = 0b10000; // 追加模式
    }
}
```

**fd 分配策略**：
- 从 3 开始（0=stdin, 1=stdout, 2=stderr 保留）
- 单调递增
- 关闭后不复用（简化实现）

### 3. WasiConfig

**位置**: `src/wasi/config.rs`

适配器配置：

```rust
pub struct WasiConfig {
    /// 数据库模式
    pub db_mode: DbMode,
    /// HTTP API URL（Http 模式）
    pub api_url: Option<String>,
    /// API 密钥（Http 模式）
    pub api_key: Option<String>,
    /// 缓存大小（MB）
    pub cache_size_mb: usize,
    /// 缓存 TTL（秒）
    pub cache_ttl_secs: u64,
    /// 默认租户 ID
    pub tenant_id: Option<Uuid>,
}

pub enum DbMode {
    /// 直连 PostgreSQL
    Direct,
    /// 通过 HTTP API
    Http,
}
```

**环境变量**：

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `TARBOX_DB_MODE` | `direct` 或 `http` | `direct` |
| `TARBOX_API_URL` | HTTP API 地址 | - |
| `TARBOX_API_KEY` | API 密钥 | - |
| `TARBOX_CACHE_SIZE` | 缓存大小（MB） | 100 |
| `TARBOX_CACHE_TTL` | 缓存 TTL（秒） | 300 |

### 4. WasiError

**位置**: `src/wasi/error.rs`

WASI 标准错误码映射：

```rust
pub enum WasiError {
    /// ENOENT - 文件不存在
    NotFound,
    /// EACCES - 权限拒绝
    PermissionDenied,
    /// EEXIST - 文件已存在
    AlreadyExists,
    /// EINVAL - 无效参数
    InvalidInput,
    /// EISDIR - 是目录
    IsDirectory,
    /// ENOTDIR - 不是目录
    NotDirectory,
    /// EBADF - 无效 fd
    BadFileDescriptor,
    /// ENOTEMPTY - 目录非空
    NotEmpty,
    /// EIO - I/O 错误
    IoError(String),
}
```

**转换函数**：

```rust
/// 转换为 WASI errno
pub fn to_wasi_errno(err: &WasiError) -> u16 {
    match err {
        WasiError::NotFound => 44,         // ENOENT
        WasiError::PermissionDenied => 2,  // EACCES
        WasiError::AlreadyExists => 20,    // EEXIST
        WasiError::InvalidInput => 28,     // EINVAL
        WasiError::IsDirectory => 31,      // EISDIR
        WasiError::NotDirectory => 54,     // ENOTDIR
        WasiError::BadFileDescriptor => 8, // EBADF
        WasiError::NotEmpty => 55,         // ENOTEMPTY
        WasiError::IoError(_) => 29,       // EIO
    }
}
```

## 数据流

### 文件读取流程

```
1. fd_open("/workspace/data.txt", READ)
   ├─ 验证路径（绝对路径，无 ..）
   ├─ FileSystem::stat() 获取 inode
   ├─ 创建 FileDescriptor
   ├─ 分配 fd，存入 FdTable
   └─ 返回 fd

2. fd_read(fd, buffer)
   ├─ FdTable 查找 fd
   ├─ 检查 READ 权限
   ├─ FileSystem::read_file() 读取数据
   ├─ 从 position 开始填充 buffer
   ├─ 更新 position
   └─ 返回读取字节数

3. fd_close(fd)
   ├─ FdTable 移除 fd
   └─ 释放 FileDescriptor
```

### 文件写入流程

```
1. fd_open("/workspace/output.txt", WRITE | CREATE)
   ├─ 验证路径
   ├─ 检查文件是否存在
   │   ├─ 不存在且有 CREATE → FileSystem::create_file()
   │   └─ 存在 → 获取 inode
   ├─ 创建 FileDescriptor
   └─ 返回 fd

2. fd_write(fd, data)
   ├─ FdTable 查找 fd
   ├─ 检查 WRITE 权限
   ├─ 根据 position 和 APPEND 标志确定写入位置
   ├─ FileSystem::write_file() 写入数据
   ├─ 更新 position
   └─ 返回写入字节数
```

### 目录操作流程

```
1. path_create_directory("/workspace/subdir")
   ├─ 验证路径
   ├─ 检查父目录存在
   ├─ FileSystem::create_directory()
   └─ 返回成功

2. fd_readdir(dir_fd)
   ├─ FdTable 查找 fd
   ├─ 检查是目录
   ├─ FileSystem::list_directory()
   └─ 返回目录项列表
```

## 安全模型

### 租户隔离

每个 `WasiAdapter` 实例绑定一个 `tenant_id`：

```rust
let adapter = WasiAdapter::new(fs, tenant_id, config);
// 所有操作限定在 tenant_id 下
```

- 数据库查询自动添加 `WHERE tenant_id = $1`
- 无法跨租户访问
- 配置错误的租户 ID 会导致空结果，不会泄露数据

### 路径验证

```rust
fn validate_path(path: &str) -> Result<(), WasiError> {
    // 必须是绝对路径
    if !path.starts_with('/') {
        return Err(WasiError::InvalidInput);
    }
    
    // 禁止 .. 逃逸
    if path.contains("..") {
        return Err(WasiError::PermissionDenied);
    }
    
    // 禁止访问 /.tarbox（系统目录）
    if path.starts_with("/.tarbox") {
        return Err(WasiError::PermissionDenied);
    }
    
    Ok(())
}
```

### 权限检查

```rust
impl FileDescriptor {
    pub fn can_read(&self) -> bool {
        self.flags.contains(OpenFlags::READ)
    }
    
    pub fn can_write(&self) -> bool {
        self.flags.contains(OpenFlags::WRITE)
    }
}
```

## HTTP 模式

当无法直连 PostgreSQL 时（如 edge 环境），使用 HTTP 模式：

```
WasiAdapter → HTTP Client → Tarbox API Server → PostgreSQL
```

### 请求格式

```http
POST /api/v1/fs/read
Content-Type: application/json
Authorization: Bearer <api_key>

{
  "tenant_id": "uuid",
  "path": "/workspace/file.txt"
}
```

### 响应格式

```http
HTTP/1.1 200 OK
Content-Type: application/octet-stream

<file content>
```

### 缓存策略

HTTP 模式下启用本地缓存：
- **inode 缓存**: 减少 stat 调用
- **小文件缓存**: < 64KB 的文件内容
- **目录缓存**: 目录列表
- **TTL**: 可配置，默认 300 秒

## 与 WASI 运行时集成

### Wasmtime 集成示例（概念）

```rust
use wasmtime::*;
use wasmtime_wasi::WasiCtxBuilder;
use tarbox_wasi::WasiAdapter;

// 创建 Tarbox 适配器
let tarbox = WasiAdapter::new(fs, tenant_id, config);

// 实现 wasi-common 的 Dir trait
struct TarboxDir(Arc<WasiAdapter<'static>>);

impl WasiDir for TarboxDir {
    fn open_file(
        &self,
        path: &str,
        oflags: OFlags,
    ) -> Result<Box<dyn WasiFile>, Error> {
        let flags = convert_oflags(oflags);
        let fd = self.0.fd_open(path, flags).await?;
        Ok(Box::new(TarboxFile::new(self.0.clone(), fd)))
    }
    
    // ... 其他方法
}

// 注册到 WASI context
let wasi = WasiCtxBuilder::new()
    .preopened_dir(Box::new(TarboxDir(tarbox)), "/workspace")?
    .build();
```

### WasmEdge 集成示例（概念）

```rust
use wasmedge_sdk::*;
use tarbox_wasi::WasiAdapter;

let tarbox = WasiAdapter::new(fs, tenant_id, config);

// 实现 WasmEdge 的 filesystem plugin
let fs_plugin = TarboxFsPlugin::new(tarbox);

let vm = Vm::new(None)?
    .register_plugin(fs_plugin)?;
```

## 当前实现状态

### 已实现

- [x] `WasiAdapter` 基础结构
- [x] `FdTable` 文件描述符管理
- [x] `WasiConfig` 配置系统
- [x] `WasiError` 错误码映射
- [x] `fd_open`, `fd_read`, `fd_write`, `fd_close`
- [x] `OpenFlags` 定义
- [x] HTTP 模式配置（结构）
- [x] 环境变量配置

### 未实现

- [ ] `fd_seek` 完整实现
- [ ] `fd_readdir` 目录遍历
- [ ] `path_rename` 重命名
- [ ] HTTP 模式实际请求
- [ ] 缓存层
- [ ] Wasmtime trait 绑定
- [ ] WasmEdge trait 绑定
- [ ] 完整的 WASI Preview 2 支持

## 性能考虑

### 批量操作

```rust
// 推荐：保持 fd 打开
let fd = adapter.fd_open(path, flags).await?;
for chunk in data.chunks(4096) {
    adapter.fd_write(fd, chunk).await?;
}
adapter.fd_close(fd).await?;

// 不推荐：频繁 open/close
for chunk in data.chunks(4096) {
    let fd = adapter.fd_open(path, flags).await?;
    adapter.fd_write(fd, chunk).await?;
    adapter.fd_close(fd).await?;
}
```

### 缓存命中

- 首次访问：~10ms（数据库往返）
- 缓存命中：~0.1ms
- HTTP 模式首次：~50ms（网络延迟）
- HTTP 模式缓存：~0.1ms

## 测试策略

### 单元测试

```rust
#[tokio::test]
async fn test_fd_open_read() {
    let adapter = create_test_adapter().await;
    
    // 创建测试文件
    adapter.path_create_file("/test.txt", b"hello").await?;
    
    // 打开并读取
    let fd = adapter.fd_open("/test.txt", OpenFlags::READ).await?;
    let mut buf = vec![0u8; 10];
    let n = adapter.fd_read(fd, &mut buf).await?;
    
    assert_eq!(n, 5);
    assert_eq!(&buf[..5], b"hello");
}

#[tokio::test]
async fn test_fd_write_append() {
    let adapter = create_test_adapter().await;
    
    // 写入
    let fd = adapter.fd_open("/test.txt", OpenFlags::WRITE | OpenFlags::CREATE).await?;
    adapter.fd_write(fd, b"hello").await?;
    adapter.fd_close(fd).await?;
    
    // 追加
    let fd = adapter.fd_open("/test.txt", OpenFlags::WRITE | OpenFlags::APPEND).await?;
    adapter.fd_write(fd, b" world").await?;
    adapter.fd_close(fd).await?;
    
    // 验证
    let content = adapter.read_file("/test.txt").await?;
    assert_eq!(content, b"hello world");
}
```

### 集成测试

- 与 Wasmtime 集成测试（需要绑定层）
- HTTP 模式端到端测试
- 多租户隔离测试
- 并发操作测试

## 参考资料

- [WASI filesystem specification](https://github.com/WebAssembly/wasi-filesystem)
- [WASI Preview 2](https://github.com/WebAssembly/WASI/tree/main/wasip2)
- [Wasmtime WASI implementation](https://docs.wasmtime.dev/api/wasmtime_wasi/)
- [WasmEdge plugins](https://wasmedge.org/docs/develop/plugin/)

## 相关规范

- **spec/01**: 数据库 schema - 存储层基础
- **spec/14**: 文件系统接口 - FileSystem API
- **spec/17**: tarbox-wasi crate - crate 整体规划

# WASI 适配器使用指南

本文档介绍如何将 Tarbox 作为 WASI 文件系统后端使用。

---

## 📦 什么是 WASI？

WASI (WebAssembly System Interface) 是 WebAssembly 的系统接口标准，定义了 Wasm 模块如何访问文件系统、网络等系统资源。

## Tarbox 在 WASI 生态中的角色

**Tarbox 不是 WASI 运行时**，而是一个 **WASI 兼容的文件系统后端**。

```
AI Agent (Wasm 模块)
       ↓
WASI 运行时 (Wasmtime/WasmEdge/浏览器)
       ↓ WASI 文件系统调用
Tarbox WasiAdapter (我们提供)
       ↓
Tarbox Filesystem
       ↓
PostgreSQL (持久化存储)
```

### 为什么需要 Tarbox 作为 WASI 文件系统？

当你的 AI Agent 运行在 WASI 环境中时，Tarbox 提供企业级文件系统能力：

- **持久化存储**: 文件保存到 PostgreSQL，而不是内存或临时目录
- **版本控制**: 自动分层快照，可回溯到任意检查点
- **多租户隔离**: 不同 Agent 的数据完全隔离
- **审计追溯**: 所有文件操作自动记录
- **跨环境一致**: 同一个文件系统可在本地、云端、edge 环境使用

---

## 架构概览

### 核心组件

#### 1. WasiAdapter
**位置**: `src/wasi/adapter.rs`

WASI 文件系统适配器，提供 POSIX-like 的文件描述符 API：

```rust
pub struct WasiAdapter<'a> {
    fs: Arc<FileSystem<'a>>,     // Tarbox 文件系统
    tenant_id: Uuid,              // 租户 ID
    fd_table: Arc<Mutex<FdTable>>, // 文件描述符表
    config: WasiConfig,           // 配置
}
```

主要方法：
- `fd_open()` - 打开文件，返回文件描述符
- `fd_read()` - 从 fd 读取数据
- `fd_write()` - 向 fd 写入数据
- `fd_close()` - 关闭 fd
- `fd_seek()` - 改变文件位置
- `path_open()` - 按路径打开文件
- `path_create_directory()` - 创建目录
- `path_remove_directory()` - 删除目录
- `path_unlink_file()` - 删除文件

#### 2. FdTable
**位置**: `src/wasi/fd_table.rs`

管理文件描述符的分配和查找：

```rust
pub struct FdTable {
    next_fd: u32,
    descriptors: HashMap<u32, FileDescriptor>,
}

pub struct FileDescriptor {
    inode_id: i64,
    path: String,
    flags: OpenFlags,
    position: u64,
    is_directory: bool,
}
```

#### 3. WasiConfig
**位置**: `src/wasi/config.rs`

WASI 适配器配置：

```rust
pub struct WasiConfig {
    pub db_mode: DbMode,          // 数据库模式 (Direct/Http)
    pub api_url: Option<String>,  // HTTP API URL (如果使用 Http 模式)
    pub api_key: Option<String>,  // API 密钥
    pub cache_size_mb: usize,     // 缓存大小
    pub cache_ttl_secs: u64,      // 缓存 TTL
    pub tenant_id: Option<Uuid>,  // 默认租户 ID
}
```

环境变量：
- `TARBOX_DB_MODE`: `direct` 或 `http`
- `TARBOX_API_URL`: API 地址
- `TARBOX_API_KEY`: API 密钥
- `TARBOX_CACHE_SIZE`: 缓存大小（MB）

#### 4. WasiError
**位置**: `src/wasi/error.rs`

WASI 标准错误码映射：

```rust
pub enum WasiError {
    NotFound,           // ENOENT
    PermissionDenied,   // EACCES
    AlreadyExists,      // EEXIST
    InvalidInput,       // EINVAL
    IsDirectory,        // EISDIR
    NotDirectory,       // ENOTDIR
    // ... 等等
}
```

---

## 使用示例

### 示例 1：在 Rust WASI 运行时中集成

如果你正在构建一个 WASI 运行时或需要为 Wasm 模块提供文件系统：

```rust
use tarbox::wasi::{WasiAdapter, WasiConfig};
use tarbox::fs::FileSystem;
use tarbox::storage::DatabasePool;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. 初始化 Tarbox 文件系统
    let config = tarbox::config::Config::from_env()?;
    let pool = DatabasePool::new(&config).await?;
    let fs = FileSystem::new(pool);
    
    // 2. 创建 WASI 适配器
    let wasi_config = WasiConfig::default();
    let adapter = WasiAdapter::new(
        Arc::new(fs),
        tenant_id,
        wasi_config,
    );
    
    // 3. 使用适配器进行文件操作
    // 打开文件
    let fd = adapter.fd_open(
        "/workspace/data.txt",
        OpenFlags::READ,
    ).await?;
    
    // 读取数据
    let mut buffer = vec![0u8; 1024];
    let bytes_read = adapter.fd_read(fd, &mut buffer).await?;
    
    // 写入文件
    let fd_write = adapter.fd_open(
        "/workspace/output.txt",
        OpenFlags::WRITE | OpenFlags::CREATE,
    ).await?;
    adapter.fd_write(fd_write, b"Hello from WASI").await?;
    
    // 关闭文件
    adapter.fd_close(fd).await?;
    adapter.fd_close(fd_write).await?;
    
    Ok(())
}
```

### 示例 2：在 Wasmtime 中使用（概念示例）

虽然当前实现没有直接的 Wasmtime 绑定，但展示如何集成的概念：

```rust
// 注意：这是概念代码，需要额外的绑定层
use wasmtime::*;
use wasmtime_wasi::WasiCtxBuilder;

// 创建 Tarbox 适配器
let tarbox_adapter = WasiAdapter::new(fs, tenant_id, config);

// 创建 WASI 上下文（需要自定义绑定）
let wasi_ctx = WasiCtxBuilder::new()
    .inherit_stdio()
    .preopened_dir(
        // 这里需要实现一个桥接层，将 WASI Dir trait 映射到 WasiAdapter
        tarbox_dir_wrapper(tarbox_adapter),
        "/workspace",
    )?
    .build();

// 加载并运行 Wasm 模块
let engine = Engine::default();
let module = Module::from_file(&engine, "agent.wasm")?;
// ... 运行模块
```

### 示例 3：HTTP 模式（Edge 环境）

在无法直接连接数据库的环境（如 Cloudflare Workers）：

```rust
use tarbox::wasi::{WasiAdapter, WasiConfig, DbMode};

// 配置为 HTTP 模式
let wasi_config = WasiConfig {
    db_mode: DbMode::Http,
    api_url: Some("https://tarbox.example.com/api".to_string()),
    api_key: Some("your-api-key".to_string()),
    cache_size_mb: 50,
    cache_ttl_secs: 300,
    tenant_id: Some(tenant_id),
};

let adapter = WasiAdapter::new(fs, tenant_id, wasi_config);

// 文件操作会通过 HTTP API 进行
let fd = adapter.fd_open("/workspace/file.txt", OpenFlags::READ).await?;
```

---

## 文件描述符操作详解

### 打开文件

```rust
use tarbox::wasi::fd_table::OpenFlags;

// 只读
let fd = adapter.fd_open("/path/file.txt", OpenFlags::READ).await?;

// 写入（覆盖）
let fd = adapter.fd_open(
    "/path/file.txt",
    OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::TRUNC,
).await?;

// 追加
let fd = adapter.fd_open(
    "/path/file.txt",
    OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::APPEND,
).await?;
```

### 读写文件

```rust
// 读取
let mut buffer = vec![0u8; 4096];
let n = adapter.fd_read(fd, &mut buffer).await?;
let data = &buffer[..n];

// 写入
let bytes_written = adapter.fd_write(fd, b"data").await?;

// Seek
adapter.fd_seek(fd, 100, SeekFrom::Start).await?;
```

### 目录操作

```rust
// 创建目录
adapter.path_create_directory("/workspace/subdir").await?;

// 删除目录
adapter.path_remove_directory("/workspace/subdir").await?;

// 读取目录（返回 inode 列表）
let entries = adapter.fd_readdir(dir_fd).await?;
```

### 文件元数据

```rust
// 获取文件信息
let stat = adapter.fd_filestat_get(fd).await?;
println!("Size: {}", stat.size);
println!("Type: {:?}", stat.file_type);
println!("Modified: {:?}", stat.mtime);
```

---

## 配置选项

### 数据库模式

#### Direct 模式（默认）
直接连接 PostgreSQL：

```rust
let config = WasiConfig {
    db_mode: DbMode::Direct,
    ..Default::default()
};
```

环境变量需要：
- `DATABASE_URL=postgres://user:pass@host/tarbox`

#### HTTP 模式
通过 HTTP API 访问（适用于 edge 环境）：

```rust
let config = WasiConfig {
    db_mode: DbMode::Http,
    api_url: Some("https://api.tarbox.io".to_string()),
    api_key: Some("key".to_string()),
    ..Default::default()
};
```

环境变量：
- `TARBOX_DB_MODE=http`
- `TARBOX_API_URL=https://api.tarbox.io`
- `TARBOX_API_KEY=your-key`

### 缓存配置

```rust
let config = WasiConfig::default()
    .with_cache_size(200); // 200 MB 缓存

// 或通过环境变量
// TARBOX_CACHE_SIZE=200
```

---

## 权限和安全

### 租户隔离

每个 `WasiAdapter` 实例绑定到一个 `tenant_id`：

```rust
let adapter = WasiAdapter::new(fs, tenant_id, config);
// 所有操作都限定在这个 tenant_id 下
```

- ✅ 不同租户的数据完全隔离
- ✅ 数据库层面强制隔离
- ✅ 无法跨租户访问文件

### 路径限制

- ✅ 所有路径必须是绝对路径（以 `/` 开头）
- ✅ 不允许 `..` 逃逸到父目录
- ✅ 符号链接受限（防止逃逸）

### 文件权限

当前实现简化的权限模型：
- 通过 `OpenFlags` 控制读写权限
- 不支持 Unix UID/GID
- 不支持文件 mode bits (chmod)

---

## 性能优化

### 缓存策略

```rust
let config = WasiConfig {
    cache_size_mb: 100,      // 缓存大小
    cache_ttl_secs: 300,     // 5 分钟 TTL
    ..Default::default()
};
```

缓存内容：
- Inode 元数据
- 目录项
- 小文件内容

### 批量操作

```rust
// ❌ 不推荐：逐个文件操作
for path in paths {
    let fd = adapter.fd_open(path, OpenFlags::READ).await?;
    // ...
    adapter.fd_close(fd).await?;
}

// ✅ 推荐：保持 fd 打开，减少 open/close 开销
let fds: Vec<_> = futures::future::try_join_all(
    paths.iter().map(|p| adapter.fd_open(p, OpenFlags::READ))
).await?;

// 并行读取
// ...

for fd in fds {
    adapter.fd_close(fd).await?;
}
```

---

## 限制和注意事项

### 当前实现状态

- ✅ 文件描述符操作（open, read, write, close, seek）
- ✅ 目录操作（create, remove, readdir）
- ✅ 文件元数据（stat, filestat）
- ✅ 多租户隔离
- ✅ HTTP API 模式
- ❌ 完整的 WASI Preview 2 绑定（需要额外实现）
- ❌ 直接的 Wasmtime 集成（需要绑定层）
- ❌ 文件锁（flock）
- ❌ 文件权限 (chmod/chown)

### 已知限制

- **不是完整的 WASI 运行时**: 只提供文件系统适配器
- **需要异步运行时**: 所有操作都是 async，需要 tokio
- **不支持硬链接**: Tarbox 设计限制
- **符号链接支持有限**: 防止目录逃逸

### 与标准 WASI 的差异

Tarbox WasiAdapter 是底层适配器，不是标准 WASI 接口的直接实现。要在标准 WASI 运行时中使用，需要额外的绑定层。

---

## 故障排查

### 无法打开文件

**问题**: `fd_open()` 返回 `WasiError::NotFound`

**排查**:
```rust
// 1. 检查路径是否正确（必须是绝对路径）
let fd = adapter.fd_open("/workspace/file.txt", flags).await?;
// 不能是 "workspace/file.txt" 或 "./file.txt"

// 2. 检查租户 ID 是否正确
println!("Tenant ID: {}", adapter.tenant_id());

// 3. 检查文件是否存在
let stat = fs.stat("/workspace/file.txt").await;
```

### 权限被拒绝

**问题**: `fd_write()` 返回 `WasiError::PermissionDenied`

**原因**: 文件打开时没有 WRITE 标志

**解决**:
```rust
// ❌ 错误
let fd = adapter.fd_open(path, OpenFlags::READ).await?;
adapter.fd_write(fd, data).await?; // 错误！

// ✅ 正确
let fd = adapter.fd_open(path, OpenFlags::WRITE).await?;
adapter.fd_write(fd, data).await?;
```

### HTTP 模式连接失败

**问题**: 使用 HTTP 模式时操作超时

**排查**:
```rust
// 检查配置
let config = WasiConfig::from_env();
println!("DB Mode: {:?}", config.db_mode);
println!("API URL: {:?}", config.api_url);

// 检查网络连接
// curl https://api.tarbox.io/health
```

---

## 开发指南

### 实现自定义 WASI 绑定

如果你需要将 Tarbox 集成到特定的 WASI 运行时：

```rust
// 1. 实现 WASI filesystem trait
use wasi_common::dir::DirCaps;
use wasi_common::file::{FileCaps, File};

struct TarboxFile {
    adapter: Arc<WasiAdapter<'static>>,
    fd: u32,
}

#[async_trait::async_trait]
impl File for TarboxFile {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        self.adapter.fd_read(self.fd, buf)
            .await
            .map_err(|e| Error::from(e))
    }
    
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        self.adapter.fd_write(self.fd, buf)
            .await
            .map_err(|e| Error::from(e))
    }
    
    // ... 实现其他方法
}

// 2. 创建 WASI 上下文
let wasi_ctx = WasiCtxBuilder::new()
    .preopened_dir(Box::new(TarboxDir::new(adapter)), "/workspace")?
    .build();
```

---

## 相关文档

- [Tarbox 文件系统核心](../spec/02-fuse-interface.md)
- [架构设计总览](../spec/00-overview.md)
- [数据库 Schema](../spec/01-database-schema.md)

---

## 外部资源

- [WASI 规范](https://github.com/WebAssembly/WASI)
- [Wasmtime 文档](https://docs.wasmtime.dev/)
- [WASI filesystem API](https://github.com/WebAssembly/wasi-filesystem)

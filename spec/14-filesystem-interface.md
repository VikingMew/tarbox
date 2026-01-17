# Spec 14: 文件系统接口抽象层

**优先级**: P0 (核心架构)  
**状态**: 设计阶段  
**依赖**: spec/01 (数据库 Schema)

## 概述

定义统一的文件系统接口抽象层，为 FUSE、Kubernetes CSI、WASI 等不同接口提供共同的实现基础。通过这个抽象层，所有接口适配器都可以共享相同的后端实现，避免代码重复，简化维护。

## 设计理念

### 问题背景

Tarbox 需要支持多种接口：
- **FUSE**: 本地文件系统挂载（POSIX 接口）
- **Kubernetes CSI**: 容器存储接口
- **WASI**: WebAssembly 文件系统接口
- **CLI**: 命令行工具（已实现）
- **未来可能**: HTTP FS, 9P, NFS 等

这些接口虽然协议不同，但本质上都在做同一件事：**提供文件系统操作能力**。

### 设计目标

1. **统一抽象**: 定义清晰的文件系统操作接口
2. **代码复用**: 避免在每个适配器中重复实现相同逻辑
3. **易于测试**: 可以 mock 接口进行单元测试
4. **易于扩展**: 添加新接口只需实现适配器层
5. **类型安全**: 利用 Rust 类型系统保证正确性

## 架构设计

### 三层架构

```
┌──────────────────────────────────────────────────────┐
│                   应用层                              │
│  FUSE Client │ K8s Pod │ WASM Runtime │ CLI Tool     │
└──────┬───────┴────┬────┴──────┬────────┴──────┬──────┘
       │            │           │               │
┌──────▼────────────▼───────────▼───────────────▼──────┐
│                  接口适配器层                         │
│  FuseAdapter │ CsiAdapter │ WasiAdapter │ CliAdapter │
│  (spec/02)   │ (spec/05)  │ (spec/13)   │ (Task 04)  │
└──────┬────────────┬───────────┬───────────────┬──────┘
       │            │           │               │
       └────────────┴───────────┴───────────────┘
                         │
┌────────────────────────▼─────────────────────────────┐
│          文件系统接口抽象层 (本规范)                  │
│         FilesystemInterface Trait                    │
└────────────────────────┬─────────────────────────────┘
                         │
┌────────────────────────▼─────────────────────────────┐
│               Tarbox Core Backend                    │
│    FileSystem (fs/) + Storage (storage/)             │
│           + Layer (layer/) + Cache                   │
└──────────────────────────────────────────────────────┘
```

### 职责划分

**接口适配器层** (FUSE/CSI/WASI Adapters):
- 协议特定的序列化/反序列化
- 协议错误码转换
- 异步/同步模型桥接
- 协议特定的优化

**接口抽象层** (FilesystemInterface):
- 定义标准化的文件系统操作
- 统一的错误类型
- 统一的数据类型 (FileAttr, DirEntry)
- 异步操作模型

**后端实现层** (Tarbox Core):
- 实际的文件系统逻辑
- 数据库交互
- 分层文件系统
- 缓存管理

## 核心接口定义

### FilesystemInterface Trait

```rust
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// 文件系统接口抽象
/// 
/// 所有接口适配器（FUSE, CSI, WASI）都通过实现此 trait 来访问文件系统。
#[async_trait]
pub trait FilesystemInterface: Send + Sync {
    // ==================== 文件操作 ====================
    
    /// 读取文件内容
    /// 
    /// # 参数
    /// - `path`: 文件路径（绝对路径）
    /// 
    /// # 返回
    /// - `Ok(Vec<u8>)`: 文件内容
    /// - `Err(FsError)`: 路径不存在、不是文件、权限不足等
    async fn read_file(&self, path: &str) -> FsResult<Vec<u8>>;
    
    /// 写入文件内容（覆盖模式）
    /// 
    /// # 参数
    /// - `path`: 文件路径
    /// - `data`: 要写入的数据
    async fn write_file(&self, path: &str, data: &[u8]) -> FsResult<()>;
    
    /// 创建空文件
    /// 
    /// # 参数
    /// - `path`: 文件路径
    /// 
    /// # 返回
    /// - `Ok(FileAttr)`: 新创建文件的属性
    async fn create_file(&self, path: &str) -> FsResult<FileAttr>;
    
    /// 删除文件
    /// 
    /// # 参数
    /// - `path`: 文件路径
    async fn delete_file(&self, path: &str) -> FsResult<()>;
    
    /// 截断文件到指定大小
    /// 
    /// # 参数
    /// - `path`: 文件路径
    /// - `size`: 新的文件大小
    async fn truncate(&self, path: &str, size: u64) -> FsResult<()>;
    
    // ==================== 目录操作 ====================
    
    /// 创建目录
    /// 
    /// # 参数
    /// - `path`: 目录路径
    /// 
    /// # 返回
    /// - `Ok(FileAttr)`: 新创建目录的属性
    async fn create_dir(&self, path: &str) -> FsResult<FileAttr>;
    
    /// 读取目录内容
    /// 
    /// # 参数
    /// - `path`: 目录路径
    /// 
    /// # 返回
    /// - `Ok(Vec<DirEntry>)`: 目录项列表
    async fn read_dir(&self, path: &str) -> FsResult<Vec<DirEntry>>;
    
    /// 删除空目录
    /// 
    /// # 参数
    /// - `path`: 目录路径
    /// 
    /// # 错误
    /// - 如果目录非空，返回 DirectoryNotEmpty 错误
    async fn remove_dir(&self, path: &str) -> FsResult<()>;
    
    // ==================== 元数据操作 ====================
    
    /// 获取文件/目录属性
    /// 
    /// # 参数
    /// - `path`: 文件或目录路径
    /// 
    /// # 返回
    /// - `Ok(FileAttr)`: 文件属性
    async fn get_attr(&self, path: &str) -> FsResult<FileAttr>;
    
    /// 设置文件/目录属性
    /// 
    /// # 参数
    /// - `path`: 文件或目录路径
    /// - `attr`: 要设置的属性
    async fn set_attr(&self, path: &str, attr: SetAttr) -> FsResult<FileAttr>;
    
    /// 修改权限
    /// 
    /// # 参数
    /// - `path`: 文件或目录路径
    /// - `mode`: 新的权限模式（UNIX 风格）
    async fn chmod(&self, path: &str, mode: u32) -> FsResult<()>;
    
    /// 修改所有者
    /// 
    /// # 参数
    /// - `path`: 文件或目录路径
    /// - `uid`: 用户 ID
    /// - `gid`: 组 ID
    async fn chown(&self, path: &str, uid: u32, gid: u32) -> FsResult<()>;
    
    /// 更新访问和修改时间
    /// 
    /// # 参数
    /// - `path`: 文件或目录路径
    /// - `atime`: 访问时间（None 表示不修改）
    /// - `mtime`: 修改时间（None 表示不修改）
    async fn utimens(
        &self,
        path: &str,
        atime: Option<DateTime<Utc>>,
        mtime: Option<DateTime<Utc>>,
    ) -> FsResult<()>;
    
    // ==================== 链接操作（可选）====================
    
    /// 创建符号链接
    /// 
    /// # 参数
    /// - `target`: 目标路径
    /// - `link`: 链接路径
    async fn create_symlink(&self, target: &str, link: &str) -> FsResult<FileAttr> {
        Err(FsError::NotSupported("Symlink not supported".to_string()))
    }
    
    /// 读取符号链接
    /// 
    /// # 参数
    /// - `path`: 链接路径
    /// 
    /// # 返回
    /// - `Ok(String)`: 目标路径
    async fn read_symlink(&self, path: &str) -> FsResult<String> {
        Err(FsError::NotSupported("Symlink not supported".to_string()))
    }
    
    /// 创建硬链接
    /// 
    /// # 参数
    /// - `target`: 目标路径
    /// - `link`: 链接路径
    async fn create_hardlink(&self, target: &str, link: &str) -> FsResult<()> {
        Err(FsError::NotSupported("Hardlink not supported".to_string()))
    }
    
    // ==================== 扩展属性（可选）====================
    
    /// 设置扩展属性
    async fn setxattr(&self, path: &str, name: &str, value: &[u8]) -> FsResult<()> {
        Err(FsError::NotSupported("Extended attributes not supported".to_string()))
    }
    
    /// 获取扩展属性
    async fn getxattr(&self, path: &str, name: &str) -> FsResult<Vec<u8>> {
        Err(FsError::NotSupported("Extended attributes not supported".to_string()))
    }
    
    /// 列出扩展属性
    async fn listxattr(&self, path: &str) -> FsResult<Vec<String>> {
        Err(FsError::NotSupported("Extended attributes not supported".to_string()))
    }
    
    /// 删除扩展属性
    async fn removexattr(&self, path: &str, name: &str) -> FsResult<()> {
        Err(FsError::NotSupported("Extended attributes not supported".to_string()))
    }
}
```

## 数据类型定义

### FileAttr - 文件属性

```rust
/// 文件/目录属性
#[derive(Debug, Clone)]
pub struct FileAttr {
    /// Inode ID
    pub inode: u64,
    
    /// 文件类型
    pub kind: FileType,
    
    /// 文件大小（字节）
    pub size: u64,
    
    /// 块数量（用于 du 命令）
    pub blocks: u64,
    
    /// 访问时间
    pub atime: DateTime<Utc>,
    
    /// 修改时间
    pub mtime: DateTime<Utc>,
    
    /// 状态改变时间
    pub ctime: DateTime<Utc>,
    
    /// 创建时间（可选）
    pub crtime: Option<DateTime<Utc>>,
    
    /// 权限模式（UNIX 风格）
    pub mode: u32,
    
    /// 硬链接数
    pub nlink: u32,
    
    /// 用户 ID
    pub uid: u32,
    
    /// 组 ID
    pub gid: u32,
    
    /// 设备 ID（对于设备文件）
    pub rdev: u32,
    
    /// 块大小
    pub blksize: u32,
}

/// 文件类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    /// 普通文件
    RegularFile,
    
    /// 目录
    Directory,
    
    /// 符号链接
    Symlink,
    
    /// 块设备
    BlockDevice,
    
    /// 字符设备
    CharDevice,
    
    /// FIFO (命名管道)
    Fifo,
    
    /// Socket
    Socket,
}
```

### DirEntry - 目录项

```rust
/// 目录项
#[derive(Debug, Clone)]
pub struct DirEntry {
    /// Inode ID
    pub inode: u64,
    
    /// 文件名
    pub name: String,
    
    /// 文件类型
    pub kind: FileType,
}
```

### SetAttr - 属性设置

```rust
/// 要设置的属性
/// 
/// 使用 Option 表示只设置提供的字段，None 表示不修改
#[derive(Debug, Default)]
pub struct SetAttr {
    /// 文件大小
    pub size: Option<u64>,
    
    /// 访问时间
    pub atime: Option<DateTime<Utc>>,
    
    /// 修改时间
    pub mtime: Option<DateTime<Utc>>,
    
    /// 权限模式
    pub mode: Option<u32>,
    
    /// 用户 ID
    pub uid: Option<u32>,
    
    /// 组 ID
    pub gid: Option<u32>,
}
```

## 错误类型

### FsError 枚举

```rust
/// 文件系统错误类型
#[derive(Debug, thiserror::Error)]
pub enum FsError {
    /// 路径不存在
    #[error("Path not found: {0}")]
    PathNotFound(String),
    
    /// 已存在
    #[error("Already exists: {0}")]
    AlreadyExists(String),
    
    /// 不是目录
    #[error("Not a directory: {0}")]
    NotDirectory(String),
    
    /// 是目录
    #[error("Is a directory: {0}")]
    IsDirectory(String),
    
    /// 目录非空
    #[error("Directory not empty: {0}")]
    DirectoryNotEmpty(String),
    
    /// 无效路径
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    
    /// 路径太长
    #[error("Path too long: {0} bytes")]
    PathTooLong(usize),
    
    /// 文件名太长
    #[error("Filename too long: {0} bytes")]
    FilenameTooLong(usize),
    
    /// 权限不足
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    
    /// 不支持的操作
    #[error("Not supported: {0}")]
    NotSupported(String),
    
    /// I/O 错误
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    /// 数据库错误
    #[error("Database error: {0}")]
    Database(#[from] anyhow::Error),
    
    /// 其他错误
    #[error("{0}")]
    Other(String),
}

pub type FsResult<T> = Result<T, FsError>;
```

### 错误码映射

不同接口需要将 `FsError` 映射到各自的错误码：

| FsError | FUSE (errno) | WASI | HTTP Status |
|---------|-------------|------|-------------|
| PathNotFound | ENOENT (2) | ENOENT | 404 Not Found |
| AlreadyExists | EEXIST (17) | EEXIST | 409 Conflict |
| NotDirectory | ENOTDIR (20) | ENOTDIR | 400 Bad Request |
| IsDirectory | EISDIR (21) | EISDIR | 400 Bad Request |
| DirectoryNotEmpty | ENOTEMPTY (39) | ENOTEMPTY | 409 Conflict |
| InvalidPath | EINVAL (22) | EINVAL | 400 Bad Request |
| PermissionDenied | EACCES (13) | EACCES | 403 Forbidden |
| NotSupported | ENOSYS (38) | ENOSYS | 501 Not Implemented |

## 默认实现

### TarboxBackend

```rust
/// Tarbox 后端实现
/// 
/// 将 FilesystemInterface 映射到 Tarbox 核心文件系统
pub struct TarboxBackend {
    fs: FileSystem,
}

impl TarboxBackend {
    pub fn new(pool: &PgPool, tenant_id: TenantId) -> Self {
        Self {
            fs: FileSystem::new(pool, tenant_id),
        }
    }
}

#[async_trait]
impl FilesystemInterface for TarboxBackend {
    async fn read_file(&self, path: &str) -> FsResult<Vec<u8>> {
        self.fs.read_file(path).await
    }
    
    async fn write_file(&self, path: &str, data: &[u8]) -> FsResult<()> {
        self.fs.write_file(path, data).await
    }
    
    async fn create_file(&self, path: &str) -> FsResult<FileAttr> {
        let inode = self.fs.create_file(path).await?;
        Ok(inode_to_attr(inode))
    }
    
    async fn delete_file(&self, path: &str) -> FsResult<()> {
        self.fs.delete_file(path).await
    }
    
    async fn create_dir(&self, path: &str) -> FsResult<FileAttr> {
        let inode = self.fs.create_directory(path).await?;
        Ok(inode_to_attr(inode))
    }
    
    async fn read_dir(&self, path: &str) -> FsResult<Vec<DirEntry>> {
        let inodes = self.fs.list_directory(path).await?;
        Ok(inodes.into_iter().map(inode_to_entry).collect())
    }
    
    async fn remove_dir(&self, path: &str) -> FsResult<()> {
        self.fs.remove_directory(path).await
    }
    
    async fn get_attr(&self, path: &str) -> FsResult<FileAttr> {
        let inode = self.fs.stat(path).await?;
        Ok(inode_to_attr(inode))
    }
    
    async fn set_attr(&self, path: &str, attr: SetAttr) -> FsResult<FileAttr> {
        // 实现属性设置逻辑
        todo!()
    }
    
    async fn chmod(&self, path: &str, mode: u32) -> FsResult<()> {
        self.fs.chmod(path, mode).await
    }
    
    async fn chown(&self, path: &str, uid: u32, gid: u32) -> FsResult<()> {
        self.fs.chown(path, uid, gid).await
    }
    
    async fn truncate(&self, path: &str, size: u64) -> FsResult<()> {
        // 实现截断逻辑
        todo!()
    }
    
    async fn utimens(
        &self,
        path: &str,
        atime: Option<DateTime<Utc>>,
        mtime: Option<DateTime<Utc>>,
    ) -> FsResult<()> {
        // 实现时间戳更新逻辑
        todo!()
    }
}

// 辅助函数：Inode -> FileAttr 转换
fn inode_to_attr(inode: Inode) -> FileAttr {
    FileAttr {
        inode: inode.inode_id as u64,
        kind: match inode.inode_type {
            InodeType::File => FileType::RegularFile,
            InodeType::Dir => FileType::Directory,
            InodeType::Symlink => FileType::Symlink,
        },
        size: inode.size as u64,
        blocks: (inode.size as u64 + 4095) / 4096,
        atime: inode.atime,
        mtime: inode.mtime,
        ctime: inode.ctime,
        crtime: Some(inode.created_at),
        mode: inode.mode as u32,
        nlink: 1,
        uid: inode.uid as u32,
        gid: inode.gid as u32,
        rdev: 0,
        blksize: 4096,
    }
}

// 辅助函数：Inode -> DirEntry 转换
fn inode_to_entry(inode: Inode) -> DirEntry {
    DirEntry {
        inode: inode.inode_id as u64,
        name: inode.name,
        kind: match inode.inode_type {
            InodeType::File => FileType::RegularFile,
            InodeType::Dir => FileType::Directory,
            InodeType::Symlink => FileType::Symlink,
        },
    }
}
```

## 适配器实现示例

### FUSE 适配器

```rust
use fuser::{Filesystem, Request, ReplyAttr, ReplyData, ReplyEntry, ReplyDirectory};

pub struct FuseAdapter {
    backend: Arc<dyn FilesystemInterface>,
    runtime: tokio::runtime::Runtime,
}

impl Filesystem for FuseAdapter {
    fn read(
        &mut self,
        _req: &Request,
        ino: u64,
        fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock: Option<u64>,
        reply: ReplyData,
    ) {
        // 通过 inode 查找路径
        let path = self.ino_to_path(ino);
        
        // 异步调用转同步
        let result = self.runtime.block_on(async {
            self.backend.read_file(&path).await
        });
        
        match result {
            Ok(data) => {
                let end = (offset + size as i64).min(data.len() as i64);
                reply.data(&data[offset as usize..end as usize]);
            }
            Err(e) => reply.error(fs_error_to_errno(e)),
        }
    }
    
    // ... 其他 FUSE 方法
}

// FsError -> errno 映射
fn fs_error_to_errno(err: FsError) -> i32 {
    match err {
        FsError::PathNotFound(_) => libc::ENOENT,
        FsError::AlreadyExists(_) => libc::EEXIST,
        FsError::NotDirectory(_) => libc::ENOTDIR,
        FsError::IsDirectory(_) => libc::EISDIR,
        FsError::DirectoryNotEmpty(_) => libc::ENOTEMPTY,
        FsError::InvalidPath(_) => libc::EINVAL,
        FsError::PermissionDenied(_) => libc::EACCES,
        FsError::NotSupported(_) => libc::ENOSYS,
        _ => libc::EIO,
    }
}
```

### WASI 适配器

```rust
use wasi::filesystem::*;

pub struct WasiAdapter {
    backend: Arc<dyn FilesystemInterface>,
}

impl WasiAdapter {
    pub async fn read_file(&self, path: String) -> Result<Vec<u8>, WasiError> {
        self.backend
            .read_file(&path)
            .await
            .map_err(fs_error_to_wasi)
    }
    
    // ... 其他 WASI 方法
}

// FsError -> WASI Error 映射
fn fs_error_to_wasi(err: FsError) -> WasiError {
    match err {
        FsError::PathNotFound(_) => WasiError::ENOENT,
        FsError::AlreadyExists(_) => WasiError::EEXIST,
        FsError::NotDirectory(_) => WasiError::ENOTDIR,
        // ...
        _ => WasiError::EIO,
    }
}
```

## 测试策略

### Mock 实现

```rust
pub struct MockFilesystem {
    files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

#[async_trait]
impl FilesystemInterface for MockFilesystem {
    async fn read_file(&self, path: &str) -> FsResult<Vec<u8>> {
        let files = self.files.lock().unwrap();
        files
            .get(path)
            .cloned()
            .ok_or_else(|| FsError::PathNotFound(path.to_string()))
    }
    
    async fn write_file(&self, path: &str, data: &[u8]) -> FsResult<()> {
        let mut files = self.files.lock().unwrap();
        files.insert(path.to_string(), data.to_vec());
        Ok(())
    }
    
    // ... 其他方法的 mock 实现
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_fuse_adapter_with_mock() {
        let mock_fs = Arc::new(MockFilesystem::new());
        let adapter = FuseAdapter::new(mock_fs.clone());
        
        // 测试 FUSE 操作
        // ...
    }
}
```

## 实现路线图

### Phase 1: 基础接口定义 (当前)
- [x] 定义 `FilesystemInterface` trait
- [x] 定义数据类型 (`FileAttr`, `DirEntry`, `SetAttr`)
- [x] 定义错误类型 (`FsError`)
- [ ] 实现 `TarboxBackend`

### Phase 2: FUSE 适配器 (Task 05)
- [ ] 实现 `FuseAdapter`
- [ ] 异步/同步桥接
- [ ] 错误码映射
- [ ] 集成测试

### Phase 3: CSI 适配器 (Task 12)
- [ ] 实现 `CsiAdapter`
- [ ] Volume 生命周期管理
- [ ] 快照支持
- [ ] Kubernetes 集成测试

### Phase 4: WASI 适配器 (Task 15)
- [ ] 实现 `WasiAdapter`
- [ ] WASI Preview 2 支持
- [ ] WebAssembly 运行时集成
- [ ] 浏览器测试

## 与其他 Spec 的关系

- **spec/01**: 数据库 Schema - 后端实现的数据层
- **spec/02**: FUSE 接口 - 基于本接口的 FUSE 适配器
- **spec/04**: 分层文件系统 - 后端实现的功能层
- **spec/05**: Kubernetes CSI - 基于本接口的 CSI 适配器
- **spec/13**: WASI 接口 - 基于本接口的 WASI 适配器

## 决策记录

### DR-14-1: 使用 async_trait 而非原生 async

**原因**:
- Rust 原生 trait 的 async 方法支持还不完善
- `async_trait` 是成熟的解决方案
- 性能开销可接受（虚拟调用本身就有开销）

### DR-14-2: 可选方法使用默认实现返回 NotSupported

**原因**:
- 不是所有接口都需要所有功能
- MVP 可以先实现核心方法
- 渐进式实现，不blocking

### DR-14-3: 使用 thiserror 定义错误类型

**原因**:
- 遵循项目的 fail-fast 原则
- 类型安全的错误处理
- 清晰的错误信息

## 参考资料

- [FUSE Low-Level API](https://libfuse.github.io/doxygen/)
- [Kubernetes CSI Specification](https://github.com/container-storage-interface/spec)
- [WASI Filesystem Specification](https://github.com/WebAssembly/WASI/blob/main/preview2/README.md)
- [Rust Async Trait](https://rust-lang.github.io/async-book/)

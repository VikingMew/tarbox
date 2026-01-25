# Spec 17: tarbox-wasi Crate 规划

**优先级**: P2  
**状态**: 规划中  
**依赖**: spec/13 (WASI 适配器技术设计)

## 概述

本规范描述 `tarbox-wasi` crate 的整体规划，包括发布策略、API 设计、依赖管理和使用场景。

## 定位

`tarbox-wasi` 是一个发布到 crates.io 的独立 Rust crate，让其他 WASI 运行时可以使用 Tarbox 作为文件系统后端。

### 核心价值

| 对用户的价值 | 说明 |
|-------------|------|
| **持久化存储** | 文件保存到 PostgreSQL，不丢失 |
| **版本控制** | 分层快照，可回溯任意状态 |
| **多租户隔离** | 不同 Agent 数据完全隔离 |
| **审计追溯** | 所有操作自动记录 |
| **企业级可靠性** | PostgreSQL ACID 保证 |

### 不是什么

- ❌ **不是 WASI 运行时** - 不执行 Wasm 代码
- ❌ **不是独立应用** - 需要集成到运行时中使用
- ❌ **不是轻量级方案** - 需要 PostgreSQL

## Crate 结构

### Workspace 组织

`tarbox-wasi` 作为同一仓库的 workspace member，不是独立仓库：

```
tarbox/                          # 仓库根目录
├── Cargo.toml                   # workspace 根配置
├── src/                         # 主应用源码
│   ├── main.rs                  # CLI 入口
│   ├── fuse/                    # FUSE 实现
│   ├── csi/                     # CSI 实现
│   ├── fs/                      # 文件系统核心
│   ├── storage/                 # 存储层
│   └── wasi/                    # WASI 适配器（当前位置）
├── crates/                      # 可发布的 crates
│   └── tarbox-wasi/             # WASI crate（发布到 crates.io）
│       ├── Cargo.toml
│       ├── src/
│       │   └── lib.rs           # 重导出 tarbox::wasi
│       ├── README.md
│       └── examples/
└── ...
```

### Workspace Cargo.toml

```toml
# 根 Cargo.toml
[workspace]
members = [
    ".",              # tarbox 主应用
    "crates/tarbox-wasi",  # WASI crate
]
resolver = "2"

[workspace.package]
edition = "2024"
rust-version = "1.92"
license = "MPL-2.0"
repository = "https://github.com/vikingmew/tarbox"
```

### tarbox-wasi Crate 结构

```toml
# crates/tarbox-wasi/Cargo.toml
[package]
name = "tarbox-wasi"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "WASI filesystem adapter backed by PostgreSQL"
documentation = "https://docs.rs/tarbox-wasi"
keywords = ["wasi", "filesystem", "postgresql", "wasm", "ai"]
categories = ["filesystem", "wasm", "database"]

[dependencies]
# 依赖主项目的模块
tarbox = { path = "../..", default-features = false, features = ["wasi"] }
```

### 代码组织方式

**方式 A：重导出（推荐）**

`tarbox-wasi` 简单重导出主项目的 wasi 模块：

```rust
// crates/tarbox-wasi/src/lib.rs
//! WASI filesystem adapter backed by PostgreSQL.
//!
//! This crate provides a WASI-compatible filesystem interface that stores
//! files in PostgreSQL. See [tarbox](https://github.com/vikingmew/tarbox) for
//! the full project.

pub use tarbox::wasi::*;
```

**方式 B：独立实现**

如果需要减少依赖，可以将 wasi 模块代码移动到 crate 中：

```rust
// crates/tarbox-wasi/src/lib.rs
mod adapter;
mod config;
mod error;
mod fd_table;

pub use adapter::WasiAdapter;
pub use config::{DbMode, WasiConfig};
pub use error::{WasiError, to_wasi_errno};
pub use fd_table::{FdTable, FileDescriptor, OpenFlags};
```

**推荐方式 A**：保持代码在主项目中，crate 只做重导出，避免代码重复。

### Feature Flags

```toml
# 主项目 Cargo.toml
[features]
default = ["fuse", "csi"]
fuse = ["fuser"]
csi = ["tonic", "prost"]
wasi = []  # WASI 适配器，默认编译但不包含额外依赖

# crates/tarbox-wasi/Cargo.toml
[features]
default = []
http = ["tarbox/wasi-http"]  # HTTP 模式
```

## API 设计

### 公开 API

```rust
// tarbox-wasi/src/lib.rs

/// WASI 文件系统适配器
pub struct WasiAdapter<'a> { /* ... */ }

impl<'a> WasiAdapter<'a> {
    /// 创建新的适配器
    pub fn new(
        fs: Arc<FileSystem<'a>>,
        tenant_id: Uuid,
        config: WasiConfig,
    ) -> Self;
    
    /// 获取租户 ID
    pub fn tenant_id(&self) -> Uuid;
    
    /// 获取配置
    pub fn config(&self) -> &WasiConfig;
    
    // === 文件描述符操作 ===
    
    /// 打开文件
    pub async fn fd_open(&self, path: &str, flags: OpenFlags) -> Result<u32, WasiError>;
    
    /// 读取数据
    pub async fn fd_read(&self, fd: u32, buf: &mut [u8]) -> Result<usize, WasiError>;
    
    /// 写入数据
    pub async fn fd_write(&self, fd: u32, buf: &[u8]) -> Result<usize, WasiError>;
    
    /// 关闭文件
    pub async fn fd_close(&self, fd: u32) -> Result<(), WasiError>;
    
    /// 移动文件位置
    pub async fn fd_seek(&self, fd: u32, offset: i64, whence: SeekFrom) -> Result<u64, WasiError>;
    
    /// 获取文件状态
    pub async fn fd_filestat_get(&self, fd: u32) -> Result<FileStat, WasiError>;
    
    /// 读取目录
    pub async fn fd_readdir(&self, fd: u32) -> Result<Vec<DirEntry>, WasiError>;
    
    // === 路径操作 ===
    
    /// 创建目录
    pub async fn path_create_directory(&self, path: &str) -> Result<(), WasiError>;
    
    /// 删除目录
    pub async fn path_remove_directory(&self, path: &str) -> Result<(), WasiError>;
    
    /// 删除文件
    pub async fn path_unlink_file(&self, path: &str) -> Result<(), WasiError>;
    
    /// 重命名
    pub async fn path_rename(&self, old: &str, new: &str) -> Result<(), WasiError>;
    
    /// 获取文件状态（通过路径）
    pub async fn path_filestat_get(&self, path: &str) -> Result<FileStat, WasiError>;
}

/// 文件打开标志
pub struct OpenFlags { /* ... */ }

/// 配置
pub struct WasiConfig { /* ... */ }

/// 错误类型
pub enum WasiError { /* ... */ }

/// 文件状态
pub struct FileStat { /* ... */ }

/// 目录项
pub struct DirEntry { /* ... */ }

/// 文件描述符表（可选公开，用于高级集成）
pub struct FdTable { /* ... */ }

/// 文件描述符
pub struct FileDescriptor { /* ... */ }
```

### 版本稳定性

遵循 SemVer：
- **0.x.y**: 初始开发，API 可能变化
- **1.0.0**: 稳定 API，向后兼容
- **1.x.y**: 小版本添加功能，补丁版本修复 bug

### API 稳定性承诺

| API | 稳定性 |
|-----|--------|
| `WasiAdapter::new` | 稳定 |
| `fd_*` 方法 | 稳定 |
| `path_*` 方法 | 稳定 |
| `WasiConfig` | 可能扩展字段 |
| `WasiError` | 可能添加变体 |
| `FdTable` | 不稳定（内部实现可能变化） |

## 依赖管理

### Crate 依赖

由于 `tarbox-wasi` 通过重导出主项目模块，它的依赖非常简单：

```toml
# crates/tarbox-wasi/Cargo.toml
[package]
name = "tarbox-wasi"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
description = "WASI filesystem adapter backed by PostgreSQL"
documentation = "https://docs.rs/tarbox-wasi"
readme = "README.md"
keywords = ["wasi", "filesystem", "postgresql", "wasm", "ai"]
categories = ["filesystem", "wasm", "database"]

[dependencies]
tarbox = { path = "../..", default-features = false, features = ["wasi"] }

[features]
default = []
http = ["tarbox/wasi-http"]

[dev-dependencies]
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

### 传递依赖

实际依赖通过 `tarbox` 主项目传递：

| 依赖 | 来源 | 说明 |
|------|------|------|
| tokio | tarbox | 异步运行时 |
| sqlx | tarbox | PostgreSQL 连接 |
| uuid | tarbox | 租户 ID、inode ID |
| anyhow | tarbox | 错误处理 |
| reqwest | tarbox (可选) | HTTP 模式 |

### 主项目 Feature 配置

```toml
# 根 Cargo.toml
[features]
default = ["fuse", "csi", "wasi"]
fuse = ["fuser"]
csi = ["tonic", "prost"]
wasi = []              # WASI 适配器核心
wasi-http = ["reqwest"] # WASI HTTP 模式
```

### 关于 PostgreSQL 依赖

**为什么保留 PostgreSQL 依赖？**

1. **核心价值**: Tarbox 的价值就是 PostgreSQL 存储
2. **多租户**: 需要数据库级别的隔离
3. **分层存储**: 需要关系型存储支持
4. **审计日志**: 需要持久化存储

**不提供纯内存版本**：
- 内存版本不是 Tarbox 的定位
- 如果用户只需要内存文件系统，应该用其他方案

## 使用场景

### 场景 1: AI Agent 运行时

```rust
use tarbox_wasi::{WasiAdapter, WasiConfig};
use wasmtime::*;

// AI 平台提供多租户 Agent 执行环境
async fn run_agent(agent_wasm: &[u8], tenant_id: Uuid) -> Result<()> {
    // 为每个 Agent 创建独立的 Tarbox 文件系统
    let fs = create_filesystem().await?;
    let adapter = WasiAdapter::new(fs, tenant_id, WasiConfig::default());
    
    // 集成到 Wasmtime
    let tarbox_dir = TarboxDir::new(adapter);
    let wasi = WasiCtxBuilder::new()
        .preopened_dir(tarbox_dir, "/workspace")?
        .build();
    
    // 运行 Agent
    let engine = Engine::default();
    let module = Module::new(&engine, agent_wasm)?;
    // ...
}
```

### 场景 2: 代码沙箱

```rust
// 在线代码执行平台
async fn execute_user_code(code: &str, user_id: Uuid) -> Result<String> {
    let adapter = WasiAdapter::new(fs, user_id, config);
    
    // 用户代码可以读写文件，但隔离在自己的租户下
    // 所有操作自动审计
    // 可以随时回滚到之前的状态
}
```

### 场景 3: Edge 部署

```rust
// Cloudflare Workers / Fastly Compute@Edge
use tarbox_wasi::{WasiAdapter, WasiConfig, DbMode};

let config = WasiConfig {
    db_mode: DbMode::Http,
    api_url: Some("https://tarbox-api.example.com".into()),
    api_key: Some(env::var("TARBOX_API_KEY")?),
    ..Default::default()
};

let adapter = WasiAdapter::new_http(config)?;
// 通过 HTTP API 访问远程 Tarbox 服务器
```

## 文档要求

### README.md

```markdown
# tarbox-wasi

WASI filesystem adapter backed by PostgreSQL.

## What is this?

`tarbox-wasi` provides a WASI-compatible filesystem interface that stores 
files in PostgreSQL. It's designed to be integrated into WASI runtimes 
(like Wasmtime or WasmEdge) as a filesystem backend.

## Features

- **Persistent storage**: Files stored in PostgreSQL with ACID guarantees
- **Multi-tenant**: Complete data isolation per tenant
- **Versioning**: Git-like layered storage with snapshots
- **Audit logging**: All operations automatically logged

## Requirements

- Rust 1.92+
- PostgreSQL 16+
- Tokio async runtime

## Quick Start

\`\`\`rust
use tarbox_wasi::{WasiAdapter, WasiConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let pool = sqlx::PgPool::connect("postgres://...").await?;
    let fs = FileSystem::new(pool);
    let adapter = WasiAdapter::new(fs, tenant_id, WasiConfig::default());
    
    // Open and read a file
    let fd = adapter.fd_open("/workspace/file.txt", OpenFlags::READ).await?;
    let mut buf = vec![0u8; 1024];
    let n = adapter.fd_read(fd, &mut buf).await?;
    adapter.fd_close(fd).await?;
    
    Ok(())
}
\`\`\`

## License

MPL-2.0
```

### API 文档

所有公开 API 必须有文档注释：

```rust
/// WASI 文件系统适配器
///
/// 将 WASI 文件系统调用映射到 Tarbox 的 PostgreSQL 存储。
/// 每个适配器实例绑定一个租户 ID，确保多租户隔离。
///
/// # Example
///
/// ```rust
/// use tarbox_wasi::{WasiAdapter, WasiConfig, OpenFlags};
///
/// # async fn example() -> anyhow::Result<()> {
/// let adapter = WasiAdapter::new(fs, tenant_id, WasiConfig::default());
///
/// // 写入文件
/// let fd = adapter.fd_open("/test.txt", OpenFlags::WRITE | OpenFlags::CREATE).await?;
/// adapter.fd_write(fd, b"hello").await?;
/// adapter.fd_close(fd).await?;
/// # Ok(())
/// # }
/// ```
pub struct WasiAdapter<'a> { /* ... */ }
```

### Examples

```
examples/
├── basic_usage.rs           # 基础用法
├── multi_tenant.rs          # 多租户示例
├── http_mode.rs             # HTTP 模式示例
└── wasmtime_integration.rs  # Wasmtime 集成（概念）
```

## 发布检查清单

### 代码准备

- [ ] 从 tarbox 主项目分离代码
- [ ] 创建独立的 Cargo.toml
- [ ] 所有公开 API 有文档注释
- [ ] 所有公开类型实现必要 trait（Debug, Clone 等）
- [ ] 无 `unwrap()` 或 `expect()` 在库代码中
- [ ] 错误类型实现 `std::error::Error`

### 测试

- [ ] 单元测试覆盖率 >80%
- [ ] 集成测试（需要 PostgreSQL）
- [ ] 文档测试（`cargo test --doc`）
- [ ] 示例代码可运行

### 文档

- [ ] README.md 完整
- [ ] API 文档完整
- [ ] CHANGELOG.md
- [ ] LICENSE 文件
- [ ] examples/ 目录

### Cargo.toml

- [ ] `name` 唯一（检查 crates.io）
- [ ] `version` 正确
- [ ] `description` 简洁
- [ ] `license` 正确
- [ ] `repository` 正确
- [ ] `documentation` 指向 docs.rs
- [ ] `keywords` 和 `categories` 设置
- [ ] `rust-version` 设置
- [ ] `exclude` 排除不需要的文件

### 质量检查

- [ ] `cargo fmt --check` 通过
- [ ] `cargo clippy` 无警告
- [ ] `cargo deny check` 通过
- [ ] `cargo audit` 无安全漏洞
- [ ] `cargo doc --no-deps` 生成成功

### 发布

- [ ] `cargo package` 成功
- [ ] `cargo package --list` 检查文件
- [ ] `cargo publish --dry-run` 成功
- [ ] crates.io 账号准备
- [ ] `cargo publish`

## 版本规划

### v0.1.0 (初始版本)

- 基础 `WasiAdapter` API
- `fd_open`, `fd_read`, `fd_write`, `fd_close`
- `path_create_directory`, `path_remove_directory`
- `WasiConfig` 基础配置
- `WasiError` 错误类型
- Direct 模式（直连 PostgreSQL）

### v0.2.0

- `fd_seek`, `fd_readdir`
- `path_rename`, `path_unlink_file`
- `fd_filestat_get`, `path_filestat_get`
- HTTP 模式支持

### v0.3.0

- 缓存层
- 性能优化
- Wasmtime trait 绑定示例

### v1.0.0

- API 稳定
- 完整文档
- 生产就绪

## 与其他 Spec 的关系

- **spec/01**: 数据库 schema - 底层存储
- **spec/13**: WASI 适配器技术设计 - 实现细节
- **spec/14**: 文件系统接口 - FileSystem API
- **spec/06**: API 设计 - HTTP 模式使用的 API

## 风险和考虑

### 技术风险

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| PostgreSQL 依赖体积 | 用户需要 PostgreSQL | 明确文档说明 |
| 异步运行时锁定 | 必须用 tokio | 这是标准选择 |
| API 变更 | 破坏用户代码 | 遵循 SemVer |

### 维护考虑

- 需要持续维护依赖更新
- 需要响应用户 issue
- 需要保持与 tarbox 主项目同步

### 竞品分析

| 方案 | 优点 | 缺点 |
|------|------|------|
| 内存文件系统 | 轻量 | 不持久化 |
| 本地文件系统 | 简单 | 无多租户 |
| S3/对象存储 | 云原生 | 延迟高，无 POSIX |
| **tarbox-wasi** | 持久化+多租户+版本控制 | 需要 PostgreSQL |

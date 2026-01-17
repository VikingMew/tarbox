# Spec 13: WASI Interface

**优先级**: P2 (高级功能)  
**状态**: 设计阶段  
**依赖**: spec/14 (文件系统接口抽象层), spec/06 (API 设计)  
**基于**: [spec/14-filesystem-interface.md](14-filesystem-interface.md)

## 概述

WASI (WebAssembly System Interface) 支持使 Tarbox 可以编译为 WebAssembly 模块并在各种 WASM 运行时中运行。这为 Tarbox 带来以下能力：

- **跨平台部署**：在浏览器、边缘节点、serverless 环境中运行
- **安全隔离**：WASM 的沙箱特性提供额外的安全层
- **轻量级部署**：无需操作系统级依赖，启动快速
- **云原生集成**：与 Kubernetes、Spin、Wasmtime、WasmEdge 等运行时集成

**本规范描述 WASI 适配器的具体实现细节**，包括 WASI filesystem 接口映射、HTTP database client、运行时集成等。核心的文件系统操作通过 spec/14 定义的 `FilesystemInterface` 实现。

## 架构定位

```
WASM Runtime (Wasmtime/Wasmer/Browser)
    ↓
WASI Preview 2 (wasi-filesystem)
    ↓
┌─────────────────────────────────┐
│  WasiAdapter (本规范)           │  ← 协议适配层
│  - WASI → Interface 映射        │
│  - 文件描述符管理               │
│  - WASI 错误码转换              │
└─────────────────────────────────┘
    ↓ 实现 FilesystemInterface trait
┌─────────────────────────────────┐
│  FilesystemInterface (spec/14)  │  ← 统一抽象层
└─────────────────────────────────┘
    ↓
┌─────────────────────────────────┐
│  HTTP Database Client           │  ← WASM 后端
│  或 SQLite Embedded             │
└─────────────────────────────────┘
```

## 设计目标

1. **Rust WASI 兼容性**：利用 Rust 的 `wasm32-wasi` target
2. **POSIX 兼容子集**：实现 WASI 文件系统接口
3. **异步支持**：支持 WASI Preview 2 的异步 I/O
4. **无 FUSE 依赖**：WASM 环境中使用 WASI 文件系统接口替代 FUSE
5. **数据库连接**：通过 HTTP/WebSocket 连接 PostgreSQL（或使用 SQLite）

## 架构设计

### 层次结构

```
┌─────────────────────────────────────────┐
│   WASM Runtime (Wasmtime/Wasmer/Spin)   │
├─────────────────────────────────────────┤
│   WASI Preview 2 (wasi-filesystem)      │
├─────────────────────────────────────────┤
│   Tarbox WASI Adapter                   │
│   ├─ WASI Filesystem Implementation     │
│   ├─ HTTP Database Client               │
│   └─ Memory Cache                       │
├─────────────────────────────────────────┤
│   Tarbox Core (fs/, storage/, layer/)   │
└─────────────────────────────────────────┘
```

### 组件划分

**1. WASI Adapter Layer** (`src/wasi/`)
- 实现 WASI filesystem preview 2 接口
- 将 WASI 调用桥接到 Tarbox Core
- 管理租户上下文和权限

**2. HTTP Database Client** (`src/storage/http_client.rs`)
- 通过 HTTP/gRPC 连接远程 PostgreSQL
- 使用 `reqwest` 或 `hyper` (WASM 兼容版本)
- 支持连接池和重试机制

**3. SQLite Fallback** (`src/storage/sqlite.rs`)
- 在不需要多租户时使用嵌入式 SQLite
- 利用 `rusqlite` 的 WASM 支持
- 本地缓存和离线模式

**4. Memory Cache** (`src/cache/wasi.rs`)
- 使用线性内存实现 LRU 缓存
- 减少数据库往返次数
- 支持 inode 和 block 缓存

## WASI Filesystem 接口实现

### 核心接口映射

| WASI Function | Tarbox Implementation | 说明 |
|--------------|----------------------|------|
| `fd_read` | `FileSystem::read_file` | 读取文件数据 |
| `fd_write` | `FileSystem::write_file` | 写入文件数据 |
| `fd_seek` | 内部实现 seek | 文件偏移 |
| `fd_close` | 释放句柄 | 关闭文件 |
| `path_open` | `FileSystem::resolve_path` | 打开文件/目录 |
| `path_create_directory` | `FileSystem::create_directory` | 创建目录 |
| `path_remove_directory` | `FileSystem::remove_directory` | 删除目录 |
| `path_unlink_file` | `FileSystem::delete_file` | 删除文件 |
| `path_filestat_get` | `FileSystem::stat` | 获取元数据 |
| `fd_readdir` | `FileSystem::list_directory` | 列出目录 |

### 文件描述符管理

```rust
// 伪代码示例（不是实现）
struct WasiFileDescriptor {
    fd: u32,
    tenant_id: TenantId,
    inode_id: InodeId,
    path: String,
    flags: OpenFlags,
    position: u64,
}

struct WasiFsContext {
    tenant_id: TenantId,
    fd_table: HashMap<u32, WasiFileDescriptor>,
    next_fd: AtomicU32,
    fs: FileSystem,
}
```

### 路径解析

WASI 使用 preopened directories 概念：

```
/                    -> Tarbox tenant root
/data               -> 挂载点 (preopened)
/tmp                -> 临时目录 (preopened)
/.tarbox            -> 虚拟文件系统钩子
```

## 数据库连接策略

### 方案 A: HTTP Proxy (推荐)

通过 HTTP API 访问 PostgreSQL：

```
WASM Module ──HTTP──> Tarbox API Server ──PostgreSQL──> Database
```

**优点**：
- 无需在 WASM 中链接 PostgreSQL 驱动
- 支持所有 WASM 运行时
- 可以添加认证和授权
- 易于扩展和负载均衡

**实现**：
- 使用 spec 06 (API Design) 的 REST/gRPC 接口
- WASM 模块只需要 HTTP 客户端
- 支持流式传输大文件

### 方案 B: SQLite Embedded

在 WASM 内嵌入 SQLite：

```
WASM Module ──libSQL/rusqlite──> SQLite WASM
```

**优点**：
- 无网络依赖
- 离线可用
- 启动快速

**缺点**：
- 单租户模式
- 无法共享数据
- 内存限制

**适用场景**：
- 边缘计算
- 离线应用
- 开发测试环境

### 方案 C: PostgreSQL Proxy Protocol

使用 PostgreSQL wire protocol over WebSocket：

```
WASM Module ──WebSocket/PostgreSQL Wire──> Proxy ──> PostgreSQL
```

**优点**：
- 直接使用 PostgreSQL
- 支持事务和复杂查询

**缺点**：
- WASM 中需要完整 PostgreSQL 驱动
- 包体积大
- 兼容性问题

## WASM 编译配置

### Cargo.toml 配置

```toml
[target.wasm32-wasi.dependencies]
tokio = { version = "1", features = ["rt", "macros"] }
reqwest = { version = "0.11", default-features = false, features = ["rustls-tls"] }

[profile.release]
opt-level = "z"     # 优化大小
lto = true          # 链接时优化
codegen-units = 1   # 减少代码大小
strip = true        # 去除符号
```

### 构建命令

```bash
# 安装 WASI target
rustup target add wasm32-wasi

# 构建 WASM 模块
cargo build --target wasm32-wasi --release

# 优化 WASM (使用 wasm-opt)
wasm-opt -Oz -o tarbox.wasm target/wasm32-wasi/release/tarbox.wasm

# 组件化 (WASI Preview 2)
wasm-tools component new tarbox.wasm -o tarbox.component.wasm
```

## 运行时支持

### Wasmtime (推荐)

```rust
// host.rs - WASM 主机代码示例
use wasmtime::*;
use wasmtime_wasi::WasiCtxBuilder;

let engine = Engine::default();
let mut linker = Linker::new(&engine);
wasmtime_wasi::add_to_linker(&mut linker, |s| s)?;

let wasi = WasiCtxBuilder::new()
    .inherit_stdio()
    .preopened_dir(Dir::open_ambient_dir("/data", ambient_authority())?, "/data")?
    .build();

let mut store = Store::new(&engine, wasi);
let module = Module::from_file(&engine, "tarbox.wasm")?;
let instance = linker.instantiate(&mut store, &module)?;
```

### Spin (Fermyon)

```toml
# spin.toml
spin_manifest_version = "1"

[[component]]
id = "tarbox"
source = "tarbox.wasm"
allowed_http_hosts = ["https://api.tarbox.io"]
files = ["/data/*"]
environment = { DATABASE_URL = "https://api.tarbox.io/db" }
```

### WasmEdge

```bash
wasmedge --dir /data:/host/data tarbox.wasm
```

## 限制和权衡

### WASM 限制

1. **无多线程支持**（WASI Preview 1）
   - 解决方案：使用单线程异步运行时
   - WASI Preview 2 将支持多线程

2. **内存限制**
   - 默认 4GB 线性内存上限
   - 需要谨慎管理缓存大小

3. **无 FUSE 支持**
   - WASM 无法挂载文件系统
   - 必须通过 WASI 接口访问

4. **有限的系统调用**
   - 无法使用 `fork`, `exec` 等
   - 需要纯 Rust 实现

### 性能考虑

- **网络延迟**：数据库通过 HTTP 访问会增加延迟
- **序列化开销**：数据需要序列化为 JSON/Protobuf
- **缓存策略**：必须实现高效的本地缓存
- **冷启动**：WASM 启动快，但需要预热缓存

## 使用场景

### 场景 1: 边缘计算

```
Edge Node (Wasmtime) ──> Tarbox WASM ──> SQLite Local
                                     └──> Sync to Cloud (后台)
```

**优势**：
- 低延迟本地访问
- 离线可用
- 自动同步

### 场景 2: Serverless Functions

```
AWS Lambda/Cloudflare Workers ──> Tarbox WASM ──> HTTP API
```

**优势**：
- 快速启动
- 按需扩展
- 无需容器镜像

### 场景 3: 浏览器内文件系统

```
Web App ──> Tarbox WASM ──> IndexedDB/OPFS
```

**优势**：
- 完全客户端
- 无服务器成本
- 离线优先

### 场景 4: Kubernetes + WASM

```
Kubernetes (containerd + runwasi) ──> Tarbox WASM Pod
```

**优势**：
- 轻量级部署
- 快速扩缩容
- 多租户隔离

## 开发路线图

### Phase 1: 基础支持 (2-3 周)
- [ ] 添加 `wasm32-wasi` target 支持
- [ ] 实现 HTTP database client
- [ ] 移除 FUSE 依赖（条件编译）
- [ ] 基础 WASI filesystem 接口

### Phase 2: 完整实现 (3-4 周)
- [ ] 完整 WASI Preview 2 支持
- [ ] SQLite 嵌入式支持
- [ ] 内存缓存优化
- [ ] 文件描述符管理

### Phase 3: 运行时集成 (2-3 周)
- [ ] Wasmtime 示例和文档
- [ ] Spin 组件
- [ ] WasmEdge 支持
- [ ] 浏览器 WASM 示例

### Phase 4: 优化和测试 (2-3 周)
- [ ] 性能优化
- [ ] 大小优化 (< 5MB)
- [ ] 完整测试套件
- [ ] 生产环境验证

## 技术依赖

### Rust Crates

```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
# WASI 运行时
wasi = "0.11"

# HTTP 客户端 (WASM 兼容)
reqwest = { version = "0.11", default-features = false, features = ["rustls-tls"] }

# SQLite (可选)
rusqlite = { version = "0.30", features = ["bundled"] }

# 序列化
serde_json = "1.0"
bincode = "1.3"

# 异步运行时 (WASM 兼容)
tokio = { version = "1", features = ["rt", "macros"] }
```

### 工具链

- **wasm-pack**: WASM 打包工具
- **wasm-opt**: WASM 优化器
- **wasm-tools**: WASI 组件化工具
- **wasmtime**: 测试运行时

## 配置示例

### 环境变量

```bash
# 数据库连接（HTTP 模式）
TARBOX_DB_MODE=http
TARBOX_API_URL=https://api.tarbox.io
TARBOX_API_KEY=xxx

# 数据库连接（SQLite 模式）
TARBOX_DB_MODE=sqlite
TARBOX_SQLITE_PATH=/data/tarbox.db

# 缓存配置
TARBOX_CACHE_SIZE=100MB
TARBOX_CACHE_TTL=300
```

### WASM 组件配置

```toml
# tarbox.toml
[wasi]
inherit_env = false
env = { TARBOX_DB_MODE = "http" }

[wasi.preopens]
"/data" = "/host/data"
"/tmp" = "/host/tmp"

[wasi.http]
allowed_hosts = ["api.tarbox.io"]
max_connections = 10
```

## 安全考虑

1. **沙箱隔离**：WASM 提供强隔离，限制系统调用
2. **资源限制**：设置内存和 CPU 配额
3. **网络限制**：只允许访问特定 API endpoint
4. **租户隔离**：通过 API 层强制租户隔离
5. **认证授权**：API 访问需要 token 验证

## 测试策略

### 单元测试

```bash
# 针对 WASM target 运行测试
cargo test --target wasm32-wasi
```

### 集成测试

```bash
# 使用 wasmtime 运行集成测试
wasmtime run --dir /data test.wasm
```

### 性能测试

- 冷启动时间 < 100ms
- 文件读写延迟 < 50ms (本地缓存)
- 包大小 < 5MB (压缩后 < 2MB)

## 参考资料

- [WASI Preview 2](https://github.com/WebAssembly/WASI/blob/main/preview2/README.md)
- [Wasmtime](https://wasmtime.dev/)
- [Spin Framework](https://www.fermyon.com/spin)
- [WasmEdge](https://wasmedge.org/)
- [Rust WASM Book](https://rustwasm.github.io/docs/book/)

## 与其他 Spec 的关系

- **Spec 01**: 数据库 schema 保持不变，但访问方式改为 HTTP
- **Spec 02**: FUSE 接口替换为 WASI filesystem 接口
- **Spec 06**: API 设计必须支持 WASM 客户端
- **Spec 07**: 性能优化需要考虑 WASM 特性
- **Spec 09**: 多租户通过 API 层控制

## 决策记录

### DR-13-1: 优先支持 HTTP API 模式而非嵌入式数据库

**原因**：
- 保持多租户能力
- 简化 WASM 模块大小
- 易于扩展和维护
- 符合云原生架构

### DR-13-2: 使用 WASI Preview 2

**原因**：
- Preview 2 是未来标准
- 支持异步 I/O
- 更好的组件化支持
- 虽然还在开发中，但 Rust 支持良好

### DR-13-3: 条件编译分离 FUSE 和 WASI

**原因**：
- 同一代码库支持两种模式
- 减少代码重复
- 方便测试和维护

## 状态

- **当前状态**: 设计阶段
- **优先级**: P2 (高级功能)
- **依赖**: Spec 01, 06
- **预计工作量**: 8-12 周

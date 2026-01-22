# Task 16: WASI 支持

## 状态

**📅 计划中**

## 目标

将 Tarbox 编译为 WebAssembly (WASM) 模块，支持在各种 WASM 运行时中运行，包括浏览器、边缘节点、Serverless 环境和 Kubernetes。基于 spec/14 的 FilesystemInterface 抽象层，通过适配器模式实现 WASI filesystem 接口，复用 TarboxBackend 的核心逻辑。

**核心特性**：
- **WASI Preview 2 支持**: 实现标准 WASI filesystem 接口
- **HTTP Database Client**: 通过 HTTP API 访问 PostgreSQL
- **SQLite 嵌入**: 可选的本地数据库（离线模式）
- **跨平台部署**: 浏览器、Wasmtime、Spin、WasmEdge
- **轻量级**: 优化后 WASM 模块 < 5MB
- **快速启动**: 冷启动 < 100ms

## 优先级

**P2 - 云原生集成**

## 依赖

- Task 05: FUSE 接口 ✅ (FilesystemInterface 抽象层)
- Task 14: REST API (HTTP database client 需要)
- Task 06: 数据库层高级 ✅ (存储层)

## 依赖的Spec

- **spec/13-wasi-interface.md** - WASI 接口设计（核心）
- **spec/14-filesystem-interface.md** - 文件系统抽象层（核心）
- spec/06-api-design.md - REST API（用于 HTTP client）
- spec/07-performance.md - 性能优化

## 实现内容

### 1. WASM Target 支持

- [ ] **Cargo 配置** (`Cargo.toml`)
  ```toml
  [target.wasm32-wasi.dependencies]
  # WASI 运行时
  wasi = "0.11"
  
  # HTTP 客户端 (WASM 兼容)
  reqwest = { version = "0.11", default-features = false, features = ["rustls-tls"] }
  
  # SQLite (可选)
  rusqlite = { version = "0.30", features = ["bundled"], optional = true }
  
  # 异步运行时 (WASM 兼容)
  tokio = { version = "1", features = ["rt", "macros"] }
  
  # 序列化
  serde_json = "1.0"
  bincode = "1.3"
  
  [profile.release.package.tarbox]
  opt-level = "z"     # 优化大小
  lto = true          # 链接时优化
  codegen-units = 1   # 减少代码大小
  strip = true        # 去除符号
  ```

- [ ] **条件编译** (`src/lib.rs`)
  ```rust
  #[cfg(target_arch = "wasm32")]
  pub mod wasi;
  
  #[cfg(not(target_arch = "wasm32"))]
  pub mod fuse;
  
  // 共享的核心代码
  pub mod fs;
  pub mod storage;
  pub mod layer;
  ```

- [ ] **构建脚本** (`scripts/build-wasm.sh`)
  ```bash
  #!/bin/bash
  # 构建 WASM 模块
  cargo build --target wasm32-wasi --release
  
  # 优化 WASM
  wasm-opt -Oz -o tarbox.wasm target/wasm32-wasi/release/tarbox.wasm
  
  # 组件化 (WASI Preview 2)
  wasm-tools component new tarbox.wasm -o tarbox.component.wasm
  ```

### 2. WASI Filesystem 接口

- [ ] **WASI Adapter** (`src/wasi/adapter.rs`)
  - 实现 WASI filesystem preview 2 接口
  - 桥接到 FilesystemInterface
  - 文件描述符管理
  ```rust
  pub struct WasiAdapter {
      backend: Arc<TarboxBackend>,
      fd_table: Arc<Mutex<FdTable>>,
      tenant_id: TenantId,
  }
  
  // 实现 FilesystemInterface（复用代码）
  impl FilesystemInterface for WasiAdapter {
      async fn read_file(&self, path: &str) -> Result<Vec<u8>> {
          self.backend.read_file(path).await
      }
      // ... 其他方法
  }
  
  // WASI 特有的接口
  impl WasiAdapter {
      pub fn fd_read(&mut self, fd: u32, buf: &mut [u8]) -> Result<usize>;
      pub fn fd_write(&mut self, fd: u32, buf: &[u8]) -> Result<usize>;
      pub fn fd_seek(&mut self, fd: u32, offset: i64, whence: u8) -> Result<u64>;
      pub fn fd_close(&mut self, fd: u32) -> Result<()>;
      pub fn path_open(&mut self, path: &str, flags: u16) -> Result<u32>;
      // ... 其他 WASI 函数
  }
  ```

- [ ] **文件描述符表** (`src/wasi/fd_table.rs`)
  ```rust
  pub struct FdTable {
      fds: HashMap<u32, FileDescriptor>,
      next_fd: u32,
  }
  
  pub struct FileDescriptor {
      inode_id: InodeId,
      path: String,
      flags: OpenFlags,
      position: u64,
      is_directory: bool,
  }
  ```

- [ ] **WASI 错误码映射** (`src/wasi/error.rs`)
  ```rust
  pub fn to_wasi_errno(err: &TarboxError) -> u16 {
      match err {
          TarboxError::NotFound => wasi::ERRNO_NOENT,
          TarboxError::PermissionDenied => wasi::ERRNO_ACCES,
          TarboxError::AlreadyExists => wasi::ERRNO_EXIST,
          // ... 其他映射
      }
  }
  ```

### 3. HTTP Database Client

- [ ] **HTTP Client** (`src/storage/http_client.rs`)
  - 通过 REST API 访问 Tarbox 服务器
  - 替代直接的 PostgreSQL 连接
  - 支持连接池和重试
  ```rust
  pub struct HttpDatabaseClient {
      base_url: String,
      api_key: String,
      client: reqwest::Client,
  }
  
  impl HttpDatabaseClient {
      pub async fn new(base_url: String, api_key: String) -> Result<Self>;
      
      // 实现 Repository traits 通过 HTTP
      pub async fn create_tenant(&self, input: CreateTenantInput) -> Result<Tenant>;
      pub async fn get_inode(&self, tenant_id: Uuid, inode_id: i64) -> Result<Option<Inode>>;
      pub async fn read_blocks(&self, block_ids: &[Uuid]) -> Result<Vec<DataBlock>>;
      // ... 其他方法
  }
  ```

- [ ] **API 端点映射**
  ```
  GET  /api/v1/tenants/{id}           -> TenantRepository::get_tenant
  POST /api/v1/tenants                -> TenantRepository::create_tenant
  GET  /api/v1/inodes/{tenant_id}/{inode_id} -> InodeRepository::get
  POST /api/v1/files/{tenant_id}/read -> FileSystem::read_file
  POST /api/v1/files/{tenant_id}/write -> FileSystem::write_file
  ...
  ```

- [ ] **请求缓存** (`src/storage/http_cache.rs`)
  - 减少 HTTP 往返次数
  - LRU 缓存（基于线性内存）
  - TTL 过期策略

### 4. SQLite 嵌入式支持（可选）

- [ ] **SQLite Backend** (`src/storage/sqlite_backend.rs`)
  - 用于离线模式或边缘场景
  - 实现所有 Repository traits
  - Schema 迁移
  ```rust
  #[cfg(feature = "sqlite")]
  pub struct SqliteBackend {
      conn: Arc<Mutex<rusqlite::Connection>>,
  }
  
  impl SqliteBackend {
      pub fn new(path: &str) -> Result<Self>;
      pub fn migrate(&self) -> Result<()>;
  }
  
  // 实现 Repository traits
  impl TenantRepository for SqliteBackend { ... }
  impl InodeRepository for SqliteBackend { ... }
  // ...
  ```

- [ ] **Schema 同步**
  - 与 PostgreSQL schema 保持一致
  - 简化版（单租户）
  - 迁移脚本

### 5. 内存优化

- [ ] **Memory Allocator** (`src/wasi/allocator.rs`)
  - 使用 wee_alloc 或 dlmalloc
  - 最小化内存占用
  ```rust
  #[cfg(target_arch = "wasm32")]
  #[global_allocator]
  static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;
  ```

- [ ] **缓存大小限制**
  - 配置最大缓存大小
  - 自动淘汰策略
  - 内存压力监控

### 6. 运行时集成

- [ ] **Wasmtime 示例** (`examples/wasmtime/`)
  ```rust
  // examples/wasmtime/host.rs
  use wasmtime::*;
  use wasmtime_wasi::WasiCtxBuilder;
  
  fn main() -> Result<()> {
      let engine = Engine::default();
      let mut linker = Linker::new(&engine);
      wasmtime_wasi::add_to_linker(&mut linker, |s| s)?;
      
      let wasi = WasiCtxBuilder::new()
          .inherit_stdio()
          .preopened_dir(Dir::open_ambient_dir("/data", ambient_authority())?, "/data")?
          .env("TARBOX_API_URL", "https://api.tarbox.io")?
          .build();
      
      let mut store = Store::new(&engine, wasi);
      let module = Module::from_file(&engine, "tarbox.wasm")?;
      let instance = linker.instantiate(&mut store, &module)?;
      
      // 调用导出函数
      Ok(())
  }
  ```

- [ ] **Spin 组件** (`examples/spin/`)
  ```toml
  # examples/spin/spin.toml
  spin_manifest_version = "1"
  
  [[component]]
  id = "tarbox"
  source = "../../tarbox.wasm"
  allowed_http_hosts = ["https://api.tarbox.io"]
  files = ["/data/*"]
  environment = { 
    TARBOX_API_URL = "https://api.tarbox.io",
    TARBOX_API_KEY = "{{ api_key }}"
  }
  
  [[component.trigger.http]]
  route = "/fs/..."
  executor = { type = "spin" }
  ```

- [ ] **WasmEdge 示例** (`examples/wasmedge/`)
  ```bash
  # examples/wasmedge/run.sh
  wasmedge \
    --dir /data:/host/data \
    --env TARBOX_API_URL=https://api.tarbox.io \
    tarbox.wasm
  ```

- [ ] **浏览器示例** (`examples/browser/`)
  ```html
  <!-- examples/browser/index.html -->
  <script type="module">
    import init, { TarboxFs } from './tarbox.js';
    
    async function main() {
      await init();
      const fs = new TarboxFs('https://api.tarbox.io', 'api-key');
      
      // 使用文件系统
      await fs.writeFile('/test.txt', 'Hello WASM');
      const content = await fs.readFile('/test.txt');
      console.log(content);
    }
    
    main();
  </script>
  ```

### 7. Preopened Directories

- [ ] **Preopens 管理** (`src/wasi/preopens.rs`)
  ```rust
  pub struct PreopenedDirs {
      dirs: HashMap<String, PreopenedDir>,
  }
  
  pub struct PreopenedDir {
      virtual_path: String,  // e.g., "/data"
      capabilities: Capabilities,
  }
  
  impl PreopenedDirs {
      pub fn add(&mut self, virtual_path: &str);
      pub fn resolve(&self, path: &str) -> Option<&PreopenedDir>;
  }
  ```

- [ ] **标准 Preopens**
  - `/` - 租户根目录
  - `/tmp` - 临时目录
  - `/.tarbox` - 虚拟文件系统钩子

### 8. 配置和环境变量

- [ ] **WASM 配置** (`src/wasi/config.rs`)
  ```rust
  pub struct WasiConfig {
      pub db_mode: DbMode,
      pub api_url: Option<String>,
      pub api_key: Option<String>,
      pub sqlite_path: Option<String>,
      pub cache_size_mb: usize,
      pub cache_ttl_secs: u64,
  }
  
  pub enum DbMode {
      Http,
      Sqlite,
  }
  
  impl WasiConfig {
      pub fn from_env() -> Result<Self> {
          Ok(Self {
              db_mode: match std::env::var("TARBOX_DB_MODE")?.as_str() {
                  "http" => DbMode::Http,
                  "sqlite" => DbMode::Sqlite,
                  _ => return Err(anyhow!("Invalid DB mode")),
              },
              api_url: std::env::var("TARBOX_API_URL").ok(),
              api_key: std::env::var("TARBOX_API_KEY").ok(),
              sqlite_path: std::env::var("TARBOX_SQLITE_PATH").ok(),
              cache_size_mb: std::env::var("TARBOX_CACHE_SIZE")
                  .unwrap_or_else(|_| "100".to_string())
                  .parse()?,
              cache_ttl_secs: std::env::var("TARBOX_CACHE_TTL")
                  .unwrap_or_else(|_| "300".to_string())
                  .parse()?,
          })
      }
  }
  ```

### 9. JavaScript/TypeScript 绑定

- [ ] **wasm-bindgen 支持** (`src/wasi/bindings.rs`)
  ```rust
  use wasm_bindgen::prelude::*;
  
  #[wasm_bindgen]
  pub struct TarboxFs {
      adapter: WasiAdapter,
  }
  
  #[wasm_bindgen]
  impl TarboxFs {
      #[wasm_bindgen(constructor)]
      pub fn new(api_url: String, api_key: String) -> Result<TarboxFs, JsValue>;
      
      #[wasm_bindgen(js_name = writeFile)]
      pub async fn write_file(&mut self, path: String, content: Vec<u8>) -> Result<(), JsValue>;
      
      #[wasm_bindgen(js_name = readFile)]
      pub async fn read_file(&self, path: String) -> Result<Vec<u8>, JsValue>;
      
      #[wasm_bindgen(js_name = listDirectory)]
      pub async fn list_directory(&self, path: String) -> Result<JsValue, JsValue>;
      
      // ... 其他方法
  }
  ```

- [ ] **TypeScript 类型定义** (`pkg/tarbox.d.ts`)
  ```typescript
  export class TarboxFs {
    constructor(apiUrl: string, apiKey: string);
    writeFile(path: string, content: Uint8Array): Promise<void>;
    readFile(path: string): Promise<Uint8Array>;
    listDirectory(path: string): Promise<DirectoryEntry[]>;
    // ...
  }
  
  export interface DirectoryEntry {
    name: string;
    isDirectory: boolean;
    size: number;
  }
  ```

### 10. 测试

- [ ] **单元测试**
  - WASI 接口测试
  - 文件描述符管理测试
  - HTTP client 测试
  - 错误码映射测试

- [ ] **集成测试** (`tests/wasi_integration_test.rs`)
  - test_wasi_file_operations - 文件读写
  - test_wasi_directory_operations - 目录操作
  - test_http_client - HTTP database client
  - test_sqlite_backend - SQLite 后端
  - test_fd_management - 文件描述符管理
  - test_preopens - Preopened directories

- [ ] **运行时测试**
  - Wasmtime 运行测试
  - Spin 运行测试
  - WasmEdge 运行测试
  - 浏览器运行测试

- [ ] **性能测试** (`benches/wasi_benchmark.rs`)
  - 冷启动时间 < 100ms
  - 文件读写延迟 < 50ms (缓存命中)
  - WASM 模块大小 < 5MB
  - 内存占用 < 100MB

### 11. 文档和示例

- [ ] **WASM 使用文档** (`doc/wasm-guide.md`)
  - 编译 WASM 模块
  - 在不同运行时中运行
  - 配置和环境变量
  - 性能优化建议

- [ ] **API 文档**
  - JavaScript/TypeScript API
  - Rust WASI API
  - HTTP API 规范

- [ ] **使用场景示例**
  - 边缘计算示例
  - Serverless 函数示例
  - 浏览器应用示例
  - Kubernetes WASM 示例

## 架构要点

### WASI Adapter 模式

```rust
// WASI Adapter 实现 FilesystemInterface
pub struct WasiAdapter {
    backend: Arc<TarboxBackend>,
    http_client: Option<Arc<HttpDatabaseClient>>,
    sqlite_backend: Option<Arc<SqliteBackend>>,
    fd_table: Arc<Mutex<FdTable>>,
    tenant_id: TenantId,
}

// 复用 90% 代码
impl FilesystemInterface for WasiAdapter {
    async fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        self.backend.read_file(path).await
    }
    // ... 其他方法直接转发
}

// WASI 特有的 fd 操作
impl WasiAdapter {
    pub fn fd_read(&mut self, fd: u32, buf: &mut [u8]) -> Result<usize> {
        let fd_entry = self.fd_table.lock().unwrap().get(fd)?;
        let data = self.backend.read_file(&fd_entry.path).await?;
        let to_read = data.len().min(buf.len());
        buf[..to_read].copy_from_slice(&data[..to_read]);
        Ok(to_read)
    }
}
```

### 数据库访问模式

```
HTTP Mode:
WASM Module → HTTP Client → Tarbox API Server → PostgreSQL

SQLite Mode:
WASM Module → SQLite WASM → IndexedDB (浏览器) / File (Wasmtime)

Hybrid Mode:
WASM Module → Local SQLite (cache) → HTTP Client (sync)
```

## 验收标准

### 核心功能
- [ ] WASM 模块成功编译（wasm32-wasi）
- [ ] WASI filesystem 接口正常工作
- [ ] HTTP database client 正常工作
- [ ] SQLite backend 正常工作（可选）
- [ ] 文件描述符管理正确
- [ ] Preopened directories 正常工作

### 运行时兼容性
- [ ] Wasmtime 运行成功
- [ ] Spin 运行成功
- [ ] WasmEdge 运行成功
- [ ] 浏览器运行成功

### 质量标准
- [ ] 单元测试覆盖率 >55%
- [ ] 集成测试覆盖率 >25%
- [ ] 总覆盖率 >80%
- [ ] cargo fmt 通过
- [ ] cargo clippy 无警告

### 性能标准
- [ ] WASM 模块大小 < 5MB (优化后 < 2MB)
- [ ] 冷启动时间 < 100ms
- [ ] 文件读写延迟 < 50ms (缓存命中)
- [ ] 内存占用 < 100MB

## 文件清单

### 新增文件
```
src/wasi/
├── mod.rs              - 模块导出
├── adapter.rs          - WASI Adapter
├── fd_table.rs         - 文件描述符表
├── error.rs            - 错误码映射
├── preopens.rs         - Preopened directories
├── config.rs           - WASM 配置
├── allocator.rs        - 内存分配器
└── bindings.rs         - JavaScript 绑定

src/storage/
├── http_client.rs      - HTTP database client
├── http_cache.rs       - HTTP 请求缓存
└── sqlite_backend.rs   - SQLite 后端（可选）

examples/
├── wasmtime/           - Wasmtime 示例
├── spin/               - Spin 示例
├── wasmedge/           - WasmEdge 示例
└── browser/            - 浏览器示例

scripts/
└── build-wasm.sh       - WASM 构建脚本

doc/
└── wasm-guide.md       - WASM 使用指南

tests/
└── wasi_integration_test.rs

benches/
└── wasi_benchmark.rs
```

### 修改文件
- Cargo.toml - 添加 wasm32-wasi target 依赖
- src/lib.rs - 条件编译 WASI/FUSE
- README.md - 添加 WASM 使用说明

## 技术栈

- **wasi** - WASI 运行时
- **wasm-bindgen** - JavaScript 绑定
- **reqwest** - HTTP 客户端（WASM 兼容）
- **rusqlite** - SQLite（可选）
- **wee_alloc** - 轻量级内存分配器
- **tokio** - 异步运行时（WASM 兼容）
- **wasm-opt** - WASM 优化器
- **wasm-tools** - WASI 组件化工具

## 开发路线图

### Phase 1: 基础支持 (2-3 周)
- 添加 wasm32-wasi target 支持
- 实现 HTTP database client
- 条件编译分离 FUSE/WASI
- 基础 WASI filesystem 接口

### Phase 2: 完整实现 (3-4 周)
- 完整 WASI Preview 2 支持
- SQLite 嵌入式支持
- 内存缓存优化
- 文件描述符管理

### Phase 3: 运行时集成 (2-3 周)
- Wasmtime 示例和文档
- Spin 组件
- WasmEdge 支持
- 浏览器 WASM 示例

### Phase 4: 优化和测试 (2-3 周)
- 性能优化
- 大小优化 (< 5MB)
- 完整测试套件
- 生产环境验证

**总计**: 9-13 周（得益于 FilesystemInterface 复用，减少 2-4 周）

## 后续任务

完成后可以开始：
- 边缘计算场景验证
- Serverless 集成
- 浏览器 IDE 集成

## 参考资料

- [WASI Preview 2](https://github.com/WebAssembly/WASI)
- [Wasmtime](https://wasmtime.dev/)
- [Spin Framework](https://www.fermyon.com/spin)
- [WasmEdge](https://wasmedge.org/)
- [Rust WASM Book](https://rustwasm.github.io/docs/book/)
- [wasm-bindgen](https://rustwasm.github.io/wasm-bindgen/)
- spec/13-wasi-interface.md - 完整设计文档
- spec/14-filesystem-interface.md - 抽象层设计

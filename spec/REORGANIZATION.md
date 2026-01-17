# Spec 重组方案

## 背景

当前的 spec 组织存在以下问题：
1. **接口层重复**: FUSE、CSI、WASI 三个独立规范，但共用相同后端
2. **存储层过大**: spec/01 包含所有表结构，难以维护
3. **性能规范分散**: 性能优化和原生挂载分离

## 核心发现：统一的接口抽象层

### 三个接口的共同点

```rust
// 所有接口都需要实现这些核心操作
trait FilesystemInterface {
    // 文件操作
    async fn read(&self, path: &str) -> Result<Vec<u8>>;
    async fn write(&self, path: &str, data: &[u8]) -> Result<()>;
    async fn create(&self, path: &str) -> Result<()>;
    async fn delete(&self, path: &str) -> Result<()>;
    
    // 目录操作
    async fn mkdir(&self, path: &str) -> Result<()>;
    async fn readdir(&self, path: &str) -> Result<Vec<DirEntry>>;
    async fn rmdir(&self, path: &str) -> Result<()>;
    
    // 元数据操作
    async fn stat(&self, path: &str) -> Result<FileAttr>;
    async fn chmod(&self, path: &str, mode: u32) -> Result<()>;
    async fn chown(&self, path: &str, uid: u32, gid: u32) -> Result<()>;
}
```

### 每个接口只是不同的"适配器"

```
┌──────────────────────────────────────────────────┐
│              应用层                                │
│  FUSE Client  │  K8s Pod  │  WASM Runtime        │
└───────┬───────┴─────┬─────┴──────┬───────────────┘
        │             │            │
   ┌────▼────┐   ┌───▼────┐  ┌────▼─────┐
   │  FUSE   │   │  CSI   │  │  WASI    │
   │ Adapter │   │ Adapter│  │ Adapter  │
   └────┬────┘   └───┬────┘  └────┬─────┘
        │             │            │
        └─────────────┼────────────┘
                      │
        ┌─────────────▼──────────────┐
        │ FilesystemInterface Trait  │
        │  (统一的文件系统抽象)       │
        └─────────────┬──────────────┘
                      │
        ┌─────────────▼──────────────┐
        │   Tarbox Core Backend      │
        │   (fs/ + storage/ + layer/)│
        └────────────────────────────┘
```

## 重组方案

### 方案 A：保守重组（推荐）

**保持现有编号，添加新的抽象层 spec**

1. **新建 spec/14-filesystem-interface.md** - 文件系统接口抽象
   - 定义 `FilesystemInterface` trait
   - 核心操作语义
   - 错误类型映射规范
   - 异步/同步桥接模式

2. **更新现有 spec**
   - **spec/02**: FUSE 接口 → 实现 FilesystemInterface
   - **spec/05**: Kubernetes CSI → 实现 FilesystemInterface
   - **spec/13**: WASI 接口 → 实现 FilesystemInterface
   - 每个 spec 只描述特定适配器的细节

3. **合并性能相关 spec**
   - 将 **spec/12** (原生挂载) 内容合并到 **spec/07** (性能优化)
   - spec/12 改为重定向文档，指向 spec/07

4. **拆分存储 spec**
   - **spec/01**: 核心存储 Schema (只包含 MVP tables)
   - **spec/01-advanced**: 高级存储 Schema (layers, text_blocks, audit_logs)

**优点**：
- 最小变动，现有引用不受影响
- 逐步迁移，风险低
- 清晰的抽象层次

**缺点**：
- 仍然有多个编号

---

### 方案 B：激进重组（更清晰但变动大）

**完全重新编号，按层次组织**

#### 第一层：核心架构 (00-09)
- 00: 系统概览
- 01: 核心存储 Schema (MVP)
- 02: 文件系统接口抽象 (FilesystemInterface trait)
- 03: 多租户隔离
- 04-08: (保留给未来核心概念)
- 09: 依赖管理

#### 第二层：存储与功能 (10-19)
- 10: 高级存储 Schema (layers, text, audit)
- 11: 分层文件系统
- 12: 审计系统
- 13: 文本文件优化
- 14: 文件系统钩子
- 15-19: (保留)

#### 第三层：接口适配器 (20-29)
- 20: FUSE 适配器
- 21: Kubernetes CSI 适配器
- 22: WASI 适配器
- 23: CLI 适配器 (新增)
- 24-29: (保留给未来接口)

#### 第四层：API 与服务 (30-39)
- 30: REST API
- 31: gRPC API
- 32-39: (保留)

#### 第五层：优化与工具 (40-49)
- 40: 性能优化（包含缓存、原生挂载等）
- 41-49: (保留)

**优点**：
- 层次清晰，易于理解
- 编号有逻辑规律
- 易于扩展

**缺点**：
- 需要更新所有引用
- 历史记录混乱
- 变动太大

---

## 推荐方案：方案 A + 渐进式重构

### 立即执行

#### 1. 创建 spec/14-filesystem-interface.md

```markdown
# Spec 14: 文件系统接口抽象层

## 概述

定义统一的文件系统接口抽象，为 FUSE、CSI、WASI 等不同接口提供共同的实现基础。

## 核心 Trait 定义

\`\`\`rust
#[async_trait]
pub trait FilesystemInterface: Send + Sync {
    // 文件操作
    async fn read_file(&self, path: &str) -> FsResult<Vec<u8>>;
    async fn write_file(&self, path: &str, data: &[u8]) -> FsResult<()>;
    async fn create_file(&self, path: &str) -> FsResult<FileAttr>;
    async fn delete_file(&self, path: &str) -> FsResult<()>;
    
    // 目录操作
    async fn create_dir(&self, path: &str) -> FsResult<FileAttr>;
    async fn read_dir(&self, path: &str) -> FsResult<Vec<DirEntry>>;
    async fn remove_dir(&self, path: &str) -> FsResult<()>;
    
    // 元数据操作
    async fn get_attr(&self, path: &str) -> FsResult<FileAttr>;
    async fn set_attr(&self, path: &str, attr: SetAttr) -> FsResult<()>;
    
    // 链接操作 (可选)
    async fn create_symlink(&self, target: &str, link: &str) -> FsResult<()>;
    async fn read_symlink(&self, path: &str) -> FsResult<String>;
}
\`\`\`

## 错误类型映射

定义统一的错误类型，并说明如何映射到不同接口的错误码。

## 适配器模式

每个具体接口（FUSE/CSI/WASI）实现自己的适配器：

\`\`\`rust
pub struct FuseAdapter {
    backend: Arc<dyn FilesystemInterface>,
}

pub struct CsiAdapter {
    backend: Arc<dyn FilesystemInterface>,
}

pub struct WasiAdapter {
    backend: Arc<dyn FilesystemInterface>,
}
\`\`\`

## 与其他 Spec 的关系

- **spec/02 (FUSE)**: FUSE 适配器实现细节
- **spec/05 (CSI)**: CSI 适配器实现细节
- **spec/13 (WASI)**: WASI 适配器实现细节
- 所有适配器共用此接口抽象
```

#### 2. 更新 spec/02, spec/05, spec/13

在每个文档开头添加：

```markdown
**本规范基于**: [spec/14-filesystem-interface.md](14-filesystem-interface.md)

本文档描述 [FUSE/CSI/WASI] 适配器的具体实现细节，
包括协议特定的映射、性能优化和特殊处理。
```

#### 3. 合并 spec/12 到 spec/07

- 将 spec/12 的内容作为 spec/07 的一个章节
- spec/12 文件保留，但内容改为：

```markdown
# Spec 12: 原生文件系统挂载

**⚠️ 本规范已合并到 spec/07-performance.md**

原生挂载是性能优化的一种手段，现在作为性能优化规范的一部分。

详见: [spec/07-performance.md#native-mounting](07-performance.md#native-mounting)

## 历史原因

原本计划作为独立功能实现，但经过架构评审，
发现原生挂载本质是性能优化手段，应与其他优化策略统一管理。

## 实现建议

推荐使用 bubblewrap 在容器层实现，而非在 Tarbox 内部实现。
```

#### 4. 拆分 spec/01

创建 **spec/01-advanced-storage.md**，将高级表结构移过去：
- layers, layer_entries
- text_blocks, text_line_map, text_file_metadata  
- audit_logs
- native_mounts

spec/01 只保留 MVP tables：
- tenants
- inodes
- data_blocks

### 未来计划

当完成 Phase 2 开发后，可以考虑：
1. 评估方案 A 的效果
2. 如果效果好，保持现状
3. 如果仍然混乱，考虑执行方案 B 的完全重组

## 代码实现建议

### 模块结构

```
src/
├── interface/              # 新增：接口抽象层
│   ├── mod.rs             # FilesystemInterface trait
│   ├── types.rs           # 共用类型（FileAttr, DirEntry）
│   ├── error.rs           # 错误映射
│   └── backend.rs         # 默认实现（基于 fs/）
├── fuse/                  # FUSE 适配器
│   ├── adapter.rs         # 实现 FilesystemInterface
│   └── bridge.rs          # 异步/同步桥接
├── csi/                   # CSI 适配器
│   ├── adapter.rs         # 实现 FilesystemInterface
│   └── driver.rs          # CSI 驱动
├── wasi/                  # WASI 适配器
│   ├── adapter.rs         # 实现 FilesystemInterface
│   └── runtime.rs         # WASM 运行时集成
├── fs/                    # 文件系统核心（后端）
├── storage/               # 数据库层
└── layer/                 # 分层系统
```

### 实现示例

```rust
// src/interface/mod.rs
#[async_trait]
pub trait FilesystemInterface: Send + Sync {
    async fn read_file(&self, path: &str) -> FsResult<Vec<u8>>;
    // ... 其他方法
}

// src/interface/backend.rs
pub struct TarboxBackend {
    fs: FileSystem,
}

#[async_trait]
impl FilesystemInterface for TarboxBackend {
    async fn read_file(&self, path: &str) -> FsResult<Vec<u8>> {
        self.fs.read_file(path).await
    }
}

// src/fuse/adapter.rs
pub struct FuseAdapter {
    backend: Arc<dyn FilesystemInterface>,
}

impl fuser::Filesystem for FuseAdapter {
    fn read(&mut self, ...) {
        // 同步包装
        Runtime::block_on(self.backend.read_file(path))
    }
}
```

## 决策记录

### DR-REORG-1: 采用方案 A 而非方案 B

**原因**：
- 最小化破坏性变更
- 现有 spec 引用保持有效
- 可以渐进式重构
- 风险可控

**权衡**：
- 编号不够"优雅"
- 但实用性更重要

### DR-REORG-2: 创建统一接口抽象层

**原因**：
- 三个接口（FUSE/CSI/WASI）共用 90% 相同逻辑
- DRY 原则（Don't Repeat Yourself）
- 更容易添加新接口（未来可能有 HTTP FS、9P 等）
- 测试更简单（mock 接口而非具体实现）

**实现**：
- 新增 spec/14-filesystem-interface.md
- 定义 `FilesystemInterface` trait
- 每个接口只需实现适配器

### DR-REORG-3: 合并性能相关规范

**原因**：
- 原生挂载本质是性能优化手段
- 应与缓存、并发等优化统一管理
- 减少 spec 数量，降低维护成本

**实现**：
- spec/12 内容合并到 spec/07
- spec/12 文件保留但指向 spec/07

## 时间表

1. **立即**: 创建 spec/14, 更新 spec/02/05/13 引用
2. **本周**: 合并 spec/12 到 spec/07
3. **下周**: 拆分 spec/01 为核心和高级
4. **Phase 2 开始时**: 实现 `FilesystemInterface` trait
5. **Phase 2 结束后**: 评估效果，决定是否进一步重组

## 参考

- [FUSE Low-Level API](https://libfuse.github.io/doxygen/structfuse__lowlevel__ops.html)
- [Kubernetes CSI Spec](https://github.com/container-storage-interface/spec)
- [WASI Filesystem](https://github.com/WebAssembly/WASI/blob/main/preview2/README.md)

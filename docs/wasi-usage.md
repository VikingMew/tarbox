# WASI 使用指南

本文档介绍如何在 Tarbox 中使用 WASI (WebAssembly System Interface)。

---

## 📦 什么是 WASI？

WASI (WebAssembly System Interface) 是 WebAssembly 的系统接口标准，允许 Wasm 模块安全地访问文件系统、网络等系统资源。

## 为什么 Tarbox 需要 WASI？

- **AI 沙箱环境**: 为 AI Agent 提供隔离的执行环境
- **跨平台一致性**: Wasm 代码在不同平台上行为一致
- **细粒度权限控制**: 通过 WASI capability 系统精确控制资源访问
- **多租户隔离**: 每个租户的 Wasm 运行时完全隔离

---

## 架构概览

Tarbox 实现了 WASI 的文件系统接口（wasi-filesystem），将 WASI 调用映射到 Tarbox 的分层文件系统：

```
Wasm Module (AI Agent Code)
       ↓
WASI Preview 2 Interface
       ↓
Tarbox WASI Adapter (src/wasi/)
       ↓
Tarbox Filesystem (src/fs/)
       ↓
PostgreSQL (Layered Storage)
```

### 核心组件

#### 1. WASI Filesystem 实现
**位置**: `src/wasi/filesystem.rs`

实现 WASI Preview 2 的文件系统接口：
- `read_via_stream()` - 流式读取文件
- `write_via_stream()` - 流式写入文件
- `append_via_stream()` - 追加写入
- `get_type()` - 获取文件类型（文件/目录/符号链接）
- `stat()` - 获取文件元数据（大小、修改时间等）

#### 2. WASI Types
**位置**: `src/wasi/types.rs`

WASI 标准类型定义：
- `DescriptorType` - 文件/目录/符号链接类型枚举
- `DescriptorStat` - 文件统计信息（大小、时间戳等）
- `ErrorCode` - WASI 标准错误码
- `OpenFlags` - 文件打开标志（CREATE, TRUNCATE 等）

#### 3. WASI Host
**位置**: `src/wasi/host.rs`

Wasmtime 集成层，提供完整的 WASI 运行时环境：
- 初始化 Wasmtime Engine 和 Store
- 配置 WASI 权限和预打开目录
- 执行 Wasm 模块

---

## 使用示例

### 运行支持 WASI 的 Wasm 模块

```rust
use tarbox::wasi::WasiHost;
use tarbox::storage::DatabasePool;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. 创建数据库连接
    let config = Config::from_env()?;
    let pool = DatabasePool::new(&config).await?;

    // 2. 创建 WASI 主机环境
    let wasi_host = WasiHost::new(
        pool,
        tenant_id,
        "/workspace".to_string(), // 工作目录
    )?;

    // 3. 加载并运行 Wasm 模块
    let wasm_bytes = std::fs::read("agent.wasm")?;
    let result = wasi_host.run_wasm(&wasm_bytes).await?;
    
    println!("Wasm execution result: {:?}", result);
    Ok(())
}
```

### 在 Wasm 模块中访问 Tarbox 文件系统

编译为 `wasm32-wasi` 目标的 Rust 代码：

```rust
// agent.rs - 编译为 wasm32-wasi
use std::fs;
use std::io::Write;

fn main() -> std::io::Result<()> {
    // 这些文件操作会被 Tarbox WASI 适配器处理
    
    // 读取文件
    let content = fs::read_to_string("/workspace/data.txt")?;
    println!("Read: {}", content);
    
    // 写入文件（自动触发 COW 层）
    fs::write("/workspace/output.txt", "result from wasm")?;
    
    // 创建目录
    fs::create_dir("/workspace/results")?;
    
    // 追加写入
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open("/workspace/log.txt")?;
    writeln!(file, "Log entry from wasm")?;
    
    Ok(())
}
```

编译命令：
```bash
rustup target add wasm32-wasi
cargo build --target wasm32-wasi --release
```

---

## 权限模型

WASI 访问受以下限制：

### 租户隔离
- ✅ 每个 Wasm 模块只能访问自己租户的数据
- ✅ 不同租户的 Wasm 运行时完全隔离
- ✅ 数据库层面的 `tenant_id` 强制隔离

### 路径限制
- ✅ 限定在预授权目录内（如 `/workspace`）
- ✅ 不能访问 `/.tarbox/` 等系统目录
- ✅ 符号链接限制（防止逃逸到授权目录外）

### 操作限制
- ✅ 根据 WASI capability 控制读写权限
- ✅ 可配置只读模式（适用于推理任务）
- ✅ 可禁用网络/时钟等其他 WASI 能力

### 资源配额
- ✅ 受租户配额限制（文件数、存储空间）
- ✅ Wasm 执行时间限制（防止无限循环）
- ✅ 内存限制（Wasmtime 配置）

---

## 性能考虑

### 流式 I/O
- 使用 WASI streams API 减少内存复制
- 支持大文件读写（不需要一次性加载到内存）
- 异步流处理（tokio integration）

### 异步执行
- Wasmtime 异步运行时集成
- 文件操作不阻塞主线程
- 支持并发执行多个 Wasm 模块

### 层缓存
- 利用 Tarbox 的层缓存机制加速读取
- Inode 和 block 元数据缓存
- 减少数据库查询

### 最佳实践
```rust
// ❌ 不推荐：一次性读取大文件
let content = fs::read("/workspace/large_file.bin")?;

// ✅ 推荐：流式处理
use std::io::{BufReader, BufRead};
let file = fs::File::open("/workspace/large_file.bin")?;
let reader = BufReader::new(file);
for line in reader.lines() {
    process_line(line?);
}
```

---

## 限制和注意事项

### 当前实现状态
- ✅ WASI Preview 2 filesystem 接口
- ✅ 基本文件操作（读/写/追加）
- ✅ 目录操作（创建/列出）
- ❌ WASI Preview 2 网络接口（未实现）
- ❌ WASI Preview 2 时钟接口（未实现）

### 已知限制
- 不支持硬链接（Tarbox 设计限制）
- 符号链接支持有限
- 文件权限模型简化（无 Unix UID/GID）

---

## 故障排查

### Wasm 模块无法访问文件

**问题**: `fs::read()` 返回权限错误

**解决**:
```rust
// 检查预打开目录是否正确配置
let wasi_host = WasiHost::new(
    pool,
    tenant_id,
    "/workspace".to_string(), // 确保路径正确
)?;
```

### 大文件操作超时

**问题**: 处理大文件时 Wasm 执行超时

**解决**:
```rust
// 使用流式 API 而不是一次性读取
use std::io::Read;
let mut file = fs::File::open("/workspace/large.dat")?;
let mut buffer = [0u8; 8192];
loop {
    let n = file.read(&mut buffer)?;
    if n == 0 { break; }
    process_chunk(&buffer[..n]);
}
```

---

## 相关文档

- [开发任务 - WASI 集成](../task/XX-wasi-integration.md)（如果存在）
- [架构设计 - WASI 实现](../spec/XX-wasi-integration.md)（如果存在）
- [Tarbox 文件系统核心](../spec/02-fuse-interface.md)

---

## 外部资源

- [WASI Preview 2 规范](https://github.com/WebAssembly/WASI/tree/main/wasip2)
- [Wasmtime 文档](https://docs.wasmtime.dev/)
- [Rust wasm32-wasi 编译指南](https://doc.rust-lang.org/rustc/platform-support/wasm32-wasi.html)

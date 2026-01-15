# 项目依赖设计

## 概述

本文档定义 Tarbox 项目的依赖选择、版本管理策略和依赖使用原则。

## Rust 工具链

### 版本要求

```toml
[toolchain]
channel = "1.92"
edition = "2024"
```

**选择理由：**
- Rust 1.92：最新稳定版，提供最佳性能和新特性
- Edition 2024：最新版本标准，提供改进的模块系统和语法

### 必需组件

```
- rustfmt：代码格式化
- clippy：代码检查和 lint
- rust-analyzer：IDE 支持（开发时）
```

## 核心依赖

### 异步运行时

```toml
[dependencies]
tokio = { version = "1.40", features = ["full"] }
```

**用途：**
- 异步 I/O 处理
- 并发任务调度
- 定时器和通道

**选择理由：**
- 最成熟的 Rust 异步运行时
- 完善的生态系统
- 高性能和可靠性

**替代方案：**
- async-std：更简单但生态较小
- smol：轻量但功能有限

### 数据库访问

```toml
[dependencies]
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres", "uuid", "chrono", "json"] }
```

**用途：**
- PostgreSQL 连接和查询
- 编译时 SQL 检查
- 连接池管理

**特性说明：**
- `runtime-tokio`：使用 Tokio 运行时
- `postgres`：PostgreSQL 驱动
- `uuid`：UUID 类型支持
- `chrono`：时间类型支持
- `json`：JSONB 类型支持

**选择理由：**
- 编译时 SQL 验证
- 异步设计
- 类型安全

**替代方案：**
- diesel：同步，更成熟但不支持异步
- sea-orm：ORM，但我们需要更底层的控制

### FUSE 接口

```toml
[dependencies]
fuser = "0.14"
```

**用途：**
- FUSE 文件系统实现
- POSIX 接口适配

**选择理由：**
- 纯 Rust 实现
- 活跃维护
- 跨平台支持

**替代方案：**
- polyfuse：更底层但使用复杂
- fuse-rs：已废弃

### 序列化/反序列化

```toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.8"
```

**用途：**
- 配置文件解析（TOML）
- API 数据序列化（JSON）
- 内部数据结构序列化

**选择理由：**
- Rust 生态标准
- 高性能
- 丰富的派生宏

### UUID 生成

```toml
[dependencies]
uuid = { version = "1.10", features = ["v4", "serde"] }
```

**用途：**
- 生成 layer_id、block_id 等唯一标识符

**特性说明：**
- `v4`：随机 UUID 生成
- `serde`：序列化支持

### 时间处理

```toml
[dependencies]
chrono = { version = "0.4", features = ["serde"] }
```

**用途：**
- 时间戳处理
- 时间格式化
- 时间计算

### 日志

```toml
[dependencies]
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
```

**用途：**
- 结构化日志
- 分布式追踪
- 性能分析

**特性说明：**
- `env-filter`：环境变量过滤
- `json`：JSON 格式输出

**选择理由：**
- 比 log 更强大
- 支持异步
- 结构化数据

### 错误处理

```toml
[dependencies]
anyhow = "1.0"
thiserror = "1.0"
```

**用途：**
- `anyhow`：应用层错误处理
- `thiserror`：库层错误定义

**选择理由：**
- 简化错误处理
- 保留错误上下文
- 社区标准

## 功能性依赖

### HTTP 服务器（REST API）

```toml
[dependencies]
axum = "0.7"
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "trace"] }
```

**用途：**
- REST API 服务
- 中间件支持
- CORS 和追踪

**选择理由：**
- 基于 Tokio
- 类型安全的路由
- 高性能

**替代方案：**
- actix-web：性能更高但生态较小
- rocket：易用但同步

### gRPC

```toml
[dependencies]
tonic = "0.12"
prost = "0.13"

[build-dependencies]
tonic-build = "0.12"
```

**用途：**
- gRPC 服务实现
- Protocol Buffers 序列化

**选择理由：**
- Rust 原生 gRPC
- 与 Tokio 集成
- 高性能

### Kubernetes CSI

```toml
[dependencies]
k8s-openapi = { version = "0.22", features = ["v1_30"] }
kube = { version = "0.93", features = ["client", "runtime"] }
```

**用途：**
- Kubernetes API 交互
- CSI 驱动实现

**特性说明：**
- `v1_30`：Kubernetes 1.30 API
- `client`：Kubernetes 客户端
- `runtime`：运行时支持

### 文本差异算法

```toml
[dependencies]
similar = "2.6"
```

**用途：**
- 文本文件 diff 计算
- 行级差异分析

**选择理由：**
- 实现 Myers diff 算法
- 高性能
- 易于使用

### 哈希和加密

```toml
[dependencies]
sha2 = "0.10"
blake3 = "1.5"
```

**用途：**
- `sha2`：SHA-256 校验和（内容寻址）
- `blake3`：快速哈希（去重）

**选择理由：**
- 广泛使用的标准算法
- 纯 Rust 实现
- 高性能

### 缓存

```toml
[dependencies]
moka = { version = "0.12", features = ["future"] }
```

**用途：**
- LRU 缓存实现
- 异步缓存支持

**特性说明：**
- `future`：异步 API 支持

**选择理由：**
- 高性能
- 支持异步
- TTL 和容量淘汰

**替代方案：**
- lru：同步，功能简单

### 配置管理

```toml
[dependencies]
config = "0.14"
```

**用途：**
- 多来源配置加载（文件、环境变量）
- 配置合并和覆盖

### CLI 工具

```toml
[dependencies]
clap = { version = "4.5", features = ["derive", "env"] }
```

**用途：**
- 命令行参数解析
- 子命令支持

**特性说明：**
- `derive`：派生宏
- `env`：环境变量支持

**选择理由：**
- 功能最完整
- 类型安全
- 自动生成帮助

### 指标导出（Prometheus）

```toml
[dependencies]
prometheus = "0.13"
```

**用途：**
- Prometheus 指标导出
- 性能监控

## 开发依赖

```toml
[dev-dependencies]
# 测试框架
tokio-test = "0.4"

# 模拟和测试
mockall = "0.13"

# 基准测试
criterion = "0.5"

# 测试工具
proptest = "1.5"
```

**用途：**
- `tokio-test`：异步测试工具
- `mockall`：Mock 对象生成
- `criterion`：性能基准测试
- `proptest`：属性测试

## 构建依赖

```toml
[build-dependencies]
tonic-build = "0.12"
```

**用途：**
- 从 .proto 文件生成 Rust 代码

## 平台特定依赖

### Linux

```toml
[target.'cfg(target_os = "linux")'.dependencies]
libc = "0.2"
```

### macOS

```toml
[target.'cfg(target_os = "macos")'.dependencies]
libc = "0.2"
```

## 可选特性

```toml
[features]
default = ["rest-api", "grpc-api", "csi-driver"]

# REST API 支持
rest-api = ["axum", "tower", "tower-http"]

# gRPC 支持
grpc-api = ["tonic", "prost"]

# Kubernetes CSI 驱动
csi-driver = ["k8s-openapi", "kube"]

# 性能分析
profiling = ["tracing/max_level_trace"]

# 开发模式（额外日志）
dev = ["tracing/debug"]
```

## 依赖管理原则

### 版本选择

```
1. 优先使用稳定版本
2. 关键依赖锁定小版本（如 tokio = "1.40"）
3. 工具类依赖可以宽松（如 clap = "4"）
4. 定期更新依赖（每月检查）
```

### 安全性

```
1. 定期运行 cargo audit 检查漏洞
2. 关注依赖的安全公告
3. 避免使用废弃的 crate
4. 审查新增依赖的代码质量
```

### 性能考虑

```
1. 优先选择零成本抽象的库
2. 避免重复依赖（检查 cargo tree）
3. 使用 features 减少不必要的编译
4. 关注编译时间影响
```

### 兼容性

```
1. 检查 MSRV（最低支持 Rust 版本）
2. 确保跨平台兼容性
3. 版本更新前测试完整功能
4. 保持 Cargo.lock 在版本控制中
```

## 依赖审查清单

新增依赖前需要检查：

```
□ 是否有积极维护（最近 6 个月有更新）
□ 是否有足够的社区支持（GitHub stars > 100）
□ 是否有良好的文档
□ 是否有测试覆盖
□ 许可证是否兼容（MIT/Apache-2.0）
□ 是否有已知的安全问题
□ 是否与现有依赖冲突
□ 编译时间影响是否可接受
```

## 依赖更新策略

### 定期更新

```bash
# 每月执行
cargo update

# 检查过期依赖
cargo outdated

# 检查安全漏洞
cargo audit
```

### 主版本更新

```
1. 阅读 CHANGELOG
2. 创建专门的分支
3. 运行完整测试套件
4. 性能基准测试
5. 代码审查
6. 分阶段部署
```

### 锁定策略

```toml
# 关键依赖锁定完整版本
tokio = "=1.40.0"
sqlx = "=0.8.0"

# 工具依赖可以宽松
clap = "4"
serde = "1"
```

## 编译优化

### Release 配置

```toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true
```

### 依赖编译优化

```toml
[profile.dev.package."*"]
opt-level = 2
```

**说明：**
- 开发时依赖使用优化编译
- 加速开发时的运行速度
- 不影响项目代码的调试

## 供应商化（Vendoring）

对于生产部署，考虑依赖供应商化：

```bash
# 下载所有依赖到 vendor 目录
cargo vendor

# 配置使用本地依赖
# .cargo/config.toml:
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
```

**优势：**
- 离线构建
- 构建可重现性
- 减少外部依赖风险

## 许可证合规

### 允许的许可证

```
- MIT
- Apache-2.0
- BSD-3-Clause
- BSD-2-Clause
- ISC
```

### 检查工具

```bash
# 安装
cargo install cargo-license

# 检查所有依赖的许可证
cargo license
```

## 依赖树管理

### 分析依赖

```bash
# 查看依赖树
cargo tree

# 查看特定包的依赖
cargo tree -p tokio

# 查看重复依赖
cargo tree --duplicates
```

### 减少依赖

```
1. 使用 features 精确控制
2. 避免不必要的传递依赖
3. 考虑替换重量级依赖
4. 定期清理未使用的依赖
```

## 未来依赖考虑

### 可能添加的依赖

```
- notify：文件系统变化监听
- rayon：数据并行处理
- parking_lot：更快的锁实现
- bytes：字节缓冲区管理
- memmap2：内存映射文件
- zstd：压缩支持
```

### 评估中的依赖

```
- tantivy：全文搜索（如果需要）
- arrow：列式数据处理（大数据场景）
- rdkafka：Kafka 集成（消息队列）
```

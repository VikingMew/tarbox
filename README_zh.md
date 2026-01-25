<div align="center">

# 🗄️ Tarbox

**基于 PostgreSQL 的文件系统，为 AI Agent 提供版本控制和审计日志**

[![CI](https://github.com/VikingMew/tarbox/workflows/CI/badge.svg)](https://github.com/VikingMew/tarbox/actions/workflows/ci.yml)
[![E2E Tests](https://github.com/VikingMew/tarbox/workflows/E2E%20Tests/badge.svg)](https://github.com/VikingMew/tarbox/actions/workflows/e2e.yml)
[![License: MPL-2.0](https://img.shields.io/badge/License-MPL2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.92%2B-orange.svg)](https://www.rust-lang.org)
[![PostgreSQL](https://img.shields.io/badge/PostgreSQL-16%2B-336791.svg)](https://www.postgresql.org)

[快速开始](#-快速开始) • [功能特性](#-功能特性) • [架构设计](#-架构设计) • [文档](#-文档)

[English](README.md)

</div>

---

## Tarbox 是什么？

Tarbox 是一个将所有数据存储在 PostgreSQL 中的 FUSE 文件系统。专为需要以下特性的 AI Agent 设计：

- **可靠存储** - PostgreSQL ACID 保证
- **版本控制** - Docker 风格的层和 Git 风格的文本差异
- **审计日志** - 追踪每个文件操作
- **多租户** - 每个租户完全数据隔离
- **云原生** - 准备好 Kubernetes 部署

**当前状态**：核心文件系统和分层文件系统已可用于生产。审计集成和性能优化等高级功能是下一步的路线图。

**平台支持**：完全支持 Linux。由于 `fuser` crate 的限制，macOS 支持尚不完整（需要 macFUSE 和条件编译）。详见 [Task 17](task/17-macos-fuse-support.md)。

---

## ✨ 功能特性

### ✅ 可用于生产

- **标准 POSIX 接口**：通过 FUSE 挂载为常规文件系统，使用任何 Unix 工具（vim、git、make 等）
- **PostgreSQL 存储**：ACID 保证，使用 BLAKE3 的内容寻址去重，支持标准 SQL 查询
- **多租户隔离**：每个租户完全数据分离，安全共享基础设施
- **版本控制**：Docker 风格的层和 Git 风格的文本差异，可检查点和恢复任意状态
- **智能文件存储**：文本文件行级 COW，二进制文件块级 COW，自动编码检测
- **CLI + 挂载**：通过命令行管理或挂载为标准目录

### 🚧 开发中

- **审计日志**：追踪每个文件操作以满足合规要求（数据库 schema 已完成，集成进行中）
- **性能优化**：元数据和数据块的 LRU 缓存，查询调优

---

## 🚀 快速开始

### 环境要求

- PostgreSQL 16+
- FUSE3 (Linux: `libfuse3-dev`)
- Protocol Buffers 编译器 (Linux: `protobuf-compiler`)
- Rust 1.92+（仅原生构建需要）

### 方式一：Docker Compose（推荐）

最简单的上手方式，包含 PostgreSQL 和所有依赖。

```bash
# 克隆仓库
git clone https://github.com/vikingmew/tarbox.git
cd tarbox

# 启动 PostgreSQL
docker-compose up -d postgres

# 通过 Docker 运行 tarbox CLI
docker-compose run --rm tarbox-cli tarbox init
docker-compose run --rm tarbox-cli tarbox tenant create myagent
docker-compose run --rm tarbox-cli tarbox --tenant myagent ls /

# 可选：启动 pgAdmin 进行数据库管理
docker-compose --profile tools up -d pgadmin
# 访问 http://localhost:5050 (admin@tarbox.local / admin)
```

### 方式二：原生构建

直接在本机构建和运行，需要 Rust 工具链。

```bash
# 克隆并构建
git clone https://github.com/vikingmew/tarbox.git
cd tarbox
cargo build --release

# 设置 PostgreSQL（选择一种）：
# A) 使用现有 PostgreSQL 实例
# B) 使用 Docker 启动
docker-compose up -d postgres

# 配置数据库连接
export DATABASE_URL=postgres://postgres:postgres@localhost:5432/tarbox

# 初始化并运行
./target/release/tarbox init
./target/release/tarbox tenant create myagent
```

### 基础使用

```bash
# CLI 文件操作
tarbox --tenant myagent mkdir /workspace
tarbox --tenant myagent write /workspace/config.txt "key=value"
tarbox --tenant myagent cat /workspace/config.txt
tarbox --tenant myagent ls /workspace

# 挂载为 FUSE 文件系统（需要 FUSE 权限）
tarbox --tenant myagent mount /mnt/tarbox
echo "test" > /mnt/tarbox/workspace/test.txt
ls -la /mnt/tarbox/workspace

# 使用层系统（自动快照）
echo "version 1" > /mnt/tarbox/workspace/app.py
echo "checkpoint1" > /mnt/tarbox/.tarbox/layers/new  # 创建检查点
echo "version 2" > /mnt/tarbox/workspace/app.py
cat /mnt/tarbox/.tarbox/layers/list                  # 查看层历史

tarbox umount /mnt/tarbox
```

---

## 🏗️ 架构设计

```mermaid
graph TB
    Apps[应用程序 / AI Agents]
    FUSE[FUSE 接口<br/>POSIX 操作]
    
    subgraph Core[Tarbox 核心]
        FS[文件系统层<br/>• 路径解析<br/>• Inode 管理<br/>• 权限控制]
        Storage[存储层<br/>• 租户与 inodes<br/>• 数据块<br/>• 层与审计]
    end
    
    DB[(PostgreSQL<br/>• tenants, inodes, blocks<br/>• layers, audit_logs<br/>• text_blocks)]
    
    Apps --> FUSE
    FUSE --> FS
    FS --> Storage
    Storage --> DB
    
    style Apps fill:#e1f5ff
    style FUSE fill:#fff3e0
    style Core fill:#f3e5f5
    style DB fill:#e8f5e9
```

### 关键设计决策

- **FUSE 而非内核模块**：更易开发和调试
- **PostgreSQL 而非基于文件**：ACID 保证、多租户、查询能力
- **内容寻址存储**：使用 BLAKE3 哈希去重
- **异步 Rust**：使用 tokio 运行时的高性能 I/O
- **Repository 模式**：文件系统与存储层的清晰分离

---

## 📖 文档

### 用户文档

- **[快速开始](#-快速开始)** - 5 分钟上手（见上文）
- **[CLI 参考](#cli-参考)** - 所有命令和选项
- **[配置](CLAUDE.md#configuration)** - 数据库和文件系统设置
- **[Kubernetes CSI 指南](docs/csi-usage.md)** - 部署为 K8s 存储插件
- **[WASI 适配器指南](docs/wasi-usage.md)** - WASI 文件系统后端（crate 开发中）

### 开发者文档

- **[架构概览](spec/00-overview.md)** - 系统设计和理念
- **[数据库 Schema](spec/01-database-schema.md)** - PostgreSQL 表定义
- **[FUSE 接口](spec/02-fuse-interface.md)** - POSIX 操作映射
- **[开发指南](CLAUDE.md)** - 环境搭建和编码规范
- **[贡献指南](CONTRIBUTING.md)** - 如何参与贡献

---

## 🛠️ CLI 参考

```bash
# 数据库初始化
tarbox init                                    # 创建数据库 schema

# 租户管理
tarbox tenant create <name>                    # 创建新租户
tarbox tenant list                             # 列出所有租户
tarbox tenant info <name>                      # 显示租户详情
tarbox tenant delete <name>                    # 删除租户

# 文件操作（所有命令都需要 --tenant <name>）
tarbox --tenant <name> mkdir <path>            # 创建目录
tarbox --tenant <name> rmdir <path>            # 删除空目录
tarbox --tenant <name> ls [path]               # 列出目录内容
tarbox --tenant <name> touch <path>            # 创建空文件
tarbox --tenant <name> write <path> <content>  # 写入文件
tarbox --tenant <name> cat <path>              # 读取文件
tarbox --tenant <name> rm <path>               # 删除文件
tarbox --tenant <name> stat <path>             # 显示文件元数据

# FUSE 挂载
tarbox --tenant <name> mount <mountpoint>      # 挂载文件系统
tarbox --tenant <name> mount <mp> --read-only  # 只读挂载
tarbox --tenant <name> mount <mp> --allow-other # 允许所有用户访问
tarbox umount <mountpoint>                     # 卸载文件系统

# 层管理（通过虚拟文件系统钩子）
# 挂载后，在 /.tarbox/ 上使用标准文件操作
cat /.tarbox/layers/current                    # 显示当前层
cat /.tarbox/layers/list                       # 列出所有层
echo "checkpoint1" > /.tarbox/layers/new       # 创建检查点
echo "<layer-id>" > /.tarbox/layers/switch     # 切换到层
cat /.tarbox/layers/tree                       # 显示层树
cat /.tarbox/stats/usage                       # 显示存储统计
```

---

## 🧪 开发

### 构建和测试

```bash
# 构建
cargo build
cargo build --release

# 运行测试
cargo test --lib                               # 单元测试（快速）
cargo test                                     # 所有测试（需要 PostgreSQL）

# 代码质量
cargo fmt --all                                # 格式化代码
cargo clippy --all-targets -- -D warnings      # 代码检查

# 提交前检查
cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --lib
```

---

## 比较

### vs AgentFS

[AgentFS](https://github.com/tursodatabase/agentfs) 是一个基于 SQLite 的 AI Agent 文件系统。在以下场景选择 Tarbox：
- **运行多个 Agent**，需要在共享基础设施上隔离工作空间
- **服务端部署**，PostgreSQL 已在你的技术栈中
- **细粒度版本控制**，针对文本文件（代码、配置、日志）
- **Kubernetes/云原生** 环境，需要水平扩展
- **合规要求**，需要集中式审计日志

---

## 📊 性能

设计为高性能：

- **预编译语句**：所有 PostgreSQL 查询使用预编译
- **连接池**：可配置的连接限制
- **内容寻址**：去重优化
- **异步 I/O**：使用 tokio 运行时
- **LRU 缓存**：元数据和数据块缓存（计划中）

性能基准测试即将推出。

---

## 🤝 参与贡献

欢迎贡献！请查看 [CONTRIBUTING.md](CONTRIBUTING.md) 了解：

- 行为准则
- 开发工作流
- 测试要求（>80% 覆盖率）
- 代码风格指南

### 快速贡献指南

1. Fork 仓库
2. 创建功能分支
3. 进行修改
4. 运行测试和代码检查
5. 提交 Pull Request

---

## 📜 许可证

本项目采用 [Mozilla Public License 2.0](LICENSE) 许可证。

---

## 🙏 致谢

基于 PostgreSQL、Rust 和 FUSE 构建。灵感来自 Docker 的分层文件系统和 Git 的内容寻址。

---

<div align="center">

**[⬆ 返回顶部](#-tarbox)**

为 AI Agent 用心打造 ❤️

</div>

<div align="center">

# 🗄️ Tarbox

**基于 PostgreSQL 的文件系统，为 AI Agent 提供版本控制和审计日志**

[![CI](https://github.com/VikingMew/tarbox/workflows/CI/badge.svg)](https://github.com/VikingMew/tarbox/actions/workflows/ci.yml)
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

**当前状态**：核心文件系统和分层文件系统已可用于生产（370+ 测试，75% 覆盖率）。审计集成和性能优化等高级功能是下一步的路线图。

---

## ✨ 功能特性

### ✅ 可用于生产

- **POSIX 文件系统**：通过 FUSE 提供标准文件操作（创建、读取、写入、删除）
- **PostgreSQL 后端**：ACID 保证，使用 BLAKE3 的内容寻址存储
- **多租户**：完全隔离的租户命名空间
- **CLI 工具**：从命令行管理租户和文件
- **FUSE 挂载**：挂载为标准文件系统，使用任何 Unix 工具
- **分层文件系统**：Docker 风格的快照和 COW
  - ✅ 自动创建 base layer
  - ✅ Checkpoint 创建和切换
  - ✅ 文本文件：行级 COW 和差异计算
  - ✅ 二进制文件：块级 COW（4KB 块）
  - ✅ 虚拟文件系统钩子（`/.tarbox/layers/`）
  - ✅ 跨层链的联合视图
- **文件类型检测**：自动文本/二进制分类
  - ✅ UTF-8/ASCII/Latin-1 编码检测
  - ✅ 行结束符检测（LF/CRLF/CR/Mixed）
  - ✅ 基于内容的分类

### 🚧 开发中

- **审计日志**：操作追踪和合规报告
  - ✅ 数据库 schema 和操作
  - ⏳ 与文件操作集成
- **性能优化**：缓存和查询优化
  - ⏳ 元数据和数据块的 LRU 缓存
  - ⏳ 查询优化和索引

---

## 🚀 快速开始

### 环境要求

- Rust 1.92+ (Edition 2024)
- PostgreSQL 16+
- FUSE3 (Linux: `libfuse3-dev`, macOS: `macfuse`)

### 安装

```bash
# 克隆并构建
git clone https://github.com/vikingmew/tarbox.git
cd tarbox
cargo build --release

# 启动 PostgreSQL（或使用现有实例）
docker-compose up -d postgres

# 初始化数据库 schema
export DATABASE_URL=postgres://postgres:postgres@localhost:5432/tarbox
./target/release/tarbox init
```

### 基础使用

```bash
# 创建租户
tarbox tenant create myagent

# 使用 CLI 命令
tarbox --tenant myagent mkdir /workspace
tarbox --tenant myagent write /workspace/config.txt "key=value"
tarbox --tenant myagent cat /workspace/config.txt
tarbox --tenant myagent ls /workspace

# 挂载为文件系统并使用标准工具
tarbox --tenant myagent mount /mnt/tarbox
echo "test" > /mnt/tarbox/workspace/test.txt
vim /mnt/tarbox/workspace/code.py
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

### 测试覆盖率

- **单元测试**：198 个测试（纯函数、数据结构）
- **集成测试**：160+ 个测试（数据库操作、层系统、FUSE 逻辑）
- **E2E 测试**：11 个测试（需要 PostgreSQL + FUSE，在 CI 中运行）
- **总计**：370+ 测试，0 失败
- **覆盖率**：75.27% 总体（核心层模块 >90%）

### 项目结构

```
tarbox/
├── src/
│   ├── types.rs           # 核心类型别名
│   ├── config/            # 配置系统
│   ├── storage/           # PostgreSQL 层（repositories、migrations）
│   ├── fs/                # 文件系统核心（path、operations、permissions）
│   ├── fuse/              # FUSE 接口
│   └── main.rs            # CLI 入口
├── spec/                  # 架构设计文档
├── task/                  # 开发任务和进度
├── tests/                 # 集成和 E2E 测试
└── migrations/            # 数据库 schema 迁移
```

---

## 🗺️ 开发路线图

### ✅ 阶段 1：核心文件系统（已完成）

- [x] PostgreSQL 存储后端
- [x] 多租户数据隔离
- [x] POSIX 文件操作
- [x] FUSE 挂载支持
- [x] CLI 工具

### ✅ 阶段 2：高级存储 Schema（已完成）

- [x] 审计日志表（时间分区）
- [x] 层管理表（链式查询）
- [x] 文本优化表（内容寻址）
- [x] Repository 实现（3 个模块，22 个方法）
- [x] 全面测试（198 单元 + 160+ 集成）

### ✅ 阶段 3：分层文件系统（已完成）

- [x] 文件类型检测（文本/二进制、编码、行结束符）
- [x] COW 实现（文本：行级，二进制：块级）
- [x] 层管理（创建、切换、删除、历史）
- [x] 文本差异计算和存储（使用 similar crate）
- [x] 文件系统钩子（`/.tarbox/layers/*`）
- [x] 跨层链的联合视图
- [x] FileSystem 集成和自动 base layer
- [x] 52 个新的层功能测试

### 🚧 阶段 4：生产特性（进行中）

- [ ] 审计日志与文件操作集成
- [ ] 性能优化（LRU 缓存、查询调优）
- [ ] 高级 POSIX 特性（链接、xattr）
- [ ] 覆盖率提升到 80%+

### 📋 阶段 5：云原生（计划中）

- [ ] Kubernetes CSI 驱动
- [ ] REST/gRPC API
- [ ] 监控和指标（Prometheus）
- [ ] Web 管理界面

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

## 📊 性能

设计为高性能：

- **预编译语句**：所有 PostgreSQL 查询使用预编译
- **连接池**：可配置的连接限制
- **内容寻址**：去重优化
- **异步 I/O**：使用 tokio 运行时
- **LRU 缓存**：元数据和数据块缓存（计划中）

性能基准测试即将推出。

---

## 📜 许可证

双重许可：MIT 或 Apache 2.0，任选其一。

---

## 🙏 致谢

基于 PostgreSQL、Rust 和 FUSE 构建。灵感来自 Docker 的分层文件系统和 Git 的内容寻址。

---

<div align="center">

**[⬆ 返回顶部](#-tarbox)**

为 AI Agent 用心打造 ❤️

</div>

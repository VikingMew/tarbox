<div align="center">

# 🗄️ Tarbox

**基于 PostgreSQL 的 AI Agent 分布式文件系统**

[![License: MPL-2.0](https://img.shields.io/badge/License-MPL2.0-blue.svg)](LICENSE)
[![Rust Version](https://img.shields.io/badge/rust-1.92%2B-orange.svg)](https://www.rust-lang.org)
[![PostgreSQL](https://img.shields.io/badge/PostgreSQL-14%2B-336791.svg)](https://www.postgresql.org)

[功能特性](#-功能特性) • [快速开始](#-快速开始) • [架构设计](#-架构设计) • [文档](#-文档) • [参与贡献](#-参与贡献)

[English](README.md)

</div>

---

## 📖 项目概述

Tarbox 是一个高性能的文件系统实现,使用 PostgreSQL 作为存储后端,专为需要可靠、可审计、版本控制文件存储的 AI Agent 设计。

**✅ 当前状态：核心功能完成**

Tarbox 已完成核心文件系统实现。PostgreSQL 存储后端、CLI 工具和 FUSE 挂载功能已全部可用。分层文件系统、审计系统和 Kubernetes 集成等高级功能正在开发中。

### 为什么选择 Tarbox?

传统文件系统缺乏现代 AI Agent 所需的可审计性、版本控制和多租户功能。Tarbox 旨在通过以下特性弥补这一差距:

- **数据库可靠性**: PostgreSQL 的 ACID 特性确保数据一致性
- **版本控制**: (计划中) Docker 风格的分层与 Git 风格的文本文件优化
- **多租户**: 不同 AI Agent 之间完全隔离
- **云原生**: (计划中) 内置 Kubernetes CSI 驱动,无缝部署
- **可审计性**: (计划中) 每个文件操作都被记录,用于合规和调试

---

## ✨ 功能特性

### ✅ 当前可用功能

- **🐘 PostgreSQL 存储后端**
  - ACID 保证数据一致性
  - 完全隔离的多租户数据
  - 元数据和数据块存储
  - 基于 BLAKE3 的内容寻址存储

- **📁 完整文件操作**
  - 目录操作(创建、列出、删除)
  - 文件操作(创建、读取、写入、删除)
  - 路径解析和验证
  - 元数据操作(stat、chmod、chown)

- **🔧 命令行工具**
  - 租户管理(创建、列出、删除、查看信息)
  - 文件系统操作(mkdir、ls、rm、cat、write、stat)
  - 数据库初始化
  - FUSE 挂载和卸载
  - 通过环境变量配置

- **📂 FUSE 挂载支持**
  - 作为标准 POSIX 文件系统挂载
  - 与 Unix 工具完全兼容(ls、cat、vim 等)
  - 只读或读写模式
  - 多用户访问控制
  - 适用于任何 FUSE 兼容应用程序

### 🚧 即将推出

- **🔍 完整审计追踪**
  - 每个文件操作都记录元数据
  - 按时间分区的审计表,高效查询
  - 追踪所有变更的版本历史
  - 支持合规性报告

- **🐳 Docker 风格分层文件系统**
  - 即时创建检查点和快照
  - 写时复制(Copy-on-Write)高效存储
  - 线性历史模型,快速层切换
  - 通过文件系统钩子控制(如 `echo "checkpoint" > /.tarbox/layers/new`)

- **📝 Git 风格文本文件优化**
  - 对文本文件(CSV, Markdown, YAML, 代码等)进行行级差异存储
  - 跨文件和跨层的内容去重
  - 高效版本比较和差异对比
  - 对应用程序完全透明

- **📁 高级 POSIX 特性**
  - 支持符号链接和硬链接
  - 扩展属性(xattr)
  - 文件锁定机制
  - 高级权限系统

- **☸️ Kubernetes 集成**
  - 原生 CSI(容器存储接口)驱动
  - 动态卷配置
  - 基础设施级别的多租户隔离
  - 支持快照和备份

- **🌐 现代接口**
  - REST API 远程管理
  - gRPC API 高性能访问
  - WASI 支持 WebAssembly 环境
  - Web 管理界面
  - 在边缘计算环境中运行
  - 浏览器内文件系统
  - Serverless 函数集成

---

## 🏗️ 架构设计

```mermaid
graph TB
    Apps[应用程序 / AI Agent]
    FUSE[FUSE 接口<br/>POSIX 文件操作]
    
    subgraph Core[Tarbox 核心引擎]
        FS[文件系统层<br/>• Inode 管理<br/>• 目录树<br/>• 权限控制<br/>• 原生挂载路由]
        Layer[分层文件系统<br/>• 层管理 创建/切换<br/>• 写时复制 COW<br/>• 检查点和快照]
        Audit[审计与缓存<br/>• 操作日志<br/>• 多级 LRU 缓存<br/>• 版本追踪]
    end
    
    DB[(PostgreSQL 存储后端<br/>• 元数据表 inodes, layers<br/>• 数据块 二进制和文本<br/>• 审计日志 按时间分区<br/>• 原生挂载配置)]
    
    Apps --> FUSE
    FUSE --> Core
    FS --> DB
    Layer --> DB
    Audit --> DB
    
    style Apps fill:#e1f5ff
    style FUSE fill:#fff3e0
    style Core fill:#f3e5f5
    style DB fill:#e8f5e9
```

### 模块结构

```
src/
├── types.rs        # 核心类型别名(InodeId, LayerId, TenantId)
├── config/         # 配置系统(TOML + 环境变量)
├── storage/        # PostgreSQL 层(所有数据库操作)
├── fs/             # 文件系统核心(路径解析、文件操作)
├── fuse/           # FUSE 接口(异步到同步桥接)
├── layer/          # 分层文件系统(COW、检查点)
├── native/         # 原生挂载管理
├── audit/          # 审计日志(异步批量插入)
├── cache/          # 缓存层(基于 moka 的 LRU)
├── api/            # REST/gRPC API
└── k8s/            # Kubernetes CSI 驱动
```

---

## 🚀 快速开始

### 前置要求

- **Rust**: 1.92+ (Edition 2024)
- **PostgreSQL**: 14+
- **FUSE**: libfuse3 (Linux) 或 macFUSE (macOS)

### 安装

#### 方式一：使用 Docker Compose（推荐用于开发）

```bash
# 克隆仓库
git clone https://github.com/vikingmew/tarbox.git
cd tarbox

# 启动 PostgreSQL 数据库
docker-compose up -d postgres

# 初始化数据库
export DATABASE_URL=postgres://postgres:postgres@localhost:5432/tarbox
cargo run -- init

# 或使用 CLI 容器
docker-compose run --rm tarbox-cli tarbox init
```

参见 [Docker Compose 使用指南](docs/docker-compose.md) 获取详细说明。

#### 方式二：从源码构建

```bash
# 克隆仓库
git clone https://github.com/vikingmew/tarbox.git
cd tarbox

# 从源码构建
cargo build --release

# 安装(可选)
cargo install --path .
```

### 基础使用（当前 MVP 功能）

```bash
# 初始化数据库
tarbox init

# 为你的 AI Agent 创建租户
tarbox tenant create myagent

# 创建目录结构
tarbox --tenant myagent mkdir /data
tarbox --tenant myagent mkdir /data/logs

# 列出目录
tarbox --tenant myagent ls /
tarbox --tenant myagent ls /data

# 创建和写入文件
tarbox --tenant myagent touch /data/config.txt
tarbox --tenant myagent write /data/config.txt "key=value"

# 读取文件内容
tarbox --tenant myagent cat /data/config.txt

# 查看文件信息
tarbox --tenant myagent stat /data/config.txt

# 删除文件和目录
tarbox --tenant myagent rm /data/config.txt
tarbox --tenant myagent rmdir /data/logs

# 租户管理
tarbox tenant list
tarbox tenant info myagent
tarbox tenant delete myagent
```

**注意**：分层文件系统、审计日志和文本优化等高级功能尚未实现。查看[路线图](#-路线图)了解计划功能。

### CLI 命令（当前可用）

```bash
# 数据库初始化
tarbox init                                    # 初始化数据库模式

# 租户管理
tarbox tenant create <name>                    # 创建新租户
tarbox tenant info <name>                      # 显示租户信息
tarbox tenant list                             # 列出所有租户
tarbox tenant delete <name>                    # 删除租户

# 文件操作（需要 --tenant <name>）
tarbox --tenant <name> mkdir <path>            # 创建目录
tarbox --tenant <name> ls [path]               # 列出目录（默认：/）
tarbox --tenant <name> rmdir <path>            # 删除空目录
tarbox --tenant <name> touch <path>            # 创建空文件
tarbox --tenant <name> write <path> <content>  # 写入文件内容
tarbox --tenant <name> cat <path>              # 读取文件内容
tarbox --tenant <name> rm <path>               # 删除文件
tarbox --tenant <name> stat <path>             # 显示文件信息

# FUSE 挂载（新增功能 ✅）
tarbox --tenant <name> mount <mountpoint>      # 通过 FUSE 挂载文件系统
tarbox --tenant <name> mount <mountpoint> --allow-other  # 允许其他用户访问
tarbox --tenant <name> mount <mountpoint> --read-only    # 以只读模式挂载
tarbox umount <mountpoint>                     # 卸载文件系统

# 示例：挂载后使用标准 Unix 工具访问
tarbox --tenant myagent mount /mnt/tarbox
ls /mnt/tarbox                                 # 使用标准 ls 命令
cat /mnt/tarbox/data/config.txt                # 使用标准 cat 命令
echo "hello" > /mnt/tarbox/data/test.txt       # 使用标准 shell 重定向
vim /mnt/tarbox/data/code.py                   # 使用任何文本编辑器
tarbox umount /mnt/tarbox                      # 完成后卸载
```

**计划中的命令**（开发中）:

```bash
# 层操作（快照和版本控制）
tarbox layer list --tenant <name>
tarbox layer create --tenant <name> --message "更新前的检查点"
tarbox layer switch --tenant <name> --layer <id>
tarbox layer diff --layer1 <id1> --layer2 <id2>

# 审计查询（操作历史）
tarbox audit --tenant <name> --since "1 day ago"
tarbox audit --path <path> --operation write
tarbox audit --export --format json > audit.json
```

---

## 📚 文档

### 用户文档

- **[快速开始](#-快速开始)** - 5 分钟上手（见上文）
- **[CLI 参考](#cli-命令当前可用)** - 完整命令文档（见上文）
- **[配置](CLAUDE.md)** - 开发配置指南

### 开发者文档

- **[架构概览](spec/00-overview.md)** - 系统设计和理念
- **[数据库模式](spec/01-database-schema.md)** - PostgreSQL 表定义
- **[FUSE 接口](spec/02-fuse-interface.md)** - POSIX 操作映射
- **[分层文件系统](spec/04-layered-filesystem.md)** - COW 和版本控制
- **[文本优化](spec/10-text-file-optimization.md)** - 行级差异
- **[原生挂载](spec/12-native-mounting.md)** - 性能优化
- **[贡献指南](CONTRIBUTING.md)** - 如何贡献
- **[开发设置](CLAUDE.md)** - 内部开发指南

### 开发状态

**✅ 已完成**
- PostgreSQL 存储后端，ACID 保证
- 完整的文件和目录操作
- 多租户隔离
- 命令行界面
- FUSE 挂载支持

**🚧 开发中**
- 审计日志系统
- 分层文件系统与快照
- 文本文件优化
- 高级权限系统

**📋 已规划**
- Kubernetes CSI 驱动
- REST 和 gRPC API
- Web 管理界面
- WASI 支持 WebAssembly

详细技术路线图请查看 [task/](task/) 目录。

---

## 💡 使用场景

### AI Agent 工作空间

```bash
# 每个 AI Agent 获得隔离的租户
tarbox tenant create agent-001

# Agent 在分层环境中工作
# 在危险操作前创建检查点
echo "checkpoint" > /.tarbox/layers/new

# Agent 修改文件
# 如果出错,立即回滚
echo "<previous-layer>" > /.tarbox/layers/switch
```

### 代码生成追踪

```bash
# 追踪代码生成工具所做的每个更改
tarbox audit --operation write --since "1 hour ago"

# 比较生成代码的前后状态
tarbox layer diff --layer1 <before> --layer2 <after>

# 查看文本文件的逐行更改
tarbox diff /src/generated.py
```

### 多环境开发

```bash
# 通过原生挂载共享只读系统工具
[[native_mounts]]
path = "/usr/bin"
source = "/usr/bin"
mode = "ro"
shared = true

# 租户特定的 Python 虚拟环境
[[native_mounts]]
path = "/.venv"
source = "/var/tarbox/venvs/{tenant_id}"
mode = "rw"
shared = false
```

---

## 🔧 配置

示例 `config.toml`:

```toml
[database]
url = "postgresql://tarbox:password@localhost/tarbox"
pool_size = 20
connection_timeout = "30s"

[filesystem]
block_size = 4096
max_file_size = "10GB"

[cache]
metadata_size = "1GB"
block_size = "4GB"
policy = "lru"

[audit]
enabled = true
retention_days = 90
batch_size = 100

[layer]
auto_checkpoint = false
checkpoint_interval = "1h"

# 原生文件系统挂载
[[native_mounts]]
path = "/bin"
source = "/bin"
mode = "ro"
shared = true
priority = 10

[[native_mounts]]
path = "/.venv"
source = "/var/tarbox/venvs/{tenant_id}"
mode = "rw"
shared = false
priority = 20
```

---

## 🧪 开发

### 构建和测试

```bash
# 构建项目
cargo build

# 运行所有测试
cargo test

# 运行特定测试
cargo test test_name

# 检查代码覆盖率(需要 tarpaulin)
cargo tarpaulin --out Html

# 格式化代码
cargo fmt --all

# 代码检查
cargo clippy --all-targets --all-features -- -D warnings

# 提交前检查(提交前运行)
cargo fmt --all && \
cargo clippy --all-targets --all-features -- -D warnings && \
cargo test
```

### 项目要求

- **测试覆盖率**: 必须 >80% (项目全局要求)
- **Rust Edition**: 2024
- **代码风格**: 遵循 Linus Torvalds 和 John Carmack 原则
  - 简单直接的代码
  - 快速失败的错误处理(使用 `anyhow::Result`)
  - 面向数据的设计
  - 小而专注的函数

### 依赖管理

```bash
# 添加新依赖(永远不要手动编辑 Cargo.toml)
cargo add <crate>
cargo add --dev <crate>  # 开发依赖

# 安全审计
cargo audit

# 许可证和依赖检查
cargo deny check
```

---

## 🤝 参与贡献

我们欢迎贡献!详情请参阅我们的 [贡献指南](CONTRIBUTING.md)。

### 如何贡献

1. Fork 仓库
2. 创建特性分支 (`git checkout -b feature/amazing-feature`)
3. 进行修改
4. 运行测试和检查 (`cargo test && cargo clippy`)
5. 提交更改 (`git commit -m 'Add amazing feature'`)
6. 推送到分支 (`git push origin feature/amazing-feature`)
7. 打开 Pull Request

### 开发交流

- 在 GitHub Issues 上加入我们的讨论
- 阅读 [行为准则](CODE_OF_CONDUCT.md)

---

## 📊 性能

Tarbox 通过智能缓存设计实现高性能:

- **元数据缓存**: 用于 inode 查找的 LRU 缓存
- **块缓存**: 基于内容寻址的块缓存
- **路径缓存**: 缓存的路径解析
- **预编译语句**: 所有 PostgreSQL 查询使用预编译语句
- **批量操作**: 审计日志异步批量写入
- **原生挂载**: 对性能关键路径绕过 PostgreSQL

基准测试结果(即将推出):

```
文件读取 (1MB):      ~50 MB/s
文件写入 (1MB):      ~40 MB/s
元数据操作:          ~5000 ops/s
层切换:              <100ms
文本差异:            ~1M lines/s
```

---

## 🔒 安全性

- **多租户隔离**: 租户之间完全数据分离
- **审计日志**: 每个操作都被记录以符合合规要求
- **权限模型**: 强制执行标准 UNIX 权限
- **默认安全**: 系统目录使用只读原生挂载

安全漏洞报告请参见 [SECURITY.md](SECURITY.md)。

---

## 🗺️ 路线图

### ✅ 核心功能（已完成）

- [x] PostgreSQL 存储后端，ACID 保证
- [x] 多租户数据隔离
- [x] 完整的文件和目录操作
- [x] 命令行管理界面
- [x] FUSE 挂载支持

### 🚧 高级存储（开发中）

- [ ] 完整的审计追踪与时间分区
- [ ] 分层文件系统与写时复制
- [ ] 快照和检查点支持
- [ ] 文本文件优化，行级差异
- [ ] 高级权限系统

### 📋 云原生集成（已规划）

- [ ] Kubernetes CSI 驱动
- [ ] REST API 远程管理
- [ ] gRPC API 高性能访问
- [ ] 监控和指标(Prometheus)

### 🔮 未来增强

- [ ] WASI 支持 WebAssembly
- [ ] Web 管理界面
- [ ] 分布式 PostgreSQL 支持(Citus)
- [ ] 实时复制
- [ ] ML 模型版本控制助手

---

## 📜 许可证

本项目采用双重许可:

- MIT 许可证 ([LICENSE-MIT](LICENSE) 或 http://opensource.org/licenses/MIT)
- Apache 许可证 2.0 版本 ([LICENSE-APACHE](LICENSE) 或 http://www.apache.org/licenses/LICENSE-2.0)

您可以选择任一许可证使用。

---

## 🙏 致谢

- **PostgreSQL 社区**: 提供强大的数据库系统
- **FUSE 项目**: 提供用户空间文件系统能力
- **Rust 社区**: 提供出色的生态系统
- 灵感来自 Docker 的分层文件系统和 Git 的内容寻址

---

## 📞 支持

- **文档**: [完整文档](docs/)
- **问题**: [GitHub Issues](https://github.com/vikingmew/tarbox/issues)
- **讨论**: [GitHub Discussions](https://github.com/vikingmew/tarbox/discussions)

---

<div align="center">

**[⬆ 返回顶部](#-tarbox)**

由 Tarbox 团队用 ❤️ 制作

</div>

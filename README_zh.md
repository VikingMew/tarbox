# Tarbox

一个基于 PostgreSQL 的分布式文件系统，专为 AI Agent 和云原生环境设计。

## 概述

Tarbox 是一个高性能的文件系统实现，将 PostgreSQL 作为存储后端，为 AI Agent 提供可靠、可审计的文件存储解决方案。通过 FUSE 接口提供完整的 POSIX 兼容性，支持 Kubernetes 持久卷（PV）挂载。

## 核心特性

### 1. PostgreSQL 存储后端
- 利用 PostgreSQL 的 ACID 特性保证数据一致性
- 支持分布式部署和高可用配置
- 元数据与数据分离存储，优化查询性能
- 支持大对象（Large Object）存储大文件

### 2. POSIX 兼容性
- 完整的 POSIX 文件系统接口实现
- 支持标准文件操作（read, write, open, close, mkdir, etc.）
- 支持文件权限和属性管理
- 符号链接和硬链接支持

### 3. 文件系统审计
- 完整的操作日志记录
- 文件访问追踪和审计
- 版本历史管理
- 合规性报告支持

### 4. 分层文件系统
- 类似 Docker 镜像层的版本化设计
- 写时复制（Copy-on-Write）技术
- 支持创建检查点（Checkpoint）
- 快速回滚和层间切换
- 通过文件系统 Hook 控制（如 `echo "checkpoint" > /.tarbox/layers/new`）

### 5. FUSE 接口
- 基于 FUSE（Filesystem in Userspace）实现
- 无需内核模块，易于部署
- 跨平台支持（Linux, macOS, FreeBSD）
- 高性能的用户态文件系统

### 6. 文本文件优化
- 针对 CSV、Markdown、YAML、HTML、代码等文本文件的智能支持
- 行级差异存储（类似 Git），层间只存储变化的行
- 跨文件、跨层的内容去重
- 高效的文件版本对比（`tarbox diff`）
- 完整的文件历史追踪
- 对应用层完全透明，保持标准 POSIX 语义

### 7. 原生文件系统挂载
- 将特定目录（如 `/bin`、`/usr`、`.venv`）挂载到宿主机原生文件系统
- 支持只读（ro）和读写（rw）两种模式
- 支持跨租户共享（系统目录）或租户独立（工作目录）
- 绕过数据库，直接访问原生 FS，提供更高性能
- 配置驱动，灵活控制挂载路径和访问权限
- 适用场景：系统工具、虚拟环境、构建缓存、共享模型

### 8. Kubernetes 集成
- 原生 Kubernetes PV/PVC 支持
- CSI（Container Storage Interface）驱动
- 动态卷供应
- 多租户隔离
- 快照和备份支持

## 架构

```
┌─────────────────────────────────────────────┐
│         应用层 / AI Agent                    │
└─────────────────┬───────────────────────────┘
                  │
┌─────────────────┴───────────────────────────┐
│           FUSE Interface                     │
│  (POSIX-compliant File Operations)          │
└─────────────────┬───────────────────────────┘
                  │
┌─────────────────┴───────────────────────────┐
│         Tarbox Core Engine                   │
│  ┌──────────────────────────────────────┐   │
│  │  文件系统层                           │   │
│  │  - Inode 管理                        │   │
│  │  - 目录树                            │   │
│  │  - 权限控制                          │   │
│  └──────────────────────────────────────┘   │
│  ┌──────────────────────────────────────┐   │
│  │  审计层                              │   │
│  │  - 操作日志                          │   │
│  │  - 版本控制                          │   │
│  │  - 访问追踪                          │   │
│  └──────────────────────────────────────┘   │
│  ┌──────────────────────────────────────┐   │
│  │  分层文件系统管理                     │   │
│  │  - Layer 管理（创建/切换/删除）      │   │
│  │  - 写时复制（COW）                   │   │
│  │  - 检查点（Checkpoint）              │   │
│  │  - 层合并和分支                      │   │
│  └──────────────────────────────────────┘   │
└─────────────────┬───────────────────────────┘
                  │
┌─────────────────┴───────────────────────────┐
│        PostgreSQL Storage Backend            │
│  - 元数据表 (Metadata Tables)               │
│  - 数据块存储 (Data Blocks)                 │
│  - 审计日志表 (Audit Logs)                  │
│  - 层管理表 (Layer Tables)                  │
└──────────────────────────────────────────────┘
```

## 快速开始

### 前置要求

- Rust 1.92+ (Edition 2024)
- PostgreSQL 14+
- FUSE 库（Linux: libfuse3, macOS: macFUSE）

### 安装

```bash
# 克隆仓库
git clone https://github.com/yourusername/tarbox.git
cd tarbox

# 构建项目
cargo build --release

# 安装
cargo install --path .
```

### 配置

创建配置文件 `tarbox.toml`:

```toml
[database]
host = "localhost"
port = 5432
database = "tarbox"
user = "tarbox_user"
password = "your_password"
pool_size = 20

[filesystem]
mount_point = "/mnt/tarbox"
block_size = 4096
max_file_size = "10GB"

[audit]
enabled = true
log_level = "info"
retention_days = 90

[layer]
auto_checkpoint = false
checkpoint_interval = "1h"

# 原生文件系统挂载
[[native_mounts]]
path = "/bin"
source = "/bin"
mode = "ro"
shared = true

[[native_mounts]]
path = "/usr"
source = "/usr"
mode = "ro"
shared = true

[[native_mounts]]
path = "/.venv"
source = "/var/tarbox/venvs/{tenant_id}"
mode = "rw"
shared = false
```

### 运行

```bash
# 初始化数据库
tarbox init --config tarbox.toml

# 挂载文件系统
tarbox mount --config tarbox.toml

# 卸载文件系统
tarbox umount /mnt/tarbox
```

## Kubernetes 部署

### 安装 CSI 驱动

```bash
cargo build                                      # Build
cargo test                                       # Test
cargo fmt --all                                  # Format
cargo clippy --all-targets --all-features -- -D warnings  # Lint
```

### Project Structure

```
tarbox/
├── src/              # Source code
├── spec/             # Architecture specifications
├── task/             # Development tasks
└── tests/            # Tests
```

## 性能优化

### 数据库优化

```sql
-- 为元数据表创建索引
CREATE INDEX idx_inode_parent ON inodes(parent_id);
CREATE INDEX idx_inode_name ON inodes(name);
CREATE INDEX idx_blocks_inode ON blocks(inode_id);

-- 启用并行查询
SET max_parallel_workers_per_gather = 4;
```

### 缓存配置

```toml
[cache]
metadata_cache_size = "1GB"
block_cache_size = "4GB"
cache_policy = "lru"
```

## 审计和监控

### 查询审计日志

```sql
-- 查看最近的文件操作
SELECT * FROM audit_logs
WHERE created_at > NOW() - INTERVAL '1 day'
ORDER BY created_at DESC;

-- 查看特定文件的访问历史
SELECT * FROM audit_logs
WHERE inode_id = 12345
ORDER BY created_at DESC;

-- 查看文本文件的修改统计
SELECT
    path,
    SUM((metadata->'text_changes'->>'lines_added')::int) as total_added,
    SUM((metadata->'text_changes'->>'lines_deleted')::int) as total_deleted
FROM audit_logs
WHERE metadata->'text_changes'->>'is_text_file' = 'true'
  AND created_at > NOW() - INTERVAL '7 days'
GROUP BY path
ORDER BY total_added + total_deleted DESC;
```

### 文件历史和差异

MIT OR Apache-2.0

A PostgreSQL-based distributed filesystem designed for AI agents and cloud-native environments.

[中文文档](README_zh.md)

## Overview

Tarbox is a high-performance filesystem implementation using PostgreSQL as the storage backend, providing reliable and auditable file storage for AI agents. It offers complete POSIX compatibility through a FUSE interface and supports Kubernetes Persistent Volume (PV) mounting.

## Core Features

- **PostgreSQL Storage Backend**: ACID properties, distributed deployment, HA support
- **POSIX Compatibility**: Standard file operations and permissions
- **Filesystem Auditing**: Complete operation logging and version history
- **Layered Filesystem**: Docker-like layers with Copy-on-Write
- **FUSE Interface**: User-space implementation, no kernel module required
- **Text File Optimization**: Git-like line-level diff storage
- **Native Filesystem Mounting**: Direct host FS access for performance
- **Kubernetes Integration**: CSI driver with dynamic provisioning

## Quick Start

### Prerequisites

- Rust 1.92+ (Edition 2024)
- PostgreSQL 14+
- FUSE library (Linux: libfuse3, macOS: macFUSE)

### Installation

```bash
git clone https://github.com/yourusername/tarbox.git
cd tarbox
cargo build --release
```

### Basic Usage

```bash
# Initialize database
tarbox init

# Create tenant
tarbox tenant create myagent

# Use filesystem
tarbox --tenant myagent mkdir /data
tarbox --tenant myagent write /data/test.txt "hello world"
tarbox --tenant myagent cat /data/test.txt
tarbox --tenant myagent ls /data
```

## Development

### Commands

```bash
cargo build                                      # Build
cargo test                                       # Test
cargo fmt --all                                  # Format
cargo clippy --all-targets --all-features -- -D warnings  # Lint
```

### Project Structure

```
tarbox/
├── src/              # Source code
├── spec/             # Architecture specifications
├── task/             # Development tasks
└── tests/            # Tests
```

## Roadmap

### MVP Phase (Current)
- [x] Project setup
- [ ] Database layer (MVP)
- [ ] Filesystem core (MVP)
- [ ] CLI tool (MVP)

### Advanced Features
- [ ] FUSE interface
- [ ] Layered filesystem
- [ ] Audit system
- [ ] Kubernetes CSI driver

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines and [CLAUDE.md](CLAUDE.md) for development details.

### Coding Principles

Following Linus Torvalds and John Carmack philosophies:
- Simple and direct code
- Fail fast error handling
- Data-oriented design
- Small, focused functions

## License

MIT OR Apache-2.0

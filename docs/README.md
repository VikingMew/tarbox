# Tarbox 文档

欢迎使用 Tarbox 文档！

## 📚 文档结构

Tarbox 的文档分为三个主要部分：

### 1. 用户文档（本目录 docs/）
面向最终用户的使用指南和参考文档。

**当前状态**: 文档正在完善中，大部分内容已在主 README 中提供。

### 2. 技术规范（spec/）
面向开发者的技术设计文档和架构说明。

- [spec/00-overview.md](../spec/00-overview.md) - 系统架构总览
- [spec/01-database-schema.md](../spec/01-database-schema.md) - 数据库设计（MVP）
- [spec/02-fuse-interface.md](../spec/02-fuse-interface.md) - FUSE 接口实现
- [spec/README.md](../spec/README.md) - 完整规范索引

### 3. 开发任务（task/）
面向项目开发者的任务规划和进度跟踪。

- [task/00-mvp-roadmap.md](../task/00-mvp-roadmap.md) - MVP 开发路线图
- [task/README.md](../task/README.md) - 任务索引

---

## 🚀 快速开始

### 新用户

如果你是第一次使用 Tarbox，建议按以下顺序阅读：

1. **[主 README](../README.md)** - 项目概述和快速开始
2. **[快速开始](#快速开始)** - 5 分钟上手指南（见下文）
3. **[CLI 参考](../README.md#cli-命令当前可用)** - 命令行工具使用

### 开发者

如果你想了解 Tarbox 的内部实现或参与开发：

1. **[架构概览](../spec/00-overview.md)** - 理解系统设计
2. **[开发指南](../CLAUDE.md)** - 开发环境设置和编码规范
3. **[贡献指南](../CONTRIBUTING.md)** - 如何提交代码

---

## 📖 快速开始

### 前置要求

- Rust 1.92+ (Edition 2024)
- PostgreSQL 14+
- FUSE (Linux: libfuse3, macOS: macFUSE)

### 安装

```bash
# 克隆仓库
git clone https://github.com/vikingmew/tarbox.git
cd tarbox

# 构建
cargo build --release

# 设置数据库连接
export DATABASE_URL=postgres://postgres:postgres@localhost:5432/tarbox
```

### 基础使用

```bash
# 1. 初始化数据库
tarbox init

# 2. 创建租户
tarbox tenant create myagent

# 3. 创建文件和目录
tarbox --tenant myagent mkdir /data
tarbox --tenant myagent write /data/config.txt "key=value"
tarbox --tenant myagent cat /data/config.txt

# 4. 通过 FUSE 挂载使用
tarbox --tenant myagent mount /mnt/tarbox
echo "hello" > /mnt/tarbox/data/test.txt
cat /mnt/tarbox/data/test.txt
tarbox umount /mnt/tarbox
```

完整的命令列表请查看 [主 README](../README.md#cli-命令当前可用)。

---

## 📋 文档索引

### 用户指南

- **[快速开始](#快速开始)** - 5 分钟上手（本页）
- **[CLI 参考](../README.md#cli-命令当前可用)** - 完整命令文档
- **[配置](../CLAUDE.md)** - 配置选项说明

### 技术文档

- **[架构设计](../spec/00-overview.md)** - 系统架构
- **[数据库设计](../spec/01-database-schema.md)** - PostgreSQL 表结构
- **[FUSE 接口](../spec/02-fuse-interface.md)** - POSIX 操作实现
- **[多租户设计](../spec/09-multi-tenancy.md)** - 租户隔离机制

### 开发文档

- **[贡献指南](../CONTRIBUTING.md)** - 如何参与开发
- **[开发设置](../CLAUDE.md)** - 环境搭建和编码规范
- **[测试策略](../spec/15-testing-strategy.md)** - 测试方法和覆盖率

---

## 🔄 文档更新日志

- **2026-01-18**: 创建 docs 目录和基础 README
- 文档持续完善中...

---

## 📞 获取帮助

- **问题**: [GitHub Issues](https://github.com/vikingmew/tarbox/issues)
- **讨论**: [GitHub Discussions](https://github.com/vikingmew/tarbox/discussions)
- **贡献**: [CONTRIBUTING.md](../CONTRIBUTING.md)

---

**注意**: Tarbox 目前处于活跃开发阶段。
- ✅ **已完成**: 核心功能（PostgreSQL 存储、CLI 工具、FUSE 挂载）和高级存储数据库层（审计、分层、文本优化）
- 🚧 **开发中**: 文件系统与高级存储的集成（COW、diff、hooks）

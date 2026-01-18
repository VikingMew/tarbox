# Task 04: CLI 工具实现（MVP）

## 目标

实现命令行工具，支持租户管理和基础文件操作，用于验证最小 MVP。

## 优先级

**P0 - 最高优先级**

## 状态

**✅ 已完成** - 2026-01-17

## 依赖

- Task 01: 项目初始化和基础设施搭建 ✅
- Task 02: 数据库存储层（MVP） ✅
- Task 03: 基础文件系统实现（MVP） ✅

## 子任务

### 4.1 CLI 框架搭建

- [x] 使用 clap 实现 CLI 框架
  - 主命令：`tarbox`
  - 子命令结构
  - 全局参数（--config, --database-url）

- [ ] 配置文件加载
  - 从文件加载配置
  - 从环境变量覆盖
  - 从命令行参数覆盖

### 4.2 租户管理命令

- [x] tenant create
  ```bash
  tarbox tenant create <tenant-name>
  # 创建租户，返回 tenant_id
  ```
  - 调用 create_tenant()
  - 打印 tenant_id 和根目录信息

- [x] tenant list
  ```bash
  tarbox tenant list
  # 列出所有租户
  ```
  - 查询所有租户
  - 简单格式显示：名称 (ID)

- [x] tenant info
  ```bash
  tarbox tenant info <tenant-name>
  # 显示租户详细信息
  ```
  - 查询租户信息
  - 显示 ID、root inode、创建时间

- [x] tenant delete
  ```bash
  tarbox tenant delete <tenant-name>
  # 删除租户
  ```
  - 调用 delete_tenant()
  - 级联删除所有数据

### 4.3 文件系统操作命令

所有命令需要指定租户：`--tenant <name>`

- [x] mkdir - 创建目录
  ```bash
  tarbox --tenant myagent mkdir /data
  tarbox --tenant myagent mkdir /data/files
  ```
  - 调用 create_directory()
  - 打印成功消息

- [x] ls - 列出目录
  ```bash
  tarbox --tenant myagent ls /
  tarbox --tenant myagent ls /data
  ```
  - 调用 list_directory()
  - 显示文件名和类型（目录带 / 后缀）

- [x] touch - 创建文件
  ```bash
  tarbox --tenant myagent touch /data/test.txt
  ```
  - 调用 create_file()
  - 创建空文件

- [x] write - 写入文件
  ```bash
  tarbox --tenant myagent write /data/test.txt "hello world"
  ```
  - 调用 write_file()
  - 支持直接写入字符串

- [x] cat - 读取文件
  ```bash
  tarbox --tenant myagent cat /data/test.txt
  ```
  - 调用 read_file()
  - 输出文件内容到 stdout

- [x] rm - 删除文件
  ```bash
  tarbox --tenant myagent rm /data/test.txt
  ```
  - 调用 delete_file()
  - 打印成功消息

- [x] rmdir - 删除目录
  ```bash
  tarbox --tenant myagent rmdir /data/empty
  ```
  - 调用 remove_directory()
  - 只能删除空目录

- [x] stat - 显示文件信息
  ```bash
  tarbox --tenant myagent stat /data/test.txt
  ```
  - 调用 stat()
  - 显示详细元数据（size, type, mode, uid/gid, timestamps）

### 4.4 辅助命令

- [x] init - 初始化数据库
  ```bash
  tarbox init
  ```
  - 运行数据库迁移
  - 显示初始化完成消息

- [x] mount - 挂载文件系统（新增FUSE功能）
  ```bash
  tarbox --tenant myagent mount /mnt/tarbox
  ```
  - 通过FUSE挂载文件系统
  - 支持 --allow-other, --allow-root, --read-only 选项
  
- [x] umount - 卸载文件系统
  ```bash
  tarbox umount /mnt/tarbox
  ```
  - 卸载FUSE挂载点

### 4.5 输出格式化

- [x] 实现基础输出
  - 简单的文本输出
  - 清晰的成功/错误消息

### 4.6 测试

- [x] 所有命令可执行
  - 创建租户 ✅
  - 创建目录结构 ✅
  - 创建和操作文件 ✅
  - 列出目录 ✅
  - 删除文件和目录 ✅
  - FUSE 挂载/卸载 ✅

## 验收标准

- [x] 可以通过 CLI 创建租户
- [x] 可以通过 CLI 执行所有基础文件操作
- [x] 可以通过 CLI 挂载和卸载 FUSE 文件系统
- [x] 命令输出清晰易读
- [x] 错误提示友好（使用 anyhow）
- [x] 帮助信息完整（clap 自动生成）
- [x] 项目编译成功
- [x] 代码通过 fmt 和 clippy 检查

## 预估时间

2-3 天

## 实际完成时间

**2 天** (2026-01-16 至 2026-01-17)

## 示例使用场景

```bash
# 初始化数据库
tarbox init

# 创建租户
tarbox tenant create ai-agent-001
# 输出：Created tenant: ai-agent-001 (UUID: xxx-xxx-xxx)

# 创建目录结构
tarbox --tenant ai-agent-001 mkdir /data
tarbox --tenant ai-agent-001 mkdir /data/logs
tarbox --tenant ai-agent-001 mkdir /data/models

# 创建文件并写入
tarbox --tenant ai-agent-001 touch /data/config.txt
tarbox --tenant ai-agent-001 write /data/config.txt "key=value"

# 读取文件
tarbox --tenant ai-agent-001 cat /data/config.txt
# 输出：key=value

# 列出目录
tarbox --tenant ai-agent-001 ls /data
# 输出：
# config.txt  file  9B   2026-01-15 10:30:00
# logs/       dir   -    2026-01-15 10:29:00
# models/     dir   -    2026-01-15 10:29:00

# 删除文件
tarbox --tenant ai-agent-001 rm /data/config.txt

# 查看租户信息
tarbox tenant info ai-agent-001
# 输出：
# Tenant: ai-agent-001
# ID: xxx-xxx-xxx
# Files: 0
# Size: 0 bytes
# Created: 2026-01-15 10:28:00
```

## 技术要点

- 使用 clap 的 derive API
- 每个命令是独立的子命令
- 全局参数 --tenant 在所有文件操作中可用
- 使用 anyhow 处理错误
- 支持 --help 自动生成帮助
- 环境变量 DATABASE_URL 配置数据库连接
- FUSE 挂载功能集成（Task 05）

## 已实现的完整功能

### 租户管理
- `tarbox tenant create <name>` - 创建租户
- `tarbox tenant list` - 列出所有租户
- `tarbox tenant info <name>` - 显示租户详情
- `tarbox tenant delete <name>` - 删除租户

### 文件系统操作
- `tarbox --tenant <name> mkdir <path>` - 创建目录
- `tarbox --tenant <name> ls [path]` - 列出目录
- `tarbox --tenant <name> rmdir <path>` - 删除空目录
- `tarbox --tenant <name> touch <path>` - 创建文件
- `tarbox --tenant <name> write <path> <content>` - 写入文件
- `tarbox --tenant <name> cat <path>` - 读取文件
- `tarbox --tenant <name> rm <path>` - 删除文件
- `tarbox --tenant <name> stat <path>` - 显示文件信息

### FUSE 挂载
- `tarbox --tenant <name> mount <mountpoint>` - 挂载文件系统
- `tarbox umount <mountpoint>` - 卸载文件系统

### 系统管理
- `tarbox init` - 初始化数据库

## 未实现功能（未来扩展）

- 表格格式输出（ls -l）
- JSON 输出格式 (--json)
- 从文件读取写入 (write --file)
- version 命令
- 交互式确认（删除租户）

## 后续任务

完成后可以开始：
- Task 06: 数据库层高级功能（审计、分层、文本优化）
- Task 07: 文件系统核心高级功能（权限、链接、缓存）
- Task 08: 分层文件系统实现
- Task 09: CLI 工具高级功能（快照、审计查询等）

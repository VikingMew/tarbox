# Task 04: CLI 工具实现（MVP）

## 目标

实现命令行工具，支持租户管理和基础文件操作，用于验证最小 MVP。

## 优先级

**P0 - 最高优先级**

## 依赖

- Task 01: 项目初始化和基础设施搭建
- Task 02: 数据库存储层（MVP）
- Task 03: 基础文件系统实现（MVP）

## 子任务

### 4.1 CLI 框架搭建

- [ ] 使用 clap 实现 CLI 框架
  - 主命令：`tarbox`
  - 子命令结构
  - 全局参数（--config, --database-url）

- [ ] 配置文件加载
  - 从文件加载配置
  - 从环境变量覆盖
  - 从命令行参数覆盖

### 4.2 租户管理命令

- [ ] tenant create
  ```bash
  tarbox tenant create <tenant-name>
  # 创建租户，返回 tenant_id
  ```
  - 调用 create_tenant()
  - 打印 tenant_id 和根目录信息

- [ ] tenant list
  ```bash
  tarbox tenant list
  # 列出所有租户
  ```
  - 查询所有租户
  - 表格形式显示：ID、名称、创建时间

- [ ] tenant info
  ```bash
  tarbox tenant info <tenant-name-or-id>
  # 显示租户详细信息
  ```
  - 查询租户信息
  - 显示统计（文件数、总大小等）

- [ ] tenant delete
  ```bash
  tarbox tenant delete <tenant-name-or-id>
  # 删除租户
  ```
  - 确认提示
  - 调用 delete_tenant()

### 4.3 文件系统操作命令

所有命令需要指定租户：`--tenant <name-or-id>`

- [ ] mkdir - 创建目录
  ```bash
  tarbox --tenant myagent mkdir /data
  tarbox --tenant myagent mkdir /data/files
  ```
  - 调用 create_directory()
  - 打印成功消息

- [ ] ls - 列出目录
  ```bash
  tarbox --tenant myagent ls /
  tarbox --tenant myagent ls /data
  ```
  - 调用 list_directory()
  - 显示文件名、类型、大小、时间
  - 支持 -l 参数显示详细信息

- [ ] touch - 创建文件
  ```bash
  tarbox --tenant myagent touch /data/test.txt
  ```
  - 调用 create_file()
  - 创建空文件

- [ ] write - 写入文件
  ```bash
  tarbox --tenant myagent write /data/test.txt "hello world"
  tarbox --tenant myagent write /data/test.txt --file input.txt
  ```
  - 调用 write_file()
  - 支持直接写入字符串
  - 支持从文件读取内容写入

- [ ] cat - 读取文件
  ```bash
  tarbox --tenant myagent cat /data/test.txt
  ```
  - 调用 read_file()
  - 输出文件内容到 stdout

- [ ] rm - 删除文件
  ```bash
  tarbox --tenant myagent rm /data/test.txt
  ```
  - 调用 delete_file()
  - 打印成功消息

- [ ] rmdir - 删除目录
  ```bash
  tarbox --tenant myagent rmdir /data/empty
  ```
  - 调用 remove_directory()
  - 只能删除空目录

- [ ] stat - 显示文件信息
  ```bash
  tarbox --tenant myagent stat /data/test.txt
  ```
  - 调用 stat()
  - 显示详细元数据

### 4.4 辅助命令

- [ ] init - 初始化数据库
  ```bash
  tarbox init
  ```
  - 创建所有表
  - 创建索引
  - 显示初始化完成消息

- [ ] version - 显示版本
  ```bash
  tarbox version
  ```
  - 显示版本号和构建信息

### 4.5 输出格式化

- [ ] 实现表格输出
  - 对齐列
  - 美化显示

- [ ] 实现 JSON 输出
  - 添加 --json 全局参数
  - 所有命令支持 JSON 输出

- [ ] 错误提示
  - 友好的错误消息
  - 显示可能的修复建议

### 4.6 测试

- [ ] 手动测试所有命令
  - 创建租户
  - 创建目录结构
  - 创建和操作文件
  - 列出目录
  - 删除文件和目录

- [ ] 集成测试
  - 完整的使用场景
  - 错误场景

## 验收标准

- [ ] 可以通过 CLI 创建租户
- [ ] 可以通过 CLI 执行所有基础文件操作
- [ ] 命令输出清晰易读
- [ ] 错误提示友好
- [ ] 帮助信息完整

## 预估时间

2-3 天

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
- 全局参数在所有子命令中可用
- 使用 anyhow 处理错误
- 支持 --help 自动生成帮助

## 后续任务

完成后可以开始：
- Task 05: FUSE 接口实现（完整功能）
- Task 08: CLI 工具完善（快照、审计查询等）

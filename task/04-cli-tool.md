# Task 04: CLI Tool (MVP)

## 目标

实现命令行工具，提供租户管理和基础文件系统操作功能，使开发者可以通过简单的命令管理文件系统。

## 依赖

- Task 02: 数据库存储层 ✅
- Task 03: 文件系统核心 ✅

## 任务清单

### 基础架构
- [x] 添加 clap 依赖用于命令行解析
- [x] 设计命令结构（租户管理 + 文件操作）
- [x] 实现主程序框架和命令分发

### 数据库初始化
- [x] `tarbox init` - 初始化数据库 schema
  - 运行所有 migrations
  - 创建所有必需的表

### 租户管理命令
- [x] `tarbox tenant create <name>` - 创建新租户
  - 自动创建根目录
  - 返回 tenant_id
- [x] `tarbox tenant info <name>` - 显示租户信息
  - 显示 ID、名称、根 inode、创建时间
- [x] `tarbox tenant list` - 列出所有租户
  - 按创建时间倒序排列
- [x] `tarbox tenant delete <name>` - 删除租户
  - CASCADE 删除所有关联数据

### 文件系统操作命令
所有命令需要 `--tenant <name>` 参数

- [x] `tarbox --tenant <name> mkdir <path>` - 创建目录
- [x] `tarbox --tenant <name> ls [path]` - 列出目录内容
  - 默认路径为 "/"
  - 目录显示 "/" 后缀
- [x] `tarbox --tenant <name> rmdir <path>` - 删除空目录
- [x] `tarbox --tenant <name> touch <path>` - 创建空文件
- [x] `tarbox --tenant <name> write <path> <content>` - 写入文件
  - 显示写入字节数
- [x] `tarbox --tenant <name> cat <path>` - 读取文件内容
- [x] `tarbox --tenant <name> rm <path>` - 删除文件
- [x] `tarbox --tenant <name> stat <path>` - 显示文件/目录信息
  - 大小、类型、权限、所有者、时间戳

### 配置管理
- [x] 从环境变量读取 DATABASE_URL
  - 默认值: `postgres://postgres:postgres@localhost:5432/tarbox`
- [ ] 支持配置文件（可选）
  - 位置: `~/.config/tarbox/config.toml`
  - 参数: database_url, max_connections

### 错误处理
- [x] 清晰的错误信息
- [x] 适当的退出码
  - 成功: 0
  - 错误: 1

### 输出格式
- [x] 用户友好的输出格式
- [x] 目录列表显示 "/" 后缀
- [x] stat 命令使用类似 `stat` 工具的格式

### 测试
- [ ] CLI 集成测试
  - 测试所有命令的基本功能
  - 测试错误场景
- [ ] 端到端测试（参考 task/00-mvp-roadmap.md 验收标准）

## 实现细节

### 命令结构
```rust
tarbox
├── init                           # 初始化数据库
├── tenant
│   ├── create <name>             # 创建租户
│   ├── info <name>               # 租户信息
│   ├── list                      # 列出租户
│   └── delete <name>             # 删除租户
├── --tenant <name> mkdir <path>   # 创建目录
├── --tenant <name> ls [path]      # 列出目录
├── --tenant <name> rmdir <path>   # 删除目录
├── --tenant <name> touch <path>   # 创建文件
├── --tenant <name> write <path> <content>  # 写入文件
├── --tenant <name> cat <path>     # 读取文件
├── --tenant <name> rm <path>      # 删除文件
└── --tenant <name> stat <path>    # 文件信息
```

### 代码组织
- `src/main.rs` - CLI 入口和命令实现
- 使用 clap derive API 定义命令结构
- 每个命令独立函数处理
- 共享的租户查找逻辑

### 数据库连接
- 每个命令创建独立的连接池
- 使用环境变量配置
- 适当的连接池大小（10 max, 2 min）

## 验收标准

### 功能验收
根据 task/00-mvp-roadmap.md 的验收标准：

```bash
# 1. 初始化数据库
tarbox init
# ✓ 所有表创建成功

# 2. 创建租户
tarbox tenant create test-agent
# ✓ 返回 tenant_id
# ✓ 自动创建根目录

# 3. 创建目录结构
tarbox --tenant test-agent mkdir /data
tarbox --tenant test-agent mkdir /data/logs
tarbox --tenant test-agent mkdir /data/models
# ✓ 目录创建成功

# 4. 列出目录
tarbox --tenant test-agent ls /
# ✓ 显示：data/
tarbox --tenant test-agent ls /data
# ✓ 显示：logs/, models/

# 5. 创建和写入文件
tarbox --tenant test-agent touch /data/config.txt
tarbox --tenant test-agent write /data/config.txt "key=value"
# ✓ 文件创建和写入成功

# 6. 读取文件
tarbox --tenant test-agent cat /data/config.txt
# ✓ 输出：key=value

# 7. 查看文件信息
tarbox --tenant test-agent stat /data/config.txt
# ✓ 显示大小、权限、时间戳

# 8. 删除文件
tarbox --tenant test-agent rm /data/config.txt
# ✓ 文件删除成功

# 9. 删除目录
tarbox --tenant test-agent rmdir /data/logs
# ✓ 空目录删除成功

# 10. 租户信息
tarbox tenant info test-agent
# ✓ 显示租户统计信息
```

### 质量验收
- [x] 代码编译无错误
- [x] 代码编译无警告（需清理未使用的导入）
- [ ] 集成测试通过
- [ ] 符合 Rust 2024 最佳实践

## 测试计划

### 单元测试
由于 CLI 主要是命令分发和格式化输出，单元测试较少。

### 集成测试
创建 `tests/cli_integration_test.rs`：
- 测试所有命令的正常流程
- 测试错误场景（租户不存在、路径不存在等）
- 测试输出格式

### 端到端测试
使用真实 PostgreSQL 数据库测试完整工作流。

## 当前状态

- [x] 所有命令实现完成
- [x] 编译成功
- [ ] 需要编写集成测试
- [ ] 需要运行完整验收测试

## 下一步

1. 编写 CLI 集成测试
2. 运行完整的验收测试
3. 修复发现的问题
4. 更新文档

## 注意事项

1. **简单性**: MVP 版本保持简单，不过度设计
2. **用户体验**: 提供清晰的错误信息和帮助文本
3. **配置**: 优先使用环境变量，配置文件为可选
4. **性能**: CLI 工具性能要求不高，优先考虑代码清晰度

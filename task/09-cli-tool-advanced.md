# Task 09: CLI 工具高级功能

## 目标

在 Task 04 (CLI 工具 MVP) 的基础上，实现完整的 CLI 工具，包括：
- **高级文件操作**: 符号链接、硬链接、扩展属性、权限管理
- **层管理命令**: 创建、切换、对比、导出层
- **审计查询**: 操作历史、统计分析
- **文本文件优化**: diff、合并、历史查看
- **性能分析**: 存储使用、性能指标

## 优先级

**P2 - 高级功能**

## 依赖

- Task 04: CLI 工具 MVP ✅
- Task 05: FUSE 接口 ✅
- Task 06: 数据库层高级功能
- Task 07: 文件系统核心高级功能
- Task 08: 分层文件系统

## 依赖的Spec

- **spec/04-layered-filesystem.md** - 层管理命令设计
- **spec/08-filesystem-hooks.md** - 虚拟目录命令接口（可作为 CLI 的补充或替代）
- **spec/03-audit-system.md** - 审计日志查询和统计
- **spec/10-text-file-optimization.md** - 文本文件 diff 和历史查看
- spec/02-fuse-interface.md - 高级 POSIX 操作（symlink, xattr 等）
- spec/07-performance.md - 性能指标和分析

## 子任务

### 9.1 高级文件操作

- [ ] **符号链接和硬链接**
  ```bash
  tarbox --tenant <name> ln -s <target> <link>    # 创建符号链接
  tarbox --tenant <name> ln <target> <link>       # 创建硬链接
  tarbox --tenant <name> readlink <path>          # 读取链接目标
  ```

- [ ] **扩展属性 (xattr)**
  ```bash
  tarbox --tenant <name> setxattr <path> <key> <value>
  tarbox --tenant <name> getxattr <path> <key>
  tarbox --tenant <name> listxattr <path>
  tarbox --tenant <name> removexattr <path> <key>
  ```

- [ ] **权限管理**
  ```bash
  tarbox --tenant <name> chmod <mode> <path>      # 修改权限
  tarbox --tenant <name> chown <uid>:<gid> <path> # 修改所有者
  tarbox --tenant <name> chgrp <gid> <path>       # 修改组
  ```

- [ ] **批量操作**
  ```bash
  tarbox --tenant <name> cp <src> <dst>           # 复制文件/目录
  tarbox --tenant <name> mv <src> <dst>           # 移动文件/目录
  tarbox --tenant <name> cp -r <src> <dst>        # 递归复制目录
  ```

### 9.2 层管理命令

- [ ] **层操作**
  ```bash
  tarbox --tenant <name> layer list               # 列出所有层
  tarbox --tenant <name> layer create [--name <name>] [--desc <desc>]
                                                  # 创建新层
  tarbox --tenant <name> layer info <layer-id>    # 显示层信息
  tarbox --tenant <name> layer switch <layer-id>  # 切换到指定层
  tarbox --tenant <name> layer delete <layer-id>  # 删除层
  ```

- [ ] **层历史**
  ```bash
  tarbox --tenant <name> layer history            # 显示层链历史
  tarbox --tenant <name> layer diff <layer1> <layer2>
                                                  # 对比两个层的差异
  tarbox --tenant <name> layer export <layer-id> <file>
                                                  # 导出层
  tarbox --tenant <name> layer import <file>      # 导入层
  ```

- [ ] **检查点和快照**
  ```bash
  tarbox --tenant <name> checkpoint create <name> # 创建检查点
  tarbox --tenant <name> checkpoint list          # 列出检查点
  tarbox --tenant <name> checkpoint restore <name># 恢复到检查点
  tarbox --tenant <name> checkpoint delete <name> # 删除检查点
  ```

### 9.3 审计和监控

- [ ] **审计日志查询**
  ```bash
  tarbox --tenant <name> audit logs [--since <time>] [--until <time>]
                                                  # 查询审计日志
  tarbox --tenant <name> audit logs --operation <op>
                                                  # 按操作类型筛选
  tarbox --tenant <name> audit logs --path <path>
                                                  # 按路径筛选
  tarbox --tenant <name> audit logs --user <uid>  # 按用户筛选
  tarbox --tenant <name> audit export <file>      # 导出审计日志
  ```

- [ ] **统计和分析**
  ```bash
  tarbox --tenant <name> stats summary            # 显示统计摘要
  tarbox --tenant <name> stats files              # 文件统计
  tarbox --tenant <name> stats usage              # 空间使用统计
  tarbox --tenant <name> stats top-files [--by-size|--by-access]
                                                  # 热门文件
  tarbox --tenant <name> stats dedup              # 去重统计
  ```

### 9.4 租户高级管理

- [ ] **配额管理**
  ```bash
  tarbox tenant quota <name> --bytes <size>       # 设置字节配额
  tarbox tenant quota <name> --inodes <count>     # 设置 inode 配额
  tarbox tenant quota <name> --layers <count>     # 设置层数配额
  tarbox tenant quota <name> --show               # 显示配额使用情况
  ```

- [ ] **租户状态**
  ```bash
  tarbox tenant suspend <name>                    # 暂停租户
  tarbox tenant resume <name>                     # 恢复租户
  tarbox tenant archive <name>                    # 归档租户
  ```

- [ ] **批量操作**
  ```bash
  tarbox tenant import <file>                     # 从文件导入租户
  tarbox tenant export <name> <file>              # 导出租户数据
  tarbox tenant clone <src> <dst>                 # 克隆租户
  ```

### 9.5 文本文件优化命令

- [ ] **文本文件操作**
  ```bash
  tarbox --tenant <name> text diff <path1> <path2>
                                                  # 对比文本文件
  tarbox --tenant <name> text history <path>      # 查看文件修改历史
  tarbox --tenant <name> text blame <path>        # 显示每行的修改信息
  tarbox --tenant <name> text search <pattern> [path]
                                                  # 搜索文本内容
  ```

- [ ] **去重信息**
  ```bash
  tarbox --tenant <name> text blocks <path>       # 显示文件的文本块
  tarbox --tenant <name> text dedup-stats         # 文本去重统计
  ```

### 9.6 性能和调试

- [ ] **缓存管理**
  ```bash
  tarbox cache stats                              # 缓存统计
  tarbox cache clear [--all|--metadata|--blocks]  # 清除缓存
  tarbox cache warm <tenant>                      # 预热缓存
  ```

- [ ] **性能分析**
  ```bash
  tarbox benchmark read <path>                    # 读性能测试
  tarbox benchmark write <path>                   # 写性能测试
  tarbox benchmark metadata <path>                # 元数据操作测试
  tarbox profile <command>                        # 性能分析
  ```

- [ ] **调试工具**
  ```bash
  tarbox debug inspect-inode <inode-id>           # 检查 inode 详情
  tarbox debug inspect-block <block-id>           # 检查数据块详情
  tarbox debug verify-integrity <tenant>          # 验证数据完整性
  tarbox debug check-orphans <tenant>             # 检查孤立对象
  ```

### 9.7 输出格式和交互

- [ ] **多种输出格式**
  ```bash
  --output json                                   # JSON 格式输出
  --output yaml                                   # YAML 格式输出
  --output table                                  # 表格格式（默认）
  --output csv                                    # CSV 格式
  ```

- [ ] **交互模式**
  ```bash
  tarbox shell --tenant <name>                    # 进入交互式 shell
  # 在 shell 中可以直接使用文件操作命令，无需重复 --tenant
  ```

- [ ] **批处理**
  ```bash
  tarbox batch <script-file>                      # 批量执行命令
  tarbox batch --interactive                      # 交互式批处理
  ```

### 9.8 配置和插件

- [ ] **配置管理**
  ```bash
  tarbox config init                              # 初始化配置文件
  tarbox config show                              # 显示当前配置
  tarbox config set <key> <value>                 # 设置配置项
  tarbox config get <key>                         # 获取配置项
  ```

**不在本任务范围内（未来扩展功能）：**
- 插件系统 - 需要插件架构设计
- 脚本执行支持 - 需要安全沙箱机制
- Web UI 集成 - 属于独立的 Web 界面项目

## 实现细节

### 命令组织

```
tarbox
├── init                          # 初始化（MVP）
├── tenant                        # 租户管理
│   ├── create/info/list/delete   # MVP 功能
│   ├── quota                     # 配额管理（高级）
│   ├── suspend/resume/archive    # 状态管理（高级）
│   └── import/export/clone       # 批量操作（高级）
├── --tenant <name>               # 文件系统操作
│   ├── mkdir/ls/rmdir/...        # MVP 功能
│   ├── ln/chmod/chown/...        # 高级文件操作
│   ├── layer                     # 层管理
│   ├── audit                     # 审计查询
│   ├── stats                     # 统计分析
│   ├── text                      # 文本文件操作
│   └── cp/mv                     # 批量操作
├── cache                         # 缓存管理
├── benchmark                     # 性能测试
├── debug                         # 调试工具
├── config                        # 配置管理
├── shell                         # 交互式 shell
└── batch                         # 批处理
```

### 交互式 Shell

实现类似 `psql` 或 `redis-cli` 的交互式 shell：

```bash
$ tarbox shell --tenant test-agent
tarbox [test-agent] > ls /
data/

tarbox [test-agent] > mkdir /workspace
Created directory: /workspace

tarbox [test-agent] > layer list
Layer ID                              Name        Parent    Files   Size
--------------------------------------------------------------------
550e8400-e29b-41d4-a716-446655440000  main        -         3       1.2 MB
abc12345-e29b-41d4-a716-446655440000  feature-1   main      5       2.5 MB

tarbox [test-agent] > quit
```

特性：
- 命令历史（readline）
- Tab 补全
- 多行编辑
- 彩色输出
- 快捷键支持

### JSON/YAML 输出

所有命令支持机器可读格式：

```bash
$ tarbox --tenant test --output json ls /
{
  "entries": [
    {
      "name": "data",
      "type": "directory",
      "size": 4096,
      "permissions": "0755",
      "owner": {"uid": 1000, "gid": 1000},
      "modified": "2026-01-17T10:30:00Z"
    }
  ]
}
```

### 批处理脚本

```bash
# script.tarbox
tenant create batch-test
--tenant batch-test mkdir /data
--tenant batch-test touch /data/file.txt
--tenant batch-test write /data/file.txt "Hello"
--tenant batch-test layer create --name checkpoint-1
```

执行：
```bash
tarbox batch script.tarbox
```

## 依赖的 Crate

```toml
[dependencies]
# CLI 框架
clap = { version = "4", features = ["derive", "env"] }
clap_complete = "4"  # Shell 补全

# 交互式 shell
rustyline = "14"     # Readline 实现
crossterm = "0.27"   # 终端控制

# 输出格式
serde_json = "1"
serde_yaml = "0.9"
tabled = "0.15"      # 表格输出
csv = "1"

# 其他
indicatif = "0.17"   # 进度条
colored = "2"        # 彩色输出
humantime = "2"      # 时间格式化
bytesize = "1"       # 字节大小格式化
```

## 测试计划

### 单元测试
- 命令参数解析
- 输出格式化
- 错误处理

### 集成测试
- 所有高级命令的基本功能
- 交互式 shell
- 批处理执行
- JSON/YAML 输出

### 端到端测试
- 完整工作流测试
- 性能测试
- 错误恢复测试

## 验收标准

### 功能验收

```bash
# 1. 高级文件操作
tarbox --tenant test ln -s /data/file.txt /link.txt
tarbox --tenant test readlink /link.txt
tarbox --tenant test chmod 0644 /data/file.txt

# 2. 层管理
tarbox --tenant test layer create --name feature-branch
tarbox --tenant test layer list
tarbox --tenant test layer diff main feature-branch

# 3. 审计查询
tarbox --tenant test audit logs --since "1 hour ago"
tarbox --tenant test audit logs --operation write

# 4. 统计分析
tarbox --tenant test stats summary
tarbox --tenant test stats top-files --by-size

# 5. 交互式 shell
tarbox shell --tenant test
> ls /
> layer list
> quit

# 6. JSON 输出
tarbox --tenant test --output json ls /

# 7. 批处理
tarbox batch workflow.tarbox
```

### 质量验收
- [ ] 所有命令编译无错误
- [ ] 所有命令编译无警告
- [ ] 集成测试通过
- [ ] 用户体验良好（清晰的错误信息、帮助文本）
- [ ] 性能可接受（命令响应 < 100ms）

## 当前状态

- [ ] 待开始

## 优先级说明

此任务为 P2（高级功能），在以下任务完成后实施：
- Task 05: FUSE 接口
- Task 06: 数据库层高级功能
- Task 07: 文件系统核心高级功能
- Task 08: 分层文件系统

## 参考

- Task 04: CLI 工具 MVP - 基础命令实现
- spec/04: 分层文件系统设计
- spec/03: 审计系统设计
- spec/10: 文本文件优化设计

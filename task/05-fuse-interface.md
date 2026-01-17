# Task 05: FUSE 接口实现

## 目标

实现 FUSE (Filesystem in Userspace) 接口，将文件系统核心功能暴露为标准的 POSIX 文件系统，支持用户态挂载和访问。

**⭐ 重要架构变更**: 本任务基于 spec/14 的 FilesystemInterface 抽象层设计，实现的代码将被 CSI/WASI 复用（~90%代码共享）。

## 优先级

**P0 - 最高优先级**

## 依赖

- Task 01: 项目初始化和基础设施搭建
- Task 02: 数据库存储层实现
- Task 03: 基础文件系统实现

## 依赖的Spec

- **spec/14-filesystem-interface.md** - 统一的 FilesystemInterface 抽象层（核心）
- **spec/02-fuse-interface.md** - FUSE 具体实现细节
- **spec/09-multi-tenancy.md** - 多租户隔离机制

## 架构说明

根据 spec/14 的设计，本任务分为三层实现：

```
┌─────────────────────────────────────────┐
│    FUSE Callbacks (fuser crate)         │  ← 5.3 FuseAdapter
│    (同步接口，POSIX系统调用映射)          │
└──────────────┬──────────────────────────┘
               │ 桥接
┌──────────────▼──────────────────────────┐
│  FilesystemInterface trait              │  ← 5.1 接口抽象层
│  (统一的异步接口，跨平台抽象)             │
└──────────────┬──────────────────────────┘
               │ 实现
┌──────────────▼──────────────────────────┐
│    TarboxBackend                        │  ← 5.2 核心实现
│    (调用 FileSystem + Storage)          │
└─────────────────────────────────────────┘
```

**关键优势**:
- FilesystemInterface 是纯 Rust trait，90% 代码可被 CSI/WASI 复用
- TarboxBackend 与 FUSE 解耦，易于测试
- FuseAdapter 只负责协议转换，逻辑简单

## 子任务

### 5.1 实现 FilesystemInterface 抽象层 ⭐

**基于**: spec/14-filesystem-interface.md

- [x] 定义 FilesystemInterface trait
  - [x] 核心文件操作 (read_file, write_file, create_file, delete_file)
  - [x] 目录操作 (create_dir, read_dir, remove_dir)
  - [x] 元数据操作 (get_attr, set_attr, chmod, chown)
  - [x] 文件系统信息 (statfs)
  - [ ] 符号链接操作 (create_symlink, read_symlink) - 可选
  - [ ] 扩展属性 (setxattr, getxattr, listxattr, removexattr) - 可选

- [x] 定义统一的数据类型
  - [x] FileAttr - 文件属性
  - [x] DirEntry - 目录条目
  - [x] SetAttr - 属性修改参数
  - [x] StatFs - 文件系统统计
  - [x] FileType - 文件类型枚举
  - [x] FsError - 错误类型

- [x] 实现错误映射
  - [x] FsError → errno 映射
  - [x] 标准错误类型 (PathNotFound, AlreadyExists, PermissionDenied 等)

### 5.2 实现 TarboxBackend

**基于**: spec/14 的 Backend 设计

- [x] 创建 TarboxBackend 结构
  - [x] 持有 PgPool 和 TenantId
  - [x] 提供 new() 构造函数

- [x] 实现 FilesystemInterface trait
  - [x] 所有方法委托给 FileSystem 层
  - [x] 类型转换 (Inode → FileAttr)
  - [x] 错误转换 (anyhow::Error → FsError)
  - [x] 路径解析和验证

- [ ] 实现类型转换辅助函数
  - [x] inode_to_file_attr()
  - [x] inode_type_to_file_type()
  - [ ] file_mode_to_posix()
  - [ ] posix_mode_to_file()

### 5.3 实现 FuseAdapter

**基于**: spec/02-fuse-interface.md

- [ ] 创建 FuseAdapter 结构
  - 持有 Arc<dyn FilesystemInterface>
  - 持有 Runtime handle (用于 block_on)
  - 提供 new() 构造函数

- [ ] 实现 fuser::Filesystem trait
  - 所有回调委托给 FilesystemInterface
  - 使用 block_on 桥接异步到同步
  - FUSE 类型 → FilesystemInterface 类型转换

- [ ] 实现挂载管理
  - `mount()` - 挂载文件系统
  - `unmount()` - 卸载文件系统
  - 参数解析（挂载点、选项等）
  - 后台/前台运行模式

- [ ] 实现租户上下文
  - 挂载时绑定租户
  - 所有操作自动注入 tenant_id
  - 租户验证和权限检查

### 4.2 元数据操作实现

- [ ] lookup - 路径查找
  - 将文件名解析为 inode
  - 返回文件属性
  - 处理符号链接

- [ ] getattr - 获取文件属性
  - 返回 POSIX stat 结构
  - 填充所有必需字段
  - 处理特殊文件（设备文件等）

- [ ] setattr - 设置文件属性
  - 修改大小
  - 修改权限
  - 修改所有者
  - 修改时间戳

- [ ] readlink - 读取符号链接
  - 返回符号链接目标
  - 处理相对和绝对路径

### 4.3 文件操作实现

- [ ] open - 打开文件
  - 权限检查
  - 分配文件句柄
  - 处理打开标志
  - 记录打开状态

- [ ] read - 读取文件
  - 从指定偏移量读取
  - 处理读取大小
  - 智能路由（文本/二进制）
  - 更新访问时间

- [ ] write - 写入文件
  - 写入到指定偏移量
  - 处理写入大小
  - 智能路由（文本/二进制）
  - 更新修改时间
  - 支持追加模式

- [ ] release - 关闭文件
  - 释放文件句柄
  - 刷新缓冲区
  - 清理资源

- [ ] flush - 刷新文件
  - 确保数据写入持久化
  - 处理错误

- [ ] fsync - 同步文件
  - 同步文件数据
  - 同步元数据（如指定）

### 4.4 目录操作实现

- [ ] mkdir - 创建目录
  - 创建新目录
  - 设置权限
  - 记录审计日志

- [ ] rmdir - 删除目录
  - 检查目录是否为空
  - 删除目录 inode
  - 记录审计日志

- [ ] opendir - 打开目录
  - 权限检查
  - 分配目录句柄

- [ ] readdir - 读取目录
  - 列出目录项
  - 支持分页（offset）
  - 返回 `.` 和 `..`
  - 返回文件类型

- [ ] releasedir - 关闭目录
  - 释放目录句柄
  - 清理资源

### 4.5 链接操作实现

- [ ] mknod - 创建节点
  - 创建普通文件
  - 支持不同文件类型

- [ ] link - 创建硬链接
  - 增加 inode 引用计数
  - 在目录中添加条目

- [ ] symlink - 创建符号链接
  - 创建符号链接 inode
  - 存储目标路径

- [ ] unlink - 删除文件
  - 删除目录项
  - 减少 inode 引用计数
  - 清理无引用的 inode

- [ ] rename - 重命名/移动
  - 原子操作
  - 处理同目录和跨目录情况
  - 处理覆盖已存在文件
  - 记录审计日志

### 4.6 权限和所有权操作

- [ ] access - 检查访问权限
  - 检查读/写/执行权限
  - 返回是否允许访问

- [ ] chmod - 修改权限
  - 更新 inode 权限位
  - 权限验证

- [ ] chown - 修改所有者
  - 更新 uid/gid
  - 权限验证（需要 root）

### 4.7 扩展属性操作（可选）

- [ ] setxattr - 设置扩展属性
  - 存储扩展属性
  - 验证属性名和值

- [ ] getxattr - 获取扩展属性
  - 读取扩展属性值

- [ ] listxattr - 列出扩展属性
  - 返回所有属性名

- [ ] removexattr - 删除扩展属性
  - 删除指定属性

### 4.8 文件系统信息

- [ ] statfs - 获取文件系统统计
  - 返回容量信息
  - 返回使用量
  - 返回 inode 数量
  - 基于租户配额计算

### 4.9 特殊功能

- [ ] 虚拟文件系统支持
  - `/.tarbox/` 目录实现
  - `/.tarbox/layers/` 层管理
  - `/.tarbox/layers/current` - 当前层信息
  - `/.tarbox/layers/list` - 层列表
  - `/.tarbox/layers/new` - 创建新层
  - `/.tarbox/layers/switch` - 切换层

- [ ] 文件系统 Hook 实现
  - 监听特殊文件的写入
  - 解析命令并执行
  - 返回结果

### 4.10 性能优化

- [ ] 实现缓存
  - 利用文件系统核心的缓存
  - FUSE 内核缓存配置
  - 合理设置超时时间

- [ ] 实现预读
  - 检测顺序访问模式
  - 预读后续数据块
  - 配置预读大小

- [ ] 实现写缓冲
  - 合并小写入
  - 批量刷新
  - 配置缓冲大小

- [ ] 实现直接 I/O（可选）
  - 支持 O_DIRECT 标志
  - 绕过缓存的 I/O

### 4.11 错误处理

- [ ] 实现错误映射
  - 文件系统错误 -> FUSE errno
  - 正确返回 errno 代码
  - 保留错误信息用于日志

- [ ] 实现超时处理
  - 设置操作超时
  - 处理挂起的请求
  - 避免 FUSE 挂起

### 4.12 日志和调试

- [ ] 实现操作日志
  - 记录所有 FUSE 操作
  - 可配置日志级别
  - 包含性能指标（延迟）

- [ ] 实现调试模式
  - 详细的请求/响应日志
  - 参数验证
  - 断言检查

### 4.13 平台兼容性

- [ ] Linux 支持
  - 使用 libfuse3
  - 测试常见发行版（Ubuntu, CentOS）

- [ ] macOS 支持（可选）
  - 使用 macFUSE
  - 处理平台差异

- [ ] 权限模型差异处理
  - 处理不同平台的权限位
  - 处理特殊权限（setuid 等）

### 4.14 测试

- [ ] 单元测试
  - 模拟 FUSE 请求
  - 测试每个操作
  - 测试错误处理

- [ ] 集成测试
  - 实际挂载文件系统
  - 使用标准工具测试（ls, cat, cp 等）
  - 测试并发访问

- [ ] 兼容性测试
  - POSIX 兼容性测试套件
  - 各种文件操作组合
  - 边界情况测试

- [ ] 性能测试
  - 文件操作延迟
  - 吞吐量测试
  - 并发性能测试

- [ ] 稳定性测试
  - 长时间运行测试
  - 压力测试
  - 异常情况测试

## 验收标准

- [ ] 文件系统可以成功挂载
- [ ] 支持所有基本 POSIX 操作
- [ ] 可以使用标准工具（ls, cat, vim 等）
- [ ] 并发访问正确
- [ ] 性能满足目标（P99 < 10ms）
- [ ] 所有测试通过
- [ ] 稳定运行无崩溃

## 预估时间

7-10 天

## 技术要点

- 使用 fuser 0.16
- 正确实现 FUSE 回调函数
- 处理异步操作（FUSE 是同步的，需要桥接）
- 注意 FUSE 的 inode 和我们的 inode 的映射
- 正确设置文件类型和权限位
- 处理特殊文件（`.` 和 `..`）
- 实现虚拟文件系统（`/.tarbox/`）

## 注意事项

- FUSE 操作是同步的，需要在异步运行时中桥接
- 注意处理 FUSE 内核的超时
- 文件句柄需要正确管理和释放
- 租户上下文必须在挂载时确定
- 虚拟文件要特殊处理，不存储到数据库
- 错误码必须符合 POSIX 标准
- 测试时注意权限问题（可能需要 root 或 fuse 组）

## 后续任务

完成后可以开始：
- Task 05: 分层文件系统实现
- Task 06: 审计系统实现

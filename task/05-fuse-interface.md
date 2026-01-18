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

**不在本任务范围内（由高级文件系统功能实现）：**
- 符号链接操作 (create_symlink, read_symlink) - 接口已定义，返回 NotSupported
- 硬链接操作 (create_hardlink) - 接口已定义，返回 NotSupported
- 扩展属性 (setxattr, getxattr, listxattr, removexattr) - 接口已定义，返回 NotSupported

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

- [x] 实现类型转换辅助函数
  - [x] inode_to_file_attr()
  - [x] inode_type_to_file_type()

### 5.3 实现 FuseAdapter

**基于**: spec/02-fuse-interface.md

- [x] 创建 FuseAdapter 结构
  - [x] 持有 Arc<dyn FilesystemInterface>
  - [x] 持有 Runtime handle (用于 block_on)
  - [x] inode ↔ path 映射表 (InodeMap)
  - [x] 提供 new() 构造函数

- [x] 实现 fuser::Filesystem trait
  - [x] 所有回调委托给 FilesystemInterface
  - [x] 使用 block_on 桥接异步到同步
  - [x] FUSE 类型 → FilesystemInterface 类型转换

- [x] 实现挂载管理
  - [x] `mount()` - 挂载文件系统
  - [x] `unmount()` - 卸载文件系统
  - [x] MountOptions 结构（挂载点、选项等）
  - [x] 后台运行模式 (BackgroundSession)

- [x] 实现租户上下文
  - [x] 挂载时绑定租户
  - [x] 所有操作自动注入 tenant_id (通过 TarboxBackend)
  - [x] 租户验证（在 CLI 层）

### 5.4 元数据操作实现 ✅

- [x] init - 初始化文件系统
  - [x] 设置文件系统能力标志
  - [x] 初始化 inode 映射表

- [x] lookup - 路径查找
  - [x] 将文件名解析为 inode
  - [x] 返回文件属性
  - [x] 建立 inode ↔ path 映射

- [x] getattr - 获取文件属性
  - [x] 返回 POSIX stat 结构
  - [x] 填充所有必需字段

- [x] setattr - 设置文件属性
  - [x] 修改大小 (truncate)
  - [x] 修改权限 (chmod)
  - [x] 修改所有者 (chown)
  - [x] 修改时间戳 (atime, mtime)

### 5.5 文件操作实现 ✅

- [x] open - 打开文件
  - [x] 返回文件句柄 (当前为 dummy handle)

- [x] read - 读取文件
  - [x] 从指定偏移量读取
  - [x] 处理读取大小

- [x] write - 写入文件
  - [x] 写入到指定偏移量
  - [x] 处理写入大小

- [x] release - 关闭文件
  - [x] 清理资源

- [ ] flush - 刷新文件 (暂未实现)
- [ ] fsync - 同步文件 (暂未实现)

### 5.6 目录操作实现 ✅

- [x] mkdir - 创建目录
  - [x] 创建新目录
  - [x] 设置权限

- [x] rmdir - 删除目录
  - [x] 检查目录是否为空 (后端处理)
  - [x] 删除目录 inode

- [ ] access - 检查访问权限
  - 检查读/写/执行权限
  - 返回是否允许访问

- [ ] chmod - 修改权限
  - 更新 inode 权限位
  - 支持可执行权限 (chmod +x)
  - 权限验证

- [ ] chown - 修改所有者
  - 更新 uid/gid
  - 权限验证（需要 root）

### 5.9 文件系统信息

- [ ] statfs - 获取文件系统统计
  - 返回容量信息
  - 返回使用量
  - 返回 inode 数量
  - 基于租户配额计算

### 5.10 性能优化

- [ ] 实现缓存
  - FUSE 内核缓存配置
  - 合理设置超时时间
  - 利用底层文件系统缓存

- [ ] 实现预读
  - 检测顺序访问模式
  - 预读后续数据块
  - 配置预读大小

- [ ] 实现写缓冲
  - 合并小写入
  - 批量刷新
  - 配置缓冲大小

### 5.11 错误处理

- [ ] 实现错误映射
  - FilesystemInterface 错误 → FUSE errno
  - 正确返回 errno 代码
  - 保留错误信息用于日志

- [ ] 实现超时处理
  - 设置操作超时
  - 处理挂起的请求
  - 避免 FUSE 挂起

### 5.12 日志和调试

- [ ] 实现操作日志
  - 记录所有 FUSE 操作
  - 可配置日志级别
  - 包含性能指标（延迟）

- [ ] 实现调试模式
  - 详细的请求/响应日志
  - 参数验证
  - 断言检查

### 5.13 平台支持

- [ ] Linux 支持
  - 使用 libfuse3
  - 测试常见发行版（Ubuntu, CentOS）

- [ ] 权限模型实现
  - 实现 POSIX 权限位
  - 处理特殊权限（setuid, setgid, sticky bit）

### 5.14 测试

- [ ] 单元测试
  - 测试 FuseAdapter 各个方法
  - 测试错误处理
  - 测试类型转换

- [ ] 集成测试
  - 实际挂载文件系统
  - 使用标准工具测试（ls, cat, cp, chmod 等）
  - 测试并发访问

- [ ] 兼容性测试
  - POSIX 兼容性测试
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

## 不在本任务范围内

以下功能由后续高级文件系统任务实现：

### 符号链接和硬链接
- `symlink()` - 创建符号链接
- `readlink()` - 读取符号链接
- `link()` - 创建硬链接
- **接口行为**: FilesystemInterface 中已定义，返回 ENOSYS (NotSupported)
- **实现位置**: 高级文件系统功能任务

### 扩展属性
- `setxattr()` - 设置扩展属性
- `getxattr()` - 获取扩展属性
- `listxattr()` - 列出扩展属性
- `removexattr()` - 删除扩展属性
- **接口行为**: FilesystemInterface 中已定义，返回 ENOSYS (NotSupported)
- **实现位置**: 高级文件系统功能任务

### 直接 I/O
- O_DIRECT 标志支持
- 绕过缓存的 I/O
- **实现位置**: 性能优化任务

### macOS 支持
- macFUSE 集成
- 平台特定适配
- **实现位置**: 跨平台支持任务

### 虚拟文件系统 (/.tarbox/)
- `/.tarbox/` 目录实现
- 层管理 hook
- 文件系统控制接口
- **实现位置**: 分层文件系统任务

## 验收标准

- [ ] 文件系统可以成功挂载到指定挂载点
- [ ] 支持所有基本 POSIX 文件操作（创建、读写、删除）
- [ ] 支持所有基本 POSIX 目录操作（创建、列出、删除）
- [ ] 支持权限操作（chmod, chown, access），包括 chmod +x
- [ ] 可以使用标准 Unix 工具（ls, cat, vim, chmod 等）
- [ ] 并发访问正确无数据竞争
- [ ] 性能满足目标（P99 延迟 < 10ms）
- [ ] 所有单元测试和集成测试通过
- [ ] 稳定运行无崩溃或内存泄漏

## 预估时间

7-10 天

## 技术要点

- 使用 fuser 0.16 crate
- 正确实现 fuser::Filesystem trait 的所有必需回调
- 使用 tokio::runtime::Handle::block_on 桥接异步到同步
- 维护 inode ID ↔ path 的双向映射表
- 正确设置 FUSE 文件类型和权限位
- 处理特殊目录项（`.` 和 `..`）
- 实现租户隔离（挂载时绑定 tenant_id）

## 注意事项

- FUSE 回调是同步的，需要在 tokio 运行时中桥接异步操作
- 注意处理 FUSE 内核的超时（默认 10 秒）
- 文件句柄需要正确管理和释放，避免泄漏
- 租户上下文必须在挂载时确定，不可更改
- 错误码必须严格符合 POSIX 标准
- 测试时注意权限问题（挂载需要 root 或 fuse 组权限）
- inode 映射表需要处理并发访问，使用适当的锁机制

## 后续任务

完成后可以开始：
- 分层文件系统实现
- 审计系统实现
- 高级文件系统功能（符号链接、扩展属性等）

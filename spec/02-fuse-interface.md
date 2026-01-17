# Spec 02: FUSE 接口设计

**优先级**: P0 (核心架构)  
**状态**: 设计阶段  
**依赖**: spec/14 (文件系统接口抽象层)  
**基于**: [spec/14-filesystem-interface.md](14-filesystem-interface.md)

## 概述

FUSE（Filesystem in Userspace）允许在用户空间实现文件系统，无需编写内核模块。Tarbox 通过 FUSE 提供标准 POSIX 文件系统接口。

**本规范描述 FUSE 适配器的具体实现细节**，包括 POSIX 系统调用映射、异步/同步桥接、性能优化等。核心的文件系统操作语义由 spec/14 定义。

## 架构定位

```
应用程序
    ↓
POSIX 系统调用 (open/read/write/...)
    ↓
内核 VFS 层
    ↓
FUSE 内核模块
    ↓
libfuse (用户空间)
    ↓
┌─────────────────────────────────┐
│  FuseAdapter (本规范)           │  ← 协议适配层
│  - POSIX → Interface 映射       │
│  - 异步/同步桥接                │
│  - errno 错误码转换             │
└─────────────────────────────────┘
    ↓ 实现 FilesystemInterface trait
┌─────────────────────────────────┐
│  FilesystemInterface (spec/14)  │  ← 统一抽象层
└─────────────────────────────────┘
    ↓
┌─────────────────────────────────┐
│  TarboxBackend                  │  ← 后端实现
│  (fs/ + storage/ + layer/)      │
└─────────────────────────────────┘
```

## 设计目标

### 功能目标

- **完整 POSIX 兼容**：支持标准文件操作
- **多租户隔离**：每个挂载点绑定一个租户
- **高性能**：最小化用户态和内核态切换开销
- **稳定可靠**：正确处理各种边界情况
- **易于调试**：提供详细的日志和错误信息

### 非功能目标

- **并发安全**：支持多线程并发访问
- **租户隔离**：租户间完全隔离，互不可见
- **错误处理**：优雅处理各种错误场景
- **资源管理**：正确管理文件句柄和内存

## 多租户集成

### 挂载点与租户绑定

**设计原则**：
- 每个挂载点对应一个租户
- 挂载时指定 tenant_id 或 tenant_name
- 挂载后租户上下文不可更改
- 所有文件操作自动注入租户 ID

**挂载命令**：
```bash
# 通过租户名称挂载
tarbox mount \
  --mount-point /mnt/agent-001 \
  --tenant ai-agents-agent001 \
  --database postgresql://...

# 通过租户 ID 挂载（内部使用）
tarbox mount \
  --mount-point /mnt/agent-001 \
  --tenant-id 550e8400-e29b-41d4-a716-446655440000 \
  --database postgresql://...
```

### 租户上下文传递

**实现要点**：
- FUSE 初始化时从挂载选项获取 tenant_id
- 存储在全局上下文或文件系统状态中
- 每个 FUSE 操作自动携带租户上下文
- 所有数据库查询自动添加 WHERE tenant_id = ?

**租户验证**：
```
挂载时验证：
1. 检查租户是否存在
2. 检查租户状态（active/suspended/deleted）
3. 验证访问权限
4. 加载租户配置和配额
```

### 数据隔离保证

**路径解析**：
- 每个租户看到的根目录是独立的
- 路径 `/data/file.txt` 解析为 `(tenant_id, inode_id)`
- 不同租户的相同路径指向完全不同的文件

**示例**：
```
租户 A 访问 /data/file.txt：
- 查询：SELECT * FROM inodes 
         WHERE tenant_id = 'tenant-A' 
         AND parent_id = 1 AND name = 'data'
- 结果：(tenant-A, inode_12345)

租户 B 访问 /data/file.txt：
- 查询：SELECT * FROM inodes 
         WHERE tenant_id = 'tenant-B' 
         AND parent_id = 1 AND name = 'data'
- 结果：(tenant-B, inode_67890)

完全不同的文件，完全隔离
```

## 路径解析与路由

### 原生挂载检查

在所有 FUSE 操作开始时，首先检查路径是否匹配原生挂载配置：

```
路径解析流程：
1. 接收 FUSE 请求（带路径）
2. 检查 native_mounts 配置
   - 按 priority 排序
   - 匹配路径前缀
   - 检查租户权限
3. 如果匹配原生挂载：
   - 验证访问模式（ro/rw）
   - 替换路径变量（{tenant_id}）
   - 透传到原生文件系统
4. 如果不匹配：
   - 正常路径解析（PostgreSQL）
   - 查询 inodes 表
```

**原生挂载优先级**：
- 精确匹配 > 前缀匹配
- 更长路径 > 较短路径
- 更小 priority 值 > 更大 priority 值
- 租户专属 > 共享挂载

### 路径透传

对于原生挂载的路径，操作直接转发到宿主机文件系统：

```rust
// 伪代码
fn handle_fuse_operation(path: &str, op: Operation) -> Result<()> {
    // 1. 检查原生挂载
    if let Some(mount) = check_native_mount(path, tenant_id) {
        validate_mount_access(mount, op)?;
        let native_path = resolve_native_path(mount.source_path, tenant_id);
        return forward_to_native_fs(native_path, op);
    }
    
    // 2. 正常 Tarbox 处理
    let inode = resolve_path(path, tenant_id)?;
    handle_tarbox_operation(inode, op)
}
```

## FUSE 操作映射

### 元数据操作

#### getattr - 获取文件属性

**输入**：文件路径
**输出**：文件属性（stat 结构）

```
用途：ls, stat 等命令
实现：
1. 检查原生挂载（如匹配则透传）
2. 路径解析 -> inode_id
3. 查询 inodes 表
4. 返回 POSIX 属性
```

#### readdir - 读取目录

**输入**：目录路径
**输出**：目录项列表

```
用途：ls 命令
实现：
1. 解析目录 inode
2. 查询所有 parent_id = 目录id 的 inode
3. 返回名称列表
```

#### mkdir - 创建目录

**输入**：路径、权限
**输出**：成功/失败

```
用途：mkdir 命令
实现：
1. 解析父目录
2. 检查权限
3. 创建新 inode（type=dir）
4. 记录审计日志
```

#### rmdir - 删除目录

**输入**：路径
**输出**：成功/失败

```
用途：rmdir 命令
实现：
1. 检查目录是否为空
2. 检查权限
3. 标记 inode 为已删除
4. 记录审计日志
```

### 文件操作

#### create - 创建文件

**输入**：路径、模式、权限
**输出**：文件句柄

```
用途：touch, 创建新文件
实现：
1. 解析父目录
2. 创建 inode（type=file）
3. 返回文件句柄
4. 记录审计日志
```

#### open - 打开文件

**输入**：路径、打开标志
**输出**：文件句柄

```
用途：打开已存在文件
实现：
1. 路径解析 -> inode
2. 检查权限（读/写/执行）
3. 分配文件句柄
4. 初始化读写上下文
```

#### read - 读取文件

**输入**：文件句柄、偏移量、大小
**输出**：数据

```
用途：读取文件内容

实现（智能路由）：
1. 检查文件类型（文本 vs 二进制）
   - 查询 text_file_metadata 表
   - 如果存在则为文本文件，否则为二进制文件

2a. 文本文件读取：
   - 查询 text_line_map 获取行映射
   - 批量读取相关的 text_blocks
   - 重组文件内容
   - 根据偏移量和大小返回数据切片
   - 缓存重组结果

2b. 二进制文件读取：
   - 计算需要的块范围
   - 从 data_blocks 表读取数据块
   - 拼接数据
   
3. 更新 atime
4. 记录审计日志
```

#### write - 写入文件

**输入**：文件句柄、偏移量、数据
**输出**：写入字节数

```
用途：写入文件内容

实现（智能路由）：
1. 检测文件类型：
   - 首次写入：检测数据内容（UTF-8、行结构）
   - 已存在：检查 text_file_metadata 或 data_blocks
   
2a. 文本文件写入：
   - 如果是完整覆盖（offset=0）：
     * 解析新内容为行
     * 与父层版本计算 diff（如果存在）
     * 创建/更新 text_blocks（只存储变化的行）
     * 创建/更新 text_line_map
     * 更新 text_file_metadata
   - 如果是追加写入（offset=EOF）：
     * 解析新增行
     * 添加新的 text_blocks
     * 扩展 text_line_map
   - 如果是部分修改：
     * 临时降级为二进制处理（或者完整重组后再解析）

2b. 二进制文件写入：
   - 计算影响的块范围
   - 读取-修改-写入受影响的块
   - 插入/更新 data_blocks 表
   
3. 更新 inode size 和 mtime
4. 记录审计日志（文本文件记录行级变化）
```

#### release - 关闭文件

**输入**：文件句柄
**输出**：成功/失败

```
用途：关闭文件
实现：
1. 刷新缓冲区
2. 释放文件句柄
3. 清理资源
```

#### unlink - 删除文件

**输入**：路径
**输出**：成功/失败

```
用途：rm 命令
实现：
1. 解析 inode
2. 减少 nlinks 计数
3. 如果 nlinks=0，标记删除
4. 异步删除数据块
5. 记录审计日志
```

### 链接操作

#### symlink - 创建符号链接

**输入**：目标路径、链接路径
**输出**：成功/失败

```
实现：
1. 创建 inode（type=symlink）
2. 设置 link_target 字段
```

#### readlink - 读取符号链接

**输入**：链接路径
**输出**：目标路径

```
实现：
1. 解析 inode
2. 返回 link_target 字段
```

#### link - 创建硬链接

**输入**：目标路径、链接路径
**输出**：成功/失败

```
实现：
1. 解析目标 inode
2. 创建新目录项指向同一 inode
3. 增加 nlinks 计数
```

### 权限操作

#### chmod - 修改权限

**输入**：路径、新权限
**输出**：成功/失败

```
实现：
1. 检查是否为所有者
2. 更新 inode mode 字段
3. 更新 ctime
```

#### chown - 修改所有者

**输入**：路径、新 uid/gid
**输出**：成功/失败

```
实现：
1. 检查权限（需要 root 或所有者）
2. 更新 inode uid/gid 字段
3. 更新 ctime
```

### 扩展属性

#### setxattr - 设置扩展属性

**输入**：路径、键、值
**输出**：成功/失败

```
实现：
1. 解析 inode
2. 更新 xattrs JSONB 字段
```

#### getxattr - 获取扩展属性

**输入**：路径、键
**输出**：值

```
实现：
1. 解析 inode
2. 从 xattrs JSONB 字段查询
```

#### listxattr - 列出扩展属性

**输入**：路径
**输出**：键列表

```
实现：
1. 解析 inode
2. 返回 xattrs 所有键
```

## 性能优化

### 路径解析优化

```
问题：每次操作都要解析完整路径
解决：路径缓存

路径缓存设计：
- 键：完整路径
- 值：inode_id
- 淘汰：LRU
- 失效：路径相关操作（重命名、删除）触发失效
```

### 元数据缓存

```
Inode 缓存：
- 键：inode_id
- 值：完整 inode 结构
- TTL：60秒
- 更新：写入时更新缓存
```

### 数据块缓存

```
块缓存：
- 键：(inode_id, block_index)
- 值：数据块内容
- 大小：可配置（如 4GB）
- 策略：LRU
```

### 预读策略

```
顺序读检测：
- 检测连续的读请求
- 触发预读后续块
- 预读大小：动态调整（如 1MB - 4MB）
```

### 写合并

```
小写入缓冲：
- 缓冲小于 4KB 的写入
- 定期刷新或缓冲满时刷新
- 减少数据库写入次数
```

## 并发控制

### 文件锁

```
支持 POSIX 文件锁：
- flock：整个文件锁
- fcntl：字节范围锁

实现：
- 在内存维护锁表
- 检查锁冲突
- 支持阻塞和非阻塞模式
```

### 并发写入

```
策略：
1. 不同文件：完全并行
2. 同文件不同块：并行写入
3. 同文件同块：串行化（通过锁）
```

## 错误处理

### 常见错误码

```
- ENOENT: 文件不存在
- EACCES: 权限拒绝
- EEXIST: 文件已存在
- ENOTDIR: 不是目录
- EISDIR: 是目录
- ENOTEMPTY: 目录非空
- ENOSPC: 空间不足
- EIO: 输入输出错误
```

### 数据库错误处理

```
连接失败：
- 重试机制（指数退避）
- 失败后返回 EIO

事务冲突：
- 自动重试
- 超过次数返回 EAGAIN

约束违反：
- 转换为对应的文件系统错误
```

## 审计集成

### 审计点

```
记录以下操作：
- 文件创建/删除
- 文件读写
- 权限修改
- 目录操作

记录信息：
- 操作类型
- 文件路径
- 用户信息（uid/gid/pid）
- 操作结果
- 时间戳
```

### 性能考虑

```
审计策略：
- 异步写入：不阻塞文件操作
- 批量插入：合并多条审计记录
- 选择性审计：根据配置决定审计级别
```

## 挂载选项

### 支持的选项

```
基础选项：
- ro: 只读挂载
- rw: 读写挂载
- uid: 覆盖所有文件的 uid
- gid: 覆盖所有文件的 gid
- allow_other: 允许其他用户访问
- allow_root: 允许 root 访问

Tarbox 特定：
- database_url: 数据库连接串
- cache_size: 缓存大小
- block_size: 块大小
- audit_level: 审计级别（none/basic/full）
- mount_namespace: 挂载命名空间（多租户）
```

### 配置示例

```bash
tarbox mount \
  --mount-point /mnt/tarbox \
  --database-url postgresql://user:pass@host/db \
  --cache-size 4G \
  --audit-level full \
  --namespace agent-1
```

## 诊断和调试

### 日志级别

```
- ERROR: 错误信息
- WARN: 警告信息
- INFO: 一般信息
- DEBUG: 调试信息
- TRACE: 跟踪级别（包含所有操作）
```

### 性能统计

```
实时统计：
- 操作计数（按类型）
- 平均延迟
- 缓存命中率
- 数据库连接池状态

导出：
- 标准输出
- Prometheus 指标
- 日志文件
```

### 调试工具

```
内置命令：
- tarbox stats: 显示实时统计
- tarbox cache: 缓存状态
- tarbox connections: 数据库连接状态
- tarbox audit: 审计日志查询
```

## 与其他组件交互

### VFS 层

```
FUSE -> VFS：
- FUSE 接收系统调用
- 转换为 VFS API 调用
- VFS 处理具体逻辑
```

### 缓存层

```
FUSE -> Cache：
- 优先从缓存读取
- 缓存未命中时查询数据库
- 写入时更新缓存
```

### 审计层

```
FUSE -> Audit：
- 每个操作触发审计记录
- 异步写入审计日志
- 不阻塞主流程
```

## 测试策略

### 功能测试

```
- 基本操作：创建、读写、删除
- 目录操作：mkdir、rmdir、readdir
- 权限测试：chmod、chown、访问控制
- 链接测试：symlink、hardlink
- 边界测试：大文件、深目录、特殊字符
```

### 性能测试

```
- 小文件性能：大量小文件创建和读写
- 大文件性能：大文件顺序/随机读写
- 并发性能：多线程并发操作
- 缓存效果：缓存命中率测试
```

### 稳定性测试

```
- 长时间运行测试
- 数据库连接中断恢复
- 内存泄漏检测
- 异常场景测试
```

## 实现注意事项

### Rust 库选择

```
推荐：fuser crate
- 现代化的 Rust FUSE 库
- async/await 支持
- 类型安全
- 活跃维护
```

### 生命周期管理

```
关键点：
- 文件句柄生命周期
- 缓存数据生命周期
- 数据库连接生命周期
- 正确的资源清理
```

### 错误传播

```
策略：
- 使用 Result<T, E>
- 自定义错误类型
- 错误转换层
- 统一的错误处理
```

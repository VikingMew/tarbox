# Spec 12: 原生目录挂载

## 优先级

**P3 - 可选功能（可延后或使用替代方案）**

## 替代方案

推荐使用 **bubblewrap** 在容器启动时实现目录挂载，而不是在 Tarbox 内部实现此功能。

**理由**：
1. 单一职责原则 - Tarbox 专注于文件系统本身
2. 性能相同或更好 - bubblewrap 直接在内核层面操作
3. 减少复杂度 - 无需在 Tarbox 中维护挂载逻辑
4. 更灵活 - 容器编排层可以自由控制挂载策略

**示例**：
```bash
bwrap \
  --ro-bind /usr /usr \
  --ro-bind /bin /bin \
  --bind /host/venv /.venv \
  --bind /tarbox/tenant/data /data \
  /bin/bash
```

## 概述

（如果需要在 Tarbox 内部实现）

为了提高性能和资源利用率，Tarbox 支持将特定目录挂载到宿主机的原生文件系统。这允许多个租户共享只读的系统目录（如 `/bin`、`/usr`），或将虚拟环境（如 `.venv`）挂载到本地存储以提高 I/O 性能。

## 使用场景

### 系统目录共享
- `/bin` - 系统二进制文件（只读）
- `/usr` - 用户程序和库（只读）
- `/lib`, `/lib64` - 系统库（只读）
- `/etc` - 系统配置（只读或读写）

### 性能优化
- `.venv` - Python 虚拟环境（读写）
- `node_modules` - Node.js 依赖（读写）
- `.cargo` - Rust 依赖缓存（读写）
- `build/` - 编译输出目录（读写）

### 数据共享
- `/data/models` - 共享的 AI 模型文件（只读）
- `/data/datasets` - 共享的数据集（只读）

## 配置方式

### 配置文件

在 `config.toml` 中定义原生挂载：

```toml
[[native_mounts]]
# 挂载路径（在 Tarbox 文件系统中的路径）
path = "/bin"
# 原生系统路径
source = "/bin"
# 访问模式：ro（只读）或 rw（读写）
mode = "ro"
# 是否跨租户共享
shared = true
# 可选：只对特定租户启用
# tenants = ["tenant-uuid-1", "tenant-uuid-2"]

[[native_mounts]]
path = "/usr"
source = "/usr"
mode = "ro"
shared = true

[[native_mounts]]
path = "/.venv"
source = "/var/tarbox/venvs/{tenant_id}"
mode = "rw"
shared = false  # 每个租户独立

[[native_mounts]]
path = "/data/models"
source = "/mnt/shared/models"
mode = "ro"
shared = true
```

### 动态变量

Source 路径支持变量替换：
- `{tenant_id}` - 当前租户 ID
- `{mount_id}` - 挂载点 ID（FUSE 挂载实例）
- `{user}` - 用户名（如果提供）

示例：
```toml
[[native_mounts]]
path = "/.cache"
source = "/var/tarbox/cache/{tenant_id}"
mode = "rw"
shared = false
```

## 工作机制

### 路径解析优先级

1. 检查路径是否匹配 native_mount 配置
2. 如果匹配：
   - 验证租户是否有权限访问
   - 如果是共享挂载，检查是否已挂载
   - 将操作透传到原生文件系统
3. 如果不匹配：
   - 正常处理（从 PostgreSQL 读取）

### FUSE 层实现

```
FUSE Request
    ↓
路径匹配检查
    ↓
是否为 native_mount？
    ├─ Yes → 权限检查 → 透传到原生 FS
    └─ No  → 正常 Tarbox 处理
```

### 透传操作

对于匹配的路径，以下操作直接透传到原生文件系统：
- `read()` - 读取文件
- `write()` - 写入文件（如果 mode = rw）
- `readdir()` - 列出目录
- `getattr()` - 获取属性
- `open()` - 打开文件
- `lookup()` - 路径查找

只读挂载会拒绝以下操作：
- `write()` - 返回 EROFS
- `unlink()` - 返回 EROFS
- `mkdir()` - 返回 EROFS
- `rmdir()` - 返回 EROFS

## 数据库设计

### native_mounts 表

```sql
CREATE TABLE native_mounts (
    mount_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- 挂载路径（虚拟路径）
    mount_path TEXT NOT NULL,
    
    -- 原生系统路径
    source_path TEXT NOT NULL,
    
    -- 访问模式
    mode VARCHAR(2) NOT NULL CHECK (mode IN ('ro', 'rw')),
    
    -- 是否跨租户共享
    is_shared BOOLEAN NOT NULL DEFAULT false,
    
    -- 如果不是 shared，可以指定特定租户
    tenant_id UUID REFERENCES tenants(tenant_id),
    
    -- 启用状态
    enabled BOOLEAN NOT NULL DEFAULT true,
    
    -- 优先级（用于路径匹配，数字越小优先级越高）
    priority INTEGER NOT NULL DEFAULT 100,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    
    -- 确保路径唯一性
    UNIQUE(mount_path, tenant_id)
);

CREATE INDEX idx_native_mounts_path ON native_mounts(mount_path);
CREATE INDEX idx_native_mounts_tenant ON native_mounts(tenant_id);
CREATE INDEX idx_native_mounts_enabled ON native_mounts(enabled);
```

### tenant_mounts 表（可选）

如果需要为特定租户启用/禁用某些挂载：

```sql
CREATE TABLE tenant_mounts (
    tenant_id UUID NOT NULL REFERENCES tenants(tenant_id),
    mount_id UUID NOT NULL REFERENCES native_mounts(mount_id),
    
    -- 租户级别的覆盖配置
    enabled BOOLEAN NOT NULL DEFAULT true,
    
    PRIMARY KEY (tenant_id, mount_id)
);
```

## 权限控制

### 共享挂载（shared = true）

- 所有租户可以访问
- 只读挂载：所有租户只能读
- 读写挂载：所有租户可以读写（需谨慎，可能产生冲突）

### 非共享挂载（shared = false）

- 必须指定 tenant_id
- 只有该租户可以访问
- 适合租户独立的工作目录

### 访问控制流程

```
1. FUSE 收到请求，解析路径
2. 查询 native_mounts 表匹配路径
3. 检查 is_shared：
   - true: 允许访问
   - false: 检查 tenant_id 是否匹配
4. 检查操作类型：
   - 读操作: 总是允许
   - 写操作: 检查 mode = 'rw'
5. 透传到原生文件系统
```

## 路径匹配规则

### 精确匹配

优先进行精确路径匹配：
```
mount_path = "/bin"
request_path = "/bin/ls"  → 匹配
request_path = "/usr/bin/ls"  → 不匹配
```

### 前缀匹配

路径前缀匹配：
```
mount_path = "/usr"
request_path = "/usr/bin/gcc"  → 匹配
request_path = "/usr/local/bin/node"  → 匹配
request_path = "/bin/bash"  → 不匹配
```

### 优先级

当多个挂载匹配时，按以下顺序选择：
1. 精确匹配优先于前缀匹配
2. 更长的路径优先（更具体）
3. priority 值更小的优先
4. 租户专属挂载优先于共享挂载

示例：
```toml
# 优先级排序
[[native_mounts]]
path = "/usr/local"  # 最高优先级（最长路径）
source = "/opt/local"
mode = "ro"
priority = 10

[[native_mounts]]
path = "/usr"  # 次优先级
source = "/usr"
mode = "ro"
priority = 20
```

## 审计日志

原生挂载的操作也需要记录审计日志，但使用特殊标记：

```sql
-- 审计日志增强
ALTER TABLE audit_logs ADD COLUMN is_native_mount BOOLEAN DEFAULT false;
ALTER TABLE audit_logs ADD COLUMN native_source_path TEXT;
```

记录内容：
- 操作类型（read, write, 等）
- 虚拟路径（mount_path）
- 原生路径（source_path）
- 租户 ID
- 时间戳
- 是否成功

## 缓存策略

### 挂载配置缓存

- 启动时加载所有 native_mounts 配置
- 构建路径树用于快速匹配
- 配置更新时刷新缓存

### 文件元数据缓存

对于只读共享挂载，可以缓存：
- stat 结果（文件大小、权限、时间戳）
- readdir 结果（目录列表）

缓存失效：
- TTL 超时（可配置，如 60 秒）
- 手动刷新命令

## 安全考虑

### 路径安全

- 验证 source_path 不能包含 `..` 路径遍历
- 限制只能挂载特定白名单目录
- 防止挂载到敏感系统路径（如 `/proc`, `/sys`）

### 权限隔离

- 进程运行用户需要对 source_path 有相应权限
- 只读挂载强制执行，不能通过权限绕过
- 跨租户共享挂载需要管理员审核

### 配额管理

对于读写挂载，可以设置：
- 磁盘配额（如果底层 FS 支持）
- 文件数量限制
- I/O 速率限制

## 性能考虑

### 优势

- 绕过 PostgreSQL，直接访问原生 FS
- 减少网络往返（如果 PG 是远程的）
- 利用操作系统页缓存
- 更低的延迟

### 劣势

- 不支持 Tarbox 的分层特性
- 不支持 Tarbox 的内容去重
- 需要额外的存储空间（非共享挂载）

### 适用场景判断

适合使用原生挂载：
- 大量小文件读取（如 node_modules）
- 频繁的随机访问
- 系统工具和库（只读共享）
- 临时构建目录

不适合使用原生挂载：
- 需要版本控制的文件
- 需要审计的敏感数据
- 需要跨节点访问的数据

## API 设计

### 管理 API

```rust
// 创建原生挂载
async fn create_native_mount(
    mount_path: String,
    source_path: String,
    mode: MountMode,
    shared: bool,
    tenant_id: Option<Uuid>,
) -> Result<Uuid>;

// 列出原生挂载
async fn list_native_mounts(
    tenant_id: Option<Uuid>
) -> Result<Vec<NativeMount>>;

// 删除原生挂载
async fn delete_native_mount(mount_id: Uuid) -> Result<()>;

// 更新原生挂载
async fn update_native_mount(
    mount_id: Uuid,
    enabled: Option<bool>,
    mode: Option<MountMode>,
) -> Result<()>;
```

### CLI 命令

```bash
# 列出所有原生挂载
tarbox mount list

# 添加原生挂载
tarbox mount add /bin /bin --ro --shared

# 添加租户专属挂载
tarbox mount add /.venv /var/tarbox/venvs/{tenant_id} --rw --tenant <tenant-id>

# 删除挂载
tarbox mount remove <mount-id>

# 启用/禁用挂载
tarbox mount enable <mount-id>
tarbox mount disable <mount-id>
```

## 实现步骤

1. **数据库 Schema**
   - 创建 native_mounts 表
   - 添加审计日志字段

2. **配置解析**
   - 解析 config.toml 中的 native_mounts 配置
   - 加载到内存，构建路径匹配树

3. **FUSE 层集成**
   - 在路径解析时检查原生挂载
   - 实现透传逻辑
   - 处理权限检查

4. **管理 API**
   - 实现 CRUD 接口
   - 添加配置热重载

5. **审计日志**
   - 记录原生挂载操作
   - 区分原生 vs Tarbox 操作

6. **测试**
   - 单元测试路径匹配
   - 集成测试透传操作
   - 测试权限控制

## 示例配置

### Python 开发环境

```toml
# 系统 Python
[[native_mounts]]
path = "/usr/bin/python3"
source = "/usr/bin/python3"
mode = "ro"
shared = true

# 虚拟环境（租户独立）
[[native_mounts]]
path = "/.venv"
source = "/var/tarbox/venvs/{tenant_id}"
mode = "rw"
shared = false
```

### Node.js 开发环境

```toml
# Node 运行时
[[native_mounts]]
path = "/usr/bin/node"
source = "/usr/bin/node"
mode = "ro"
shared = true

# 依赖包（租户独立）
[[native_mounts]]
path = "/node_modules"
source = "/var/tarbox/node_modules/{tenant_id}"
mode = "rw"
shared = false
```

### AI 模型共享

```toml
# 共享的预训练模型
[[native_mounts]]
path = "/models"
source = "/mnt/shared/models"
mode = "ro"
shared = true

# 租户训练输出
[[native_mounts]]
path = "/outputs"
source = "/var/tarbox/outputs/{tenant_id}"
mode = "rw"
shared = false
```

## 限制和约束

1. **不支持分层**：原生挂载的内容不参与 Tarbox 的层系统
2. **不支持跨节点**：原生路径必须在本地存在
3. **审计有限**：只能记录操作类型，无法记录详细内容
4. **不支持去重**：原生文件系统的内容不参与内容寻址
5. **路径限制**：挂载路径不能嵌套（避免歧义）

## 未来扩展

- 支持网络文件系统（NFS, Ceph）作为 source
- 支持按需挂载/卸载
- 支持挂载选项（noexec, nosuid, 等）
- 支持符号链接处理策略
- 支持热更新挂载配置

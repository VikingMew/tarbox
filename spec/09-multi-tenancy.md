# 多租户设计

## 概述

Tarbox 支持多个 Agent 共享同一个 PostgreSQL 后端，同时提供完整的数据隔离和资源隔离。每个租户有独立的文件系统命名空间、层历史、审计日志和配额限制。

## 设计目标

### 功能目标

- **完全隔离**：租户之间数据完全隔离，互不可见
- **资源隔离**：独立的配额、性能限制
- **独立管理**：每个租户独立的层管理、审计
- **共享基础设施**：多租户共享 PostgreSQL、缓存、连接池

### 安全目标

- **数据隔离**：租户 A 无法访问租户 B 的任何数据
- **操作隔离**：一个租户的操作不影响其他租户
- **审计隔离**：审计日志按租户隔离
- **故障隔离**：一个租户的问题不影响其他租户

## 租户模型

### 租户标识

**命名规则**：
- 格式：`<namespace>-<name>`
- 示例：`ai-agents-agent001`、`project-alpha-worker1`
- 长度：最多 253 字符（符合 Kubernetes 限制）
- 字符：小写字母、数字、连字符

**租户 ID**：
- 内部使用 UUID 作为唯一标识
- 与租户名称一一对应
- 不可变

### 租户层次

```
组织级别：
- 组织（Organization）
  └─ 项目（Project）
     └─ 租户（Tenant）

示例：
- organization: acme-corp
  - project: ml-training
    - tenant: agent-001
    - tenant: agent-002
  - project: data-processing
    - tenant: worker-001

用途：
- 组织：计费、全局配额
- 项目：资源分组、权限管理
- 租户：实际的文件系统实例
```

**简化模型（初期）**：
- 只有租户级别
- 未来扩展到项目和组织

## 数据隔离

### 数据库层隔离

**方案：租户 ID 字段**

每个数据表都包含 `tenant_id` 字段：

```sql
-- inodes 表
CREATE TABLE inodes (
    inode_id BIGSERIAL,
    tenant_id UUID NOT NULL,
    parent_id BIGINT,
    name VARCHAR(255) NOT NULL,
    -- ... 其他字段
    
    PRIMARY KEY (tenant_id, inode_id),
    FOREIGN KEY (tenant_id, parent_id) REFERENCES inodes(tenant_id, inode_id)
);

-- 索引包含 tenant_id
CREATE INDEX idx_inodes_tenant_parent ON inodes(tenant_id, parent_id);
CREATE INDEX idx_inodes_tenant_name ON inodes(tenant_id, parent_id, name);

-- 所有查询必须带 tenant_id
SELECT * FROM inodes WHERE tenant_id = :tenant_id AND inode_id = :inode_id;
```

**关键点**：
- 所有表都有 `tenant_id` 列
- 所有主键和外键都包含 `tenant_id`
- 所有索引都以 `tenant_id` 开头
- 所有查询都必须指定 `tenant_id`

### 租户元数据表

```sql
CREATE TABLE tenants (
    tenant_id UUID PRIMARY KEY,
    tenant_name VARCHAR(253) NOT NULL UNIQUE,
    
    -- 组织信息
    organization VARCHAR(253),
    project VARCHAR(253),
    
    -- 根 inode（每个租户独立的根目录）
    root_inode_id BIGINT NOT NULL,
    
    -- 配额
    quota_bytes BIGINT,           -- 空间配额
    quota_inodes BIGINT,          -- 文件数配额
    quota_layers INTEGER,         -- 层数配额
    
    -- 使用统计
    used_bytes BIGINT NOT NULL DEFAULT 0,
    used_inodes BIGINT NOT NULL DEFAULT 0,
    used_layers INTEGER NOT NULL DEFAULT 0,
    
    -- 状态
    status VARCHAR(20) NOT NULL CHECK (status IN ('active', 'suspended', 'deleted')),
    
    -- 时间戳
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP,
    
    -- 配置
    config JSONB
);

-- 索引
CREATE INDEX idx_tenants_name ON tenants(tenant_name) WHERE status = 'active';
CREATE INDEX idx_tenants_org_project ON tenants(organization, project);
```

### 初始化租户

**创建流程**：
1. 生成租户 UUID
2. 在 tenants 表创建记录
3. 创建租户的根 inode（inode_id=1 for this tenant）
4. 创建租户的基础层（base layer）
5. 初始化配额计数器

**租户根目录**：
- 每个租户有独立的 inode 编号空间
- 从 1 开始（租户的根目录）
- 不同租户的 inode_id 可以重复，通过 tenant_id 区分

## FUSE 层隔离

### 挂载点设计

**每租户独立挂载**：
```bash
# 租户 A 的挂载
tarbox mount \
  --mount-point /mnt/agent-001 \
  --tenant ai-agents-agent001

# 租户 B 的挂载
tarbox mount \
  --mount-point /mnt/agent-002 \
  --tenant ai-agents-agent002
```

**特点**：
- 每个挂载点绑定一个租户
- FUSE 进程启动时指定租户 ID
- 所有文件操作自动带上租户上下文

### 租户上下文传递

**FUSE 操作流程**：
```
1. FUSE 接收到文件操作请求
2. 从挂载配置获取 tenant_id
3. 所有数据库查询自动注入 tenant_id
4. 返回结果仅包含该租户的数据
```

**实现要点**：
- 租户 ID 在 FUSE 初始化时固定
- 存储在全局上下文或线程本地存储
- 每个数据库查询自动添加 WHERE tenant_id = ?

### 路径隔离

**虚拟路径**：
- 每个租户看到的都是 `/`（根目录）
- 实际上是该租户的根 inode

**示例**：
```
租户 A 视角：
/data/file.txt -> (tenant_A, inode_12345)

租户 B 视角：
/data/file.txt -> (tenant_B, inode_12345)

两个租户的 /data/file.txt 是完全不同的文件
```

## Kubernetes CSI 隔离

### PVC 到租户映射

**自动租户创建**：
```yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: agent-storage
  namespace: ai-agents
spec:
  storageClassName: tarbox
  resources:
    requests:
      storage: 10Gi
---
# CSI 驱动自动创建租户：ai-agents-agent-storage
```

**租户命名规则**：
- 格式：`<namespace>-<pvc-name>`
- 确保跨命名空间唯一
- 与 Kubernetes 资源对应

### CSI 操作隔离

**CreateVolume**：
1. 提取 namespace 和 PVC name
2. 生成租户名称：`<namespace>-<pvc-name>`
3. 创建租户记录
4. 返回 volume_id（实际上是 tenant_id）

**NodePublishVolume**：
1. 从 volume_id 获取 tenant_id
2. 挂载时指定 tenant_id
3. 该 Pod 只能访问该租户的数据

### 跨命名空间隔离

**Kubernetes 原生隔离**：
- PVC 只能在同一 namespace 内访问
- 结合 RBAC，实现命名空间级隔离

**Tarbox 额外保证**：
- 即使绕过 Kubernetes 权限
- 数据库层仍然完全隔离
- 租户之间无法互相访问

## 层管理隔离

### 独立的层历史

每个租户有完全独立的层历史：

```
租户 A：
base -> checkpoint-1 -> checkpoint-2 [current]

租户 B：
base -> layer-v1 -> layer-v2 -> layer-v3 [current]

两者完全独立，互不影响
```

### 层操作范围

**Hook 文件作用域**：
```bash
# 租户 A 的挂载点
$ cat /.tarbox/layers/current
# 只返回租户 A 的当前层

# 租户 B 的挂载点
$ cat /.tarbox/layers/current  
# 只返回租户 B 的当前层
```

### 数据库查询隔离

```sql
-- 查询租户 A 的层
SELECT * FROM layers WHERE tenant_id = 'tenant-A-uuid';

-- 查询租户 B 的层
SELECT * FROM layers WHERE tenant_id = 'tenant-B-uuid';

-- 不会互相看到对方的数据
```

## 审计隔离

### 独立的审计日志

**数据隔离**：
```sql
CREATE TABLE audit_logs (
    log_id BIGSERIAL,
    tenant_id UUID NOT NULL,
    -- ... 其他字段
    
    PRIMARY KEY (tenant_id, log_id)
) PARTITION BY HASH (tenant_id);

-- 按租户分区
CREATE TABLE audit_logs_0 PARTITION OF audit_logs
    FOR VALUES WITH (MODULUS 4, REMAINDER 0);
CREATE TABLE audit_logs_1 PARTITION OF audit_logs
    FOR VALUES WITH (MODULUS 4, REMAINDER 1);
-- ...
```

**查询限制**：
- 租户只能查询自己的审计日志
- 管理员可以查询所有租户（需要权限）

### 审计查询 Hook

```bash
# 租户 A 查询审计
$ cat /.tarbox/audit/recent
# 只返回租户 A 的审计记录

# 租户 B 查询审计
$ cat /.tarbox/audit/recent
# 只返回租户 B 的审计记录
```

## 资源配额

### 配额类型

**存储配额**：
- quota_bytes：最大存储空间
- 包括所有层的总大小
- 达到配额时写入返回 ENOSPC

**Inode 配额**：
- quota_inodes：最大文件数
- 防止创建过多小文件
- 达到配额时创建失败

**层数配额**：
- quota_layers：最大层数
- 防止创建过多检查点
- 达到配额时创建层失败

### 配额检查

**写入时检查**：
```
1. 计算写入后的大小
2. 检查：used_bytes + new_bytes <= quota_bytes
3. 如果超出，拒绝写入
4. 如果通过，执行写入并更新计数器
```

**原子性更新**：
```sql
-- 使用数据库事务保证原子性
UPDATE tenants 
SET used_bytes = used_bytes + :new_bytes
WHERE tenant_id = :tenant_id 
  AND used_bytes + :new_bytes <= quota_bytes;

-- 如果 affected_rows = 0，表示超配额
```

### 配额监控

**统计信息**：
```bash
# 查看租户配额使用
$ cat /.tarbox/stats/usage
{
  "tenant_id": "uuid",
  "tenant_name": "ai-agents-agent001",
  "quota": {
    "bytes": 10737418240,
    "inodes": 1000000,
    "layers": 100
  },
  "used": {
    "bytes": 1073741824,
    "inodes": 12345,
    "layers": 5
  },
  "usage_percent": {
    "bytes": 10.0,
    "inodes": 1.2,
    "layers": 5.0
  }
}
```

## 性能隔离

### 连接池隔离

**设计**：
- 共享连接池，但限制每个租户的连接数
- 防止单个租户占用所有连接

**配置**：
```toml
[database.pool]
total_connections = 50
per_tenant_max = 10      # 每个租户最多 10 个连接
```

### IOPS 限制

**令牌桶算法**：
- 每个租户独立的 IOPS 限制
- 配置：max_iops_per_tenant
- 超出限制时操作排队或返回 EAGAIN

### 带宽限制

**流量控制**：
- 限制每个租户的读写带宽
- 配置：max_bandwidth_per_tenant
- 使用滑动窗口计算

## 缓存隔离

### 租户缓存分区

**设计**：
- 缓存键包含 tenant_id
- 格式：`<tenant_id>:<key>`
- 防止缓存污染

**示例**：
```
租户 A 的 inode 缓存键：
tenant-A-uuid:inode:12345

租户 B 的 inode 缓存键：
tenant-B-uuid:inode:12345

虽然 inode_id 相同，但缓存键不同
```

### 缓存配额

**每租户缓存限制**：
- 防止单个租户占用全部缓存
- LRU 淘汰时考虑租户公平性
- 可配置的缓存分配策略

**配置**：
```toml
[cache.multi_tenancy]
enabled = true
per_tenant_max_size = "500MB"
eviction_policy = "fair_lru"  # 公平的 LRU
```

## 管理接口

### 创建租户

**API**：
```bash
POST /api/v1/tenants
{
  "name": "ai-agents-agent001",
  "organization": "acme-corp",
  "project": "ml-training",
  "quota": {
    "bytes": 10737418240,
    "inodes": 1000000,
    "layers": 100
  }
}
```

**CLI**：
```bash
tarbox tenant create ai-agents-agent001 \
  --quota 10GB \
  --max-inodes 1000000 \
  --max-layers 100
```

### 列出租户

**API**：
```bash
GET /api/v1/tenants?organization=acme-corp&project=ml-training
```

**CLI**：
```bash
tarbox tenant list --org acme-corp --project ml-training
```

### 删除租户

**API**：
```bash
DELETE /api/v1/tenants/ai-agents-agent001
```

**删除流程**：
1. 标记租户状态为 'deleted'
2. 拒绝新的访问
3. 等待所有活跃会话结束
4. 异步清理数据
5. 最终删除租户记录

### 修改配额

**API**：
```bash
PATCH /api/v1/tenants/ai-agents-agent001/quota
{
  "bytes": 21474836480  # 增加到 20GB
}
```

**CLI**：
```bash
tarbox tenant update-quota ai-agents-agent001 --size 20GB
```

## 监控和告警

### 租户级指标

**Prometheus 指标**：
```
# 每租户的存储使用
tarbox_tenant_storage_bytes{tenant="ai-agents-agent001"}

# 每租户的 IOPS
tarbox_tenant_iops{tenant="ai-agents-agent001",operation="read|write"}

# 每租户的配额使用率
tarbox_tenant_quota_usage_ratio{tenant="ai-agents-agent001",resource="bytes|inodes|layers"}

# 每租户的活跃挂载数
tarbox_tenant_active_mounts{tenant="ai-agents-agent001"}
```

### 告警规则

```yaml
groups:
- name: tarbox-tenant
  rules:
  - alert: TenantQuotaAlmostFull
    expr: tarbox_tenant_quota_usage_ratio{resource="bytes"} > 0.9
    for: 5m
    annotations:
      summary: "Tenant {{ $labels.tenant }} quota almost full"
      
  - alert: TenantHighIOPS
    expr: rate(tarbox_tenant_iops[5m]) > 10000
    for: 5m
    annotations:
      summary: "Tenant {{ $labels.tenant }} has high IOPS"
```

## 安全考虑

### 租户身份验证

**Kubernetes 环境**：
- 通过 ServiceAccount 自动验证
- PVC 绑定到特定租户
- Pod 只能访问自己的 PVC

**API 访问**：
- Bearer Token 绑定租户
- API Key 关联租户
- mTLS 证书包含租户信息

### 防止跨租户访问

**多层防护**：
1. **FUSE 层**：挂载时绑定租户，无法切换
2. **数据库层**：所有查询必须包含 tenant_id
3. **缓存层**：缓存键包含 tenant_id
4. **审计层**：记录所有访问尝试

**SQL 注入防护**：
- 使用参数化查询
- tenant_id 使用 UUID，不接受用户输入的字符串
- 严格的输入验证

### 资源耗尽攻击

**防护措施**：
- 配额限制：防止单个租户占用过多资源
- 速率限制：限制操作频率
- 连接限制：限制数据库连接数
- 超时机制：防止长时间占用资源

## 测试策略

### 隔离性测试

**测试项**：
- 租户 A 无法读取租户 B 的文件
- 租户 A 无法修改租户 B 的层
- 租户 A 无法查询租户 B 的审计日志
- 配额独立计算和强制

### 性能测试

**测试项**：
- 多租户并发性能
- 单租户故障不影响其他租户
- 缓存公平性
- 连接池公平性

### 压力测试

**测试项**：
- 大量租户同时操作
- 租户快速创建和删除
- 配额边界测试
- 资源耗尽场景

## 最佳实践

### 租户命名

**建议**：
- 使用有意义的名称：`<team>-<project>-<instance>`
- 避免敏感信息在名称中
- 使用小写和连字符

### 配额设置

**建议**：
- 根据实际需求设置配额
- 预留 10-20% 缓冲
- 定期审查和调整
- 监控配额使用趋势

### 租户生命周期

**建议**：
- 测试租户使用短生命周期
- 生产租户定期备份
- 删除前导出重要数据
- 使用标签标记租户用途

## 未来增强

### 租户组

**设计思路**：
- 支持租户分组
- 组级别的配额和权限
- 租户间数据共享（受控）

### 跨租户克隆

**设计思路**：
- 从一个租户克隆层到另一个租户
- 用于模板和初始化
- 需要明确授权

### 租户迁移

**设计思路**：
- 在线迁移租户到另一个数据库
- 跨区域复制
- 零停机迁移

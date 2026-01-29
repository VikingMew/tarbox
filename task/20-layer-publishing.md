# Task 20: Layer 发布机制 (Layer Publishing)

## 概述

实现 Layer 发布功能，允许 Tenant 将自己的 Layer 发布给其他 Tenant 只读访问。支持发布特定 snapshot 或 working layer（实时跟随）。

## 依赖

- ✅ Task 19: 挂载条目基础设施
- ✅ Task 08: 分层文件系统

## 交付物

### 1. 数据库 Schema

**文件**: `migrations/YYYYMMDDHHMMSS_create_published_mounts.sql`

```sql
-- 发布配置表
CREATE TABLE published_mounts (
    publish_id UUID PRIMARY KEY,
    mount_entry_id UUID NOT NULL REFERENCES mount_entries(mount_entry_id) ON DELETE CASCADE,
    tenant_id UUID NOT NULL REFERENCES tenants(tenant_id),
    
    -- 发布名称（全局唯一）
    publish_name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    
    -- 发布目标：'layer' 或 'working_layer'
    target_type VARCHAR(20) NOT NULL,
    layer_id UUID,  -- NULL if working_layer
    
    -- 访问控制
    scope VARCHAR(20) NOT NULL DEFAULT 'public',
    allowed_tenants UUID[],
    
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    
    CONSTRAINT valid_target CHECK (
        (target_type = 'layer' AND layer_id IS NOT NULL) OR
        (target_type = 'working_layer' AND layer_id IS NULL)
    )
);

CREATE INDEX idx_published_mounts_name ON published_mounts(publish_name);
CREATE INDEX idx_published_mounts_tenant ON published_mounts(tenant_id);
CREATE INDEX idx_published_mounts_mount ON published_mounts(mount_entry_id);
```

### 2. Rust 数据结构

**文件**: `src/storage/models/published_mount.rs`

```rust
/// 发布目标
pub enum PublishTarget {
    /// 发布特定的 snapshot（内容固定）
    Layer(Uuid),
    
    /// 发布当前 working layer（实时跟随）
    WorkingLayer,
}

/// 发布范围
pub enum PublishScope {
    /// 所有 tenant 可见
    Public,
    
    /// 仅指定 tenant 可见
    AllowList(Vec<Uuid>),
}

/// 发布记录
pub struct PublishedMount {
    pub publish_id: Uuid,
    pub mount_entry_id: Uuid,
    pub tenant_id: Uuid,
    pub publish_name: String,
    pub description: Option<String>,
    pub target: PublishTarget,
    pub scope: PublishScope,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 发布输入
pub struct PublishMountInput {
    pub mount_entry_id: Uuid,
    pub publish_name: String,
    pub description: Option<String>,
    pub target: PublishTarget,
    pub scope: PublishScope,
}

/// 更新发布输入
pub struct UpdatePublishInput {
    pub description: Option<String>,
    pub scope: Option<PublishScope>,
}

/// 发布过滤器
pub struct PublishedMountFilter {
    pub scope: Option<String>,          // "public" 或 "all"
    pub owner_tenant_id: Option<Uuid>,  // 过滤特定 owner
}
```

### 3. 存储层 Repository

**文件**: `src/storage/published_mount.rs`

```rust
#[async_trait]
pub trait PublishedMountRepository {
    /// 发布挂载点
    async fn publish_mount(&self, input: PublishMountInput) -> Result<PublishedMount>;
    
    /// 取消发布
    async fn unpublish_mount(&self, mount_entry_id: Uuid) -> Result<()>;
    
    /// 通过发布名称获取
    async fn get_published_by_name(&self, publish_name: &str) -> Result<Option<PublishedMount>>;
    
    /// 获取挂载点的发布信息
    async fn get_publish_info(&self, mount_entry_id: Uuid) -> Result<Option<PublishedMount>>;
    
    /// 列出已发布的挂载（全局）
    async fn list_published_mounts(
        &self,
        filter: PublishedMountFilter,
    ) -> Result<Vec<PublishedMount>>;
    
    /// 列出 Tenant 已发布的挂载
    async fn list_tenant_published_mounts(
        &self,
        tenant_id: Uuid,
    ) -> Result<Vec<PublishedMount>>;
    
    /// 更新发布信息
    async fn update_publish(
        &self,
        publish_id: Uuid,
        input: UpdatePublishInput,
    ) -> Result<PublishedMount>;
    
    /// 检查访问权限
    async fn check_access(
        &self,
        publish_name: &str,
        accessor_tenant_id: Uuid,
    ) -> Result<bool>;
    
    /// 添加授权租户
    async fn add_allowed_tenant(
        &self,
        publish_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<()>;
    
    /// 移除授权租户
    async fn remove_allowed_tenant(
        &self,
        publish_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<()>;
}
```

### 4. 发布管理服务

**文件**: `src/composition/publisher.rs`

```rust
pub struct LayerPublisher {
    pub published_mount_repo: Arc<dyn PublishedMountRepository>,
    pub mount_entry_repo: Arc<dyn MountEntryRepository>,
}

impl LayerPublisher {
    /// 发布挂载点
    /// 
    /// 验证：
    /// - 挂载点必须属于调用者
    /// - 挂载点必须是 WorkingLayer 类型
    /// - 发布名称必须全局唯一
    pub async fn publish(
        &self,
        tenant_id: Uuid,
        mount_name: &str,
        input: PublishMountInput,
    ) -> Result<PublishedMount>;
    
    /// 取消发布
    pub async fn unpublish(
        &self,
        tenant_id: Uuid,
        mount_name: &str,
    ) -> Result<()>;
    
    /// 解析已发布的挂载
    /// 
    /// 返回实际的 layer_id（对于 working_layer 类型，返回当前 working layer）
    pub async fn resolve_published(
        &self,
        publish_name: &str,
        accessor_tenant_id: Uuid,
    ) -> Result<ResolvedPublished>;
}

pub struct ResolvedPublished {
    pub mount_entry_id: Uuid,
    pub owner_tenant_id: Uuid,
    pub layer_id: Uuid,  // 实际的 layer ID
    pub is_working_layer: bool,
}
```

### 5. 访问控制集成

**更新**: `src/composition/resolver.rs`

在路径解析时验证发布访问权限：

```rust
impl PathResolver {
    async fn resolve_path(&self, tenant_id: Uuid, path: &Path) -> Result<ResolvedPath> {
        let mount = self.find_mount(tenant_id, path).await?;
        
        match &mount.source {
            MountSource::Published { publish_name, subpath } => {
                // 检查访问权限
                let resolved = self.publisher
                    .resolve_published(publish_name, tenant_id)
                    .await?;
                
                // 返回解析结果
                Ok(ResolvedPath {
                    mount_entry: mount,
                    relative_path: compute_relative(path, &mount.virtual_path, subpath),
                    source: ResolvedSource::Layer {
                        tenant_id: resolved.owner_tenant_id,
                        layer_id: resolved.layer_id,
                        path: /* ... */,
                    },
                })
            }
            // ... 其他 source 类型
        }
    }
}
```

## 功能点

| 功能 | 说明 | 验收标准 |
|------|------|----------|
| 发布 Layer | 发布特定 snapshot | 其他 tenant 可以只读访问 |
| 发布 Working Layer | 发布实时内容 | 写入后其他 tenant 立即可见 |
| 取消发布 | 移除发布 | 使用该发布的挂载无法再访问 |
| Public 范围 | 所有 tenant 可见 | 任何 tenant 都能挂载 |
| AllowList 范围 | 仅授权 tenant 可见 | 未授权 tenant 访问被拒绝 |
| 授权管理 | 添加/移除授权 tenant | 动态调整访问列表 |
| 访问检查 | 挂载时验证权限 | 无权限返回 AccessDenied |

## 测试要求

### 单元测试 (target: 25+)

1. **PublishTarget 序列化** (3 tests)
2. **PublishScope 序列化** (3 tests)
3. **访问权限检查逻辑** (10 tests)
   - Public scope 允许所有
   - AllowList scope 只允许列表内
   - AllowList 空列表拒绝所有
   - Owner 始终可访问自己的
4. **发布名称验证** (5 tests)
   - 有效名称
   - 无效字符
   - 长度限制
5. **WorkingLayer 解析** (4 tests)
   - 获取当前 working layer ID
   - working layer 变化后获取新 ID

### 集成测试 (target: 20+)

1. **发布流程** (8 tests)
   - 发布 snapshot
   - 发布 working layer
   - 重复发布名称失败
   - 非 owner 发布失败
   - 非 WorkingLayer 挂载发布失败

2. **访问控制** (8 tests)
   - Public 访问成功
   - AllowList 授权 tenant 成功
   - AllowList 未授权 tenant 失败
   - 添加授权后访问成功
   - 移除授权后访问失败

3. **取消发布** (4 tests)
   - 正常取消
   - 取消后访问失败
   - 非 owner 取消失败

## 文件清单

```
src/
├── storage/
│   ├── models/
│   │   └── published_mount.rs  # 数据模型
│   └── published_mount.rs      # Repository 实现
├── composition/
│   ├── publisher.rs            # 发布管理服务
│   └── resolver.rs             # 更新：集成访问控制
migrations/
└── YYYYMMDDHHMMSS_create_published_mounts.sql
```

## 不包含（后续 Task）

- 挂载点级别的 layer 链创建 (Task 21)
- HTTP API (Task 22)
- CLI 命令 (Task 23)

## 完成标准

- [ ] 数据库 migration 文件创建并可执行
- [ ] PublishedMount 相关数据结构实现
- [ ] PublishedMountRepository trait 和实现
- [ ] LayerPublisher 服务实现
- [ ] PathResolver 集成访问控制
- [ ] 25+ 单元测试通过
- [ ] 20+ 集成测试通过
- [ ] cargo fmt 通过
- [ ] cargo clippy 通过
- [ ] 测试覆盖率 > 80%

## 预计工作量

2-3 天

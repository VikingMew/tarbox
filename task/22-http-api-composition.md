# Task 22: HTTP API - 文件系统组合 (HTTP API for Composition)

## 概述

实现 spec/06 中定义的文件系统组合相关 HTTP API，包括挂载管理、snapshot、发布等功能。使用 Axum 框架，所有接口返回 JSON。

## 依赖

- ✅ Task 19: 挂载条目基础设施
- ✅ Task 20: Layer 发布机制
- ✅ Task 21: 挂载点级别 Layer 链
- ✅ Task 14: REST API 基础（如已完成）

## 交付物

### 1. API 路由结构

**文件**: `src/api/routes/composition.rs`

```rust
pub fn composition_routes() -> Router {
    Router::new()
        // 挂载管理
        .route("/tenants/:tenant_id/mounts", get(list_mounts))
        .route("/tenants/:tenant_id/mounts", put(set_mounts))
        .route("/tenants/:tenant_id/mounts", post(add_mount))
        .route("/tenants/:tenant_id/mounts/import", post(import_mounts))
        .route("/tenants/:tenant_id/mounts/export", get(export_mounts))
        .route("/tenants/:tenant_id/mounts/:mount_entry_id", put(update_mount))
        .route("/tenants/:tenant_id/mounts/:mount_entry_id", delete(delete_mount))
        .route("/tenants/:tenant_id/mounts/:mount_entry_id/enable", post(enable_mount))
        .route("/tenants/:tenant_id/mounts/:mount_entry_id/disable", post(disable_mount))
        
        // Snapshot（使用 mount_name）
        .route("/tenants/:tenant_id/mounts/:mount_name/snapshot", post(snapshot_mount))
        .route("/tenants/:tenant_id/snapshot", post(snapshot_multiple))
        
        // 发布（使用 mount_name）
        .route("/tenants/:tenant_id/mounts/:mount_name/publish", post(publish_mount))
        .route("/tenants/:tenant_id/mounts/:mount_name/publish", delete(unpublish_mount))
        .route("/tenants/:tenant_id/published-mounts", get(list_tenant_published))
        
        // 全局发布列表
        .route("/published-layers", get(list_published))
        .route("/published-layers/:publish_name", get(get_published_info))
        
        // 发布管理
        .route("/tenants/:tenant_id/layers/:layer_id/publish", put(update_publish))
        
        // 路径解析（调试）
        .route("/tenants/:tenant_id/resolve", get(resolve_path))
}
```

### 2. 请求/响应数据结构

**文件**: `src/api/dto/composition.rs`

```rust
// ============ 挂载管理 ============

#[derive(Deserialize)]
pub struct SetMountsRequest {
    pub mounts: Vec<MountEntryDto>,
}

#[derive(Deserialize, Serialize)]
pub struct MountEntryDto {
    pub name: String,
    pub virtual_path: String,
    pub is_file: Option<bool>,
    pub source_type: String,
    
    // Host source
    pub host_path: Option<String>,
    
    // Layer source
    pub source_tenant_id: Option<Uuid>,
    pub source_layer_id: Option<Uuid>,
    pub source_subpath: Option<String>,
    
    // Published source
    pub published_layer_name: Option<String>,
    
    pub mode: String,  // "ro", "rw", "cow"
    pub enabled: Option<bool>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Serialize)]
pub struct MountEntryResponse {
    pub mount_entry_id: Uuid,
    pub name: String,
    pub virtual_path: String,
    pub is_file: bool,
    pub source_type: String,
    pub host_path: Option<String>,
    pub source_tenant_id: Option<Uuid>,
    pub source_layer_id: Option<Uuid>,
    pub source_subpath: Option<String>,
    pub mode: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct ListMountsResponse {
    pub tenant_id: Uuid,
    pub mounts: Vec<MountEntryResponse>,
}

// ============ 导入/导出 ============

#[derive(Deserialize)]
pub struct ImportMountsRequest {
    pub mounts: Vec<SimplifiedMountDto>,
}

#[derive(Deserialize, Serialize)]
pub struct SimplifiedMountDto {
    pub name: String,
    pub path: String,
    pub source: String,  // "working_layer", "host:/usr", "layer:tenant:layer", "published:name"
    pub mode: String,
    #[serde(default)]
    pub file: bool,
    pub subpath: Option<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool { true }

#[derive(Serialize)]
pub struct ExportMountsResponse {
    pub tenant_id: Uuid,
    pub tenant_name: String,
    pub exported_at: DateTime<Utc>,
    pub mounts: Vec<SimplifiedMountDto>,
}

// ============ Snapshot ============

#[derive(Deserialize)]
pub struct SnapshotMountRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Deserialize)]
pub struct SnapshotMultipleRequest {
    pub mounts: Vec<String>,  // mount names
    pub name: String,
    #[serde(default)]
    pub skip_unchanged: bool,
}

#[derive(Serialize)]
pub struct SnapshotResponse {
    pub layer_id: Uuid,
    pub mount_entry_id: Uuid,
    pub mount_name: String,
    pub name: String,
    pub parent_layer_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct SnapshotMultipleResponse {
    pub snapshots: Vec<SnapshotResultDto>,
}

#[derive(Serialize)]
pub struct SnapshotResultDto {
    pub mount_name: String,
    pub layer_id: Option<Uuid>,
    pub skipped: bool,
    pub reason: Option<String>,
}

// ============ 发布 ============

#[derive(Deserialize)]
pub struct PublishMountRequest {
    pub publish_name: String,
    pub description: Option<String>,
    pub target: String,  // "layer" or "working_layer"
    pub layer_id: Option<Uuid>,  // required if target == "layer"
    pub scope: String,  // "public" or "allow_list"
    pub allowed_tenants: Option<Vec<Uuid>>,
}

#[derive(Serialize)]
pub struct PublishMountResponse {
    pub publish_id: Uuid,
    pub mount_entry_id: Uuid,
    pub mount_name: String,
    pub tenant_id: Uuid,
    pub target_type: String,
    pub layer_id: Option<Uuid>,
    pub publish_name: String,
    pub description: Option<String>,
    pub scope: String,
    pub allowed_tenants: Option<Vec<Uuid>>,
    pub published_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct ListPublishedResponse {
    pub published_layers: Vec<PublishedLayerDto>,
}

#[derive(Serialize)]
pub struct PublishedLayerDto {
    pub publish_id: Uuid,
    pub mount_entry_id: Uuid,
    pub tenant_id: Uuid,
    pub publish_name: String,
    pub description: Option<String>,
    pub target_type: String,
    pub layer_id: Option<Uuid>,
    pub scope: String,
    pub published_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct TenantPublishedResponse {
    pub tenant_id: Uuid,
    pub published_mounts: Vec<PublishedLayerDto>,
}

// ============ 路径解析 ============

#[derive(Serialize)]
pub struct ResolvePathResponse {
    pub path: String,
    pub resolved: ResolvedPathDto,
}

#[derive(Serialize)]
pub struct ResolvedPathDto {
    pub mount_entry_id: Uuid,
    pub mount_name: String,
    pub virtual_path: String,
    pub source_type: String,
    pub source_tenant_id: Option<Uuid>,
    pub source_layer_id: Option<Uuid>,
    pub relative_path: String,
    pub mode: String,
}
```

### 3. Handler 实现

**文件**: `src/api/handlers/composition.rs`

```rust
/// 列出挂载配置
pub async fn list_mounts(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
) -> Result<Json<ListMountsResponse>, ApiError> {
    let mounts = state.mount_entry_repo
        .list_mount_entries(tenant_id)
        .await?;
    
    Ok(Json(ListMountsResponse {
        tenant_id,
        mounts: mounts.into_iter().map(Into::into).collect(),
    }))
}

/// 批量设置挂载
pub async fn set_mounts(
    State(state): State<AppState>,
    Path(tenant_id): Path<Uuid>,
    Json(request): Json<SetMountsRequest>,
) -> Result<Json<ListMountsResponse>, ApiError> {
    // 验证无冲突
    // 转换 DTO 到 CreateMountEntry
    // 调用 set_mount_entries
    // ...
}

/// Snapshot 单个挂载点
pub async fn snapshot_mount(
    State(state): State<AppState>,
    Path((tenant_id, mount_name)): Path<(Uuid, String)>,
    Json(request): Json<SnapshotMountRequest>,
) -> Result<Json<SnapshotResponse>, ApiError> {
    let layer = state.layer_chain_manager
        .snapshot(tenant_id, &mount_name, &request.name, request.description.as_deref())
        .await?;
    
    Ok(Json(SnapshotResponse {
        layer_id: layer.layer_id,
        // ...
    }))
}

/// 发布挂载点
pub async fn publish_mount(
    State(state): State<AppState>,
    Path((tenant_id, mount_name)): Path<(Uuid, String)>,
    Json(request): Json<PublishMountRequest>,
) -> Result<Json<PublishMountResponse>, ApiError> {
    let input = PublishMountInput {
        mount_entry_id: state.mount_entry_repo
            .get_mount_entry_by_name(tenant_id, &mount_name)
            .await?
            .ok_or(ApiError::NotFound)?
            .mount_entry_id,
        publish_name: request.publish_name,
        description: request.description,
        target: match request.target.as_str() {
            "layer" => PublishTarget::Layer(request.layer_id.ok_or(ApiError::BadRequest)?),
            "working_layer" => PublishTarget::WorkingLayer,
            _ => return Err(ApiError::BadRequest),
        },
        scope: match request.scope.as_str() {
            "public" => PublishScope::Public,
            "allow_list" => PublishScope::AllowList(request.allowed_tenants.unwrap_or_default()),
            _ => return Err(ApiError::BadRequest),
        },
    };
    
    let published = state.publisher.publish(tenant_id, &mount_name, input).await?;
    
    Ok(Json(published.into()))
}

// ... 其他 handler
```

### 4. 错误处理

**文件**: `src/api/error.rs` (更新)

```rust
pub enum ApiError {
    // 现有错误...
    
    /// 挂载路径冲突
    MountPathConflict(String),
    
    /// 发布名称已存在
    PublishNameExists(String),
    
    /// 访问被拒绝
    AccessDenied(String),
    
    /// 无效的挂载源
    InvalidMountSource(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::MountPathConflict(msg) => (StatusCode::CONFLICT, msg),
            ApiError::PublishNameExists(msg) => (StatusCode::CONFLICT, msg),
            ApiError::AccessDenied(msg) => (StatusCode::FORBIDDEN, msg),
            ApiError::InvalidMountSource(msg) => (StatusCode::BAD_REQUEST, msg),
            // ...
        };
        
        (status, Json(json!({ "error": message }))).into_response()
    }
}
```

## API 端点清单

| 方法 | 路径 | 功能 | 请求体 | 响应 |
|------|------|------|--------|------|
| GET | `/tenants/{id}/mounts` | 列出挂载 | - | ListMountsResponse |
| PUT | `/tenants/{id}/mounts` | 批量设置 | SetMountsRequest | ListMountsResponse |
| POST | `/tenants/{id}/mounts` | 添加单个 | MountEntryDto | MountEntryResponse |
| POST | `/tenants/{id}/mounts/import` | 导入配置 | ImportMountsRequest | ListMountsResponse |
| GET | `/tenants/{id}/mounts/export` | 导出配置 | - | ExportMountsResponse |
| PUT | `/tenants/{id}/mounts/{entry_id}` | 更新挂载 | UpdateMountRequest | MountEntryResponse |
| DELETE | `/tenants/{id}/mounts/{entry_id}` | 删除挂载 | - | - |
| POST | `/tenants/{id}/mounts/{entry_id}/enable` | 启用 | - | MountEntryResponse |
| POST | `/tenants/{id}/mounts/{entry_id}/disable` | 禁用 | - | MountEntryResponse |
| POST | `/tenants/{id}/mounts/{name}/snapshot` | Snapshot | SnapshotMountRequest | SnapshotResponse |
| POST | `/tenants/{id}/snapshot` | 批量 Snapshot | SnapshotMultipleRequest | SnapshotMultipleResponse |
| POST | `/tenants/{id}/mounts/{name}/publish` | 发布 | PublishMountRequest | PublishMountResponse |
| DELETE | `/tenants/{id}/mounts/{name}/publish` | 取消发布 | - | - |
| GET | `/tenants/{id}/published-mounts` | 列出 Tenant 发布 | - | TenantPublishedResponse |
| GET | `/published-layers` | 列出全局发布 | - | ListPublishedResponse |
| GET | `/published-layers/{name}` | 获取发布详情 | - | PublishedLayerDto |
| PUT | `/tenants/{id}/layers/{id}/publish` | 更新发布 | UpdatePublishRequest | PublishMountResponse |
| GET | `/tenants/{id}/resolve` | 解析路径 | query: path | ResolvePathResponse |

## 测试要求

### 单元测试 (target: 15+)

1. **DTO 转换** (8 tests)
   - MountEntryDto <-> MountEntry
   - SimplifiedMountDto source 解析
   - PublishMountRequest 验证

2. **错误处理** (7 tests)
   - 路径冲突返回 409
   - 发布名称重复返回 409
   - 访问拒绝返回 403

### 集成测试 (target: 30+)

1. **挂载管理** (12 tests)
   - list_mounts
   - set_mounts
   - add_mount (成功)
   - add_mount (冲突)
   - import_mounts
   - export_mounts
   - update_mount
   - delete_mount
   - enable/disable

2. **Snapshot** (8 tests)
   - snapshot_mount (成功)
   - snapshot_mount (不存在)
   - snapshot_multiple (全部成功)
   - snapshot_multiple (部分跳过)
   - snapshot_multiple (部分不存在)

3. **发布** (10 tests)
   - publish_mount (layer)
   - publish_mount (working_layer)
   - publish_mount (名称重复)
   - unpublish_mount
   - list_published (public)
   - list_published (filter by owner)
   - list_tenant_published
   - 访问控制 (public 成功)
   - 访问控制 (allow_list 成功/失败)

## 文件清单

```
src/api/
├── routes/
│   ├── mod.rs
│   └── composition.rs          # 路由定义
├── handlers/
│   ├── mod.rs
│   └── composition.rs          # Handler 实现
├── dto/
│   ├── mod.rs
│   └── composition.rs          # 数据传输对象
└── error.rs                    # 更新：新增错误类型
```

## 完成标准

- [ ] 所有 API 端点实现
- [ ] 请求/响应 DTO 实现
- [ ] 错误处理完整
- [ ] 15+ 单元测试通过
- [ ] 30+ 集成测试通过
- [ ] cargo fmt 通过
- [ ] cargo clippy 通过
- [ ] 测试覆盖率 > 80%
- [ ] API 文档更新

## 预计工作量

3-4 天

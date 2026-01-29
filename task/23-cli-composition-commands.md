# Task 23: CLI 组合命令 (CLI Composition Commands)

## 概述

实现 spec/06 中定义的文件系统组合相关 CLI 命令，包括 mount 管理、snapshot、publish 等。CLI 使用 TOML 配置文件，与 HTTP API 的 JSON 格式区分。

## 依赖

- ✅ Task 19: 挂载条目基础设施
- ✅ Task 20: Layer 发布机制
- ✅ Task 21: 挂载点级别 Layer 链
- ✅ Task 22: HTTP API
- ✅ Task 04: CLI 工具 MVP

## 交付物

### 1. 命令结构

**文件**: `src/cli/commands/mount.rs`

```rust
#[derive(Subcommand)]
pub enum MountCommands {
    /// 从配置文件应用挂载
    Apply {
        #[arg(long)]
        tenant: String,
        
        #[arg(long, short)]
        config: PathBuf,
    },
    
    /// 列出当前挂载配置
    List {
        #[arg(long)]
        tenant: String,
        
        #[arg(long)]
        json: bool,
        
        #[arg(long)]
        toml: bool,
    },
    
    /// 导出配置到文件
    Export {
        #[arg(long)]
        tenant: String,
        
        #[arg(long, short)]
        output: PathBuf,
    },
    
    /// 验证配置文件
    Validate {
        #[arg(long, short)]
        config: PathBuf,
    },
    
    /// 清空所有挂载
    Clear {
        #[arg(long)]
        tenant: String,
    },
    
    /// 删除单个挂载
    Remove {
        #[arg(long)]
        tenant: String,
        
        #[arg(long, short)]
        mount: String,
    },
    
    /// 启用挂载
    Enable {
        #[arg(long)]
        tenant: String,
        
        #[arg(long, short)]
        mount: String,
    },
    
    /// 禁用挂载
    Disable {
        #[arg(long)]
        tenant: String,
        
        #[arg(long, short)]
        mount: String,
    },
    
    /// 更新挂载配置
    Update {
        #[arg(long)]
        tenant: String,
        
        #[arg(long, short)]
        mount: String,
        
        #[arg(long)]
        mode: Option<String>,
        
        #[arg(long)]
        enabled: Option<bool>,
    },
    
    /// 解析路径（调试）
    Resolve {
        #[arg(long)]
        tenant: String,
        
        path: String,
    },
}
```

### 2. Snapshot 命令

**文件**: `src/cli/commands/snapshot.rs`

```rust
#[derive(Args)]
pub struct SnapshotCommand {
    #[arg(long)]
    tenant: String,
    
    /// 挂载点名称（可多个）
    #[arg(long, short)]
    mount: Vec<String>,
    
    /// Snapshot 所有 WorkingLayer 挂载点
    #[arg(long)]
    all: bool,
    
    /// Snapshot 名称
    #[arg(long)]
    name: String,
    
    /// 跳过无变化的挂载点
    #[arg(long)]
    skip_unchanged: bool,
}

impl SnapshotCommand {
    pub async fn execute(&self, state: &AppState) -> Result<()> {
        if self.all {
            let results = state.layer_chain_manager
                .snapshot_all(&self.tenant, &self.name, self.skip_unchanged)
                .await?;
            
            for result in results {
                if result.skipped {
                    println!("{}: skipped ({})", result.mount_name, result.reason.unwrap_or_default());
                } else {
                    println!("{}: created layer {}", result.mount_name, result.layer_id.unwrap());
                }
            }
        } else if !self.mount.is_empty() {
            let results = state.layer_chain_manager
                .snapshot_multiple(&self.tenant, &self.mount, &self.name, self.skip_unchanged)
                .await?;
            // ...
        } else {
            return Err(anyhow!("Must specify --mount or --all"));
        }
        
        Ok(())
    }
}
```

### 3. Publish 命令

**文件**: `src/cli/commands/publish.rs`

```rust
#[derive(Subcommand)]
pub enum PublishCommands {
    /// 发布挂载点
    #[command(name = "")]  // 默认子命令
    Publish(PublishArgs),
    
    /// 列出已发布的挂载
    List {
        #[arg(long)]
        tenant: String,
        
        #[arg(long)]
        json: bool,
    },
}

#[derive(Args)]
pub struct PublishArgs {
    #[arg(long)]
    tenant: String,
    
    #[arg(long, short)]
    mount: String,
    
    /// 发布目标: "layer" 或 "working_layer"
    #[arg(long, default_value = "working_layer")]
    target: String,
    
    /// Layer ID 或名称（target=layer 时必需）
    #[arg(long)]
    layer: Option<String>,
    
    /// 发布名称
    #[arg(long)]
    name: String,
    
    /// 描述
    #[arg(long)]
    description: Option<String>,
    
    /// 范围: "public" 或 "allow_list"
    #[arg(long, default_value = "public")]
    scope: String,
    
    /// 授权租户列表（逗号分隔）
    #[arg(long)]
    allow: Option<String>,
}

#[derive(Args)]
pub struct UnpublishArgs {
    #[arg(long)]
    tenant: String,
    
    #[arg(long, short)]
    mount: String,
}
```

### 4. Layer 发布管理命令

**文件**: `src/cli/commands/layer.rs` (更新)

```rust
#[derive(Subcommand)]
pub enum LayerCommands {
    // 现有命令...
    
    /// 列出已发布的 Layer
    ListPublished {
        #[arg(long)]
        scope: Option<String>,  // public, all
        
        #[arg(long)]
        owner: Option<String>,  // tenant-id
        
        #[arg(long)]
        json: bool,
    },
    
    /// 查看已发布 Layer 详情
    PublishInfo {
        name: String,
    },
    
    /// 更新发布信息
    PublishUpdate {
        name: String,
        
        #[arg(long)]
        description: Option<String>,
        
        #[arg(long)]
        scope: Option<String>,
    },
    
    /// 添加授权租户
    PublishAllow {
        name: String,
        
        tenant: String,
    },
    
    /// 移除授权租户
    PublishRevoke {
        name: String,
        
        tenant: String,
    },
}
```

### 5. TOML 配置文件解析

**文件**: `src/cli/config/mount_config.rs`

```rust
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct MountConfig {
    #[serde(rename = "mounts")]
    pub entries: Vec<MountEntryConfig>,
}

#[derive(Deserialize, Serialize)]
pub struct MountEntryConfig {
    pub name: String,
    pub path: String,
    pub source: String,
    pub mode: String,
    
    #[serde(default)]
    pub file: bool,
    
    pub subpath: Option<String>,
    
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool { true }

impl MountConfig {
    /// 从 TOML 文件加载
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: MountConfig = toml::from_str(&content)?;
        Ok(config)
    }
    
    /// 保存到 TOML 文件
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
    
    /// 验证配置
    pub fn validate(&self) -> Result<()> {
        // 检查名称唯一
        // 检查路径无冲突
        // 检查 source 格式有效
        // ...
    }
}

/// 解析 source 字符串
/// 
/// 格式:
/// - "working_layer"
/// - "host:/usr"
/// - "layer:tenant-name:layer-name"
/// - "published:publish-name"
pub fn parse_source(source: &str) -> Result<MountSource> {
    if source == "working_layer" {
        return Ok(MountSource::WorkingLayer);
    }
    
    if let Some(path) = source.strip_prefix("host:") {
        return Ok(MountSource::Host { path: PathBuf::from(path) });
    }
    
    if let Some(rest) = source.strip_prefix("layer:") {
        let parts: Vec<&str> = rest.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(anyhow!("Invalid layer source format"));
        }
        // 解析 tenant:layer
        // ...
    }
    
    if let Some(name) = source.strip_prefix("published:") {
        return Ok(MountSource::Published {
            publish_name: name.to_string(),
            subpath: None,
        });
    }
    
    Err(anyhow!("Unknown source format: {}", source))
}
```

### 6. 输出格式化

**文件**: `src/cli/output/composition.rs`

```rust
/// 表格输出挂载列表
pub fn print_mounts_table(mounts: &[MountEntry]) {
    println!("{:<12} {:<20} {:<5} {:<30} {:<4} {:<7}",
        "NAME", "PATH", "TYPE", "SOURCE", "MODE", "ENABLED");
    
    for m in mounts {
        let source_str = format_source(&m.source);
        let type_str = if m.is_file { "file" } else { "dir" };
        let enabled_str = if m.enabled { "true" } else { "false" };
        
        println!("{:<12} {:<20} {:<5} {:<30} {:<4} {:<7}",
            m.name,
            m.virtual_path.display(),
            type_str,
            source_str,
            format_mode(&m.mode),
            enabled_str
        );
    }
}

/// 表格输出已发布列表
pub fn print_published_table(published: &[PublishedMount]) {
    println!("{:<12} {:<20} {:<15} {:<12} {:<10}",
        "MOUNT", "PUBLISH_NAME", "TARGET", "SCOPE", "PUBLISHED");
    
    for p in published {
        let target_str = match &p.target {
            PublishTarget::Layer(_) => "layer",
            PublishTarget::WorkingLayer => "working_layer",
        };
        let scope_str = match &p.scope {
            PublishScope::Public => "public",
            PublishScope::AllowList(_) => "allow_list",
        };
        
        println!("{:<12} {:<20} {:<15} {:<12} {:<10}",
            // mount_name,
            p.publish_name,
            target_str,
            scope_str,
            p.created_at.format("%Y-%m-%d")
        );
    }
}

/// 路径解析输出
pub fn print_resolved_path(resolved: &ResolvedPath) {
    println!("Path:         {}", resolved.mount_entry.virtual_path.display());
    println!("Mount Name:   {}", resolved.mount_entry.name);
    println!("Mount Path:   {} ({})", 
        resolved.mount_entry.virtual_path.display(),
        if resolved.mount_entry.is_file { "file" } else { "dir" }
    );
    println!("Source:       {}", format_source(&resolved.mount_entry.source));
    println!("Relative:     {}", resolved.relative_path.display());
    println!("Mode:         {}", format_mode(&resolved.mount_entry.mode));
}
```

## 命令清单

| 命令 | 功能 | 示例 |
|------|------|------|
| `tarbox mount apply` | 应用配置文件 | `tarbox mount apply --tenant t1 --config mounts.toml` |
| `tarbox mount list` | 列出挂载 | `tarbox mount list --tenant t1 --toml` |
| `tarbox mount export` | 导出配置 | `tarbox mount export --tenant t1 --output mounts.toml` |
| `tarbox mount validate` | 验证配置 | `tarbox mount validate --config mounts.toml` |
| `tarbox mount clear` | 清空挂载 | `tarbox mount clear --tenant t1` |
| `tarbox mount remove` | 删除挂载 | `tarbox mount remove --tenant t1 --mount models` |
| `tarbox mount enable` | 启用挂载 | `tarbox mount enable --tenant t1 --mount models` |
| `tarbox mount disable` | 禁用挂载 | `tarbox mount disable --tenant t1 --mount models` |
| `tarbox mount update` | 更新挂载 | `tarbox mount update --tenant t1 --mount data --mode cow` |
| `tarbox mount resolve` | 解析路径 | `tarbox mount resolve --tenant t1 /models/bert.bin` |
| `tarbox snapshot` | Snapshot | `tarbox snapshot --tenant t1 --mount memory --name v1` |
| `tarbox publish` | 发布 | `tarbox publish --tenant t1 --mount memory --name shared-mem` |
| `tarbox unpublish` | 取消发布 | `tarbox unpublish --tenant t1 --mount memory` |
| `tarbox publish list` | 列出发布 | `tarbox publish list --tenant t1` |
| `tarbox layer list-published` | 全局发布 | `tarbox layer list-published --scope public` |
| `tarbox layer publish-info` | 发布详情 | `tarbox layer publish-info my-model-v1` |
| `tarbox layer publish-update` | 更新发布 | `tarbox layer publish-update my-model-v1 --scope public` |
| `tarbox layer publish-allow` | 添加授权 | `tarbox layer publish-allow my-model-v1 tenant-b` |
| `tarbox layer publish-revoke` | 移除授权 | `tarbox layer publish-revoke my-model-v1 tenant-b` |

## 测试要求

### 单元测试 (target: 20+)

1. **TOML 配置解析** (10 tests)
   - 完整配置解析
   - 最小配置解析
   - 各种 source 格式
   - 默认值填充
   - 无效配置报错

2. **Source 字符串解析** (6 tests)
   - working_layer
   - host:/path
   - layer:tenant:layer
   - published:name
   - 无效格式

3. **配置验证** (4 tests)
   - 名称重复
   - 路径冲突
   - 无效模式

### 集成测试 (target: 25+)

1. **Mount 命令** (12 tests)
   - mount apply 成功
   - mount apply 配置错误
   - mount list (table/json/toml)
   - mount export
   - mount validate
   - mount clear
   - mount remove
   - mount enable/disable
   - mount update
   - mount resolve

2. **Snapshot 命令** (6 tests)
   - snapshot 单个
   - snapshot 多个
   - snapshot --all
   - snapshot --skip-unchanged
   - snapshot 不存在的挂载

3. **Publish 命令** (7 tests)
   - publish working_layer
   - publish layer
   - publish list
   - unpublish
   - layer list-published
   - layer publish-allow/revoke

## 文件清单

```
src/cli/
├── commands/
│   ├── mod.rs
│   ├── mount.rs                # Mount 子命令
│   ├── snapshot.rs             # Snapshot 命令
│   ├── publish.rs              # Publish 命令
│   └── layer.rs                # 更新：发布管理命令
├── config/
│   ├── mod.rs
│   └── mount_config.rs         # TOML 配置解析
└── output/
    ├── mod.rs
    └── composition.rs          # 输出格式化
```

## 完成标准

- [ ] 所有 CLI 命令实现
- [ ] TOML 配置文件解析
- [ ] 输出格式化（表格/JSON/TOML）
- [ ] 20+ 单元测试通过
- [ ] 25+ 集成测试通过
- [ ] cargo fmt 通过
- [ ] cargo clippy 通过
- [ ] 测试覆盖率 > 80%
- [ ] 帮助文档完整

## 预计工作量

2-3 天

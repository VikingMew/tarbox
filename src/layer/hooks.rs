//! Filesystem hooks module.
//!
//! Provides virtual filesystem interface at `/.tarbox/` for layer management.
//! Users can control layers through standard file operations.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::layer::manager::{LayerManager, LayerManagerError};
use crate::storage::Layer;
use crate::types::TenantId;

/// The base path for tarbox hooks.
pub const TARBOX_HOOK_PATH: &str = "/.tarbox";

/// Virtual file paths under /.tarbox/
pub mod paths {
    pub const LAYERS: &str = "/.tarbox/layers";
    pub const LAYERS_CURRENT: &str = "/.tarbox/layers/current";
    pub const LAYERS_LIST: &str = "/.tarbox/layers/list";
    pub const LAYERS_NEW: &str = "/.tarbox/layers/new";
    pub const LAYERS_SWITCH: &str = "/.tarbox/layers/switch";
    pub const LAYERS_DROP: &str = "/.tarbox/layers/drop";
    pub const LAYERS_TREE: &str = "/.tarbox/layers/tree";
    pub const LAYERS_DIFF: &str = "/.tarbox/layers/diff";
    pub const SNAPSHOTS: &str = "/.tarbox/snapshots";
    pub const STATS: &str = "/.tarbox/stats";
    pub const STATS_USAGE: &str = "/.tarbox/stats/usage";
}

/// Result of a hook operation.
#[derive(Debug)]
pub enum HookResult {
    /// Read operation result with content.
    Content(String),
    /// Write operation completed successfully.
    WriteSuccess { message: String },
    /// Error occurred.
    Error(HookError),
    /// Path is not a hook path (should be handled normally).
    NotAHook,
}

/// Hook-specific errors.
#[derive(Debug, thiserror::Error)]
pub enum HookError {
    #[error("Invalid hook path: {0}")]
    InvalidPath(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Layer error: {0}")]
    LayerError(#[from] LayerManagerError),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Input for creating a new layer.
#[derive(Debug, Deserialize)]
pub struct CreateLayerInput {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub confirm: bool,
}

/// Input for switching layers.
#[derive(Debug, Deserialize)]
pub struct SwitchLayerInput {
    pub layer: String, // Can be name or UUID
}

/// Input for dropping a layer.
#[derive(Debug, Deserialize)]
pub struct DropLayerInput {
    pub layer: String,
    #[serde(default)]
    pub force: bool,
}

/// Layer info for JSON output.
#[derive(Debug, Serialize)]
pub struct LayerInfo {
    pub layer_id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub created_at: String,
    pub is_readonly: bool,
    pub is_current: bool,
    pub file_count: i32,
    pub total_size: i64,
    pub description: Option<String>,
}

impl LayerInfo {
    fn from_layer(layer: &Layer, is_current: bool) -> Self {
        Self {
            layer_id: layer.layer_id.to_string(),
            name: layer.layer_name.clone(),
            parent_id: layer.parent_layer_id.map(|id| id.to_string()),
            created_at: layer.created_at.to_rfc3339(),
            is_readonly: layer.is_readonly,
            is_current,
            file_count: layer.file_count,
            total_size: layer.total_size,
            description: layer.description.clone(),
        }
    }
}

/// Filesystem hooks handler.
pub struct HooksHandler<'a> {
    pool: &'a PgPool,
    tenant_id: TenantId,
}

impl<'a> HooksHandler<'a> {
    /// Create a new hooks handler.
    pub fn new(pool: &'a PgPool, tenant_id: TenantId) -> Self {
        Self { pool, tenant_id }
    }

    /// Check if a path is a hook path.
    pub fn is_hook_path(path: &str) -> bool {
        path.starts_with(TARBOX_HOOK_PATH)
    }

    /// Handle a read operation on a hook path.
    pub async fn handle_read(&self, path: &str) -> HookResult {
        if !Self::is_hook_path(path) {
            return HookResult::NotAHook;
        }

        match path {
            paths::LAYERS_CURRENT => self.read_current_layer().await,
            paths::LAYERS_LIST => self.read_layer_list().await,
            paths::LAYERS_TREE => self.read_layer_tree().await,
            paths::LAYERS_DIFF => self.read_current_diff().await,
            paths::STATS_USAGE => self.read_stats_usage().await,
            _ if path.starts_with(paths::SNAPSHOTS) => self.handle_snapshot_read(path).await,
            _ => HookResult::Error(HookError::InvalidPath(path.to_string())),
        }
    }

    /// Handle a write operation on a hook path.
    pub async fn handle_write(&self, path: &str, data: &[u8]) -> HookResult {
        if !Self::is_hook_path(path) {
            return HookResult::NotAHook;
        }

        let input = match std::str::from_utf8(data) {
            Ok(s) => s.trim(),
            Err(_) => {
                return HookResult::Error(HookError::InvalidInput(
                    "Input must be valid UTF-8".to_string(),
                ));
            }
        };

        match path {
            paths::LAYERS_NEW => self.write_new_layer(input).await,
            paths::LAYERS_SWITCH => self.write_switch_layer(input).await,
            paths::LAYERS_DROP => self.write_drop_layer(input).await,
            _ => {
                HookResult::Error(HookError::PermissionDenied(format!("Cannot write to {}", path)))
            }
        }
    }

    /// Handle getattr for hook paths.
    pub fn get_attr(&self, path: &str) -> Option<HookFileAttr> {
        if !Self::is_hook_path(path) {
            return None;
        }

        match path {
            TARBOX_HOOK_PATH => Some(HookFileAttr::directory()),
            paths::LAYERS => Some(HookFileAttr::directory()),
            paths::LAYERS_CURRENT => Some(HookFileAttr::readonly_file()),
            paths::LAYERS_LIST => Some(HookFileAttr::readonly_file()),
            paths::LAYERS_NEW => Some(HookFileAttr::writeonly_file()),
            paths::LAYERS_SWITCH => Some(HookFileAttr::writeonly_file()),
            paths::LAYERS_DROP => Some(HookFileAttr::writeonly_file()),
            paths::LAYERS_TREE => Some(HookFileAttr::readonly_file()),
            paths::LAYERS_DIFF => Some(HookFileAttr::readonly_file()),
            paths::SNAPSHOTS => Some(HookFileAttr::directory()),
            paths::STATS => Some(HookFileAttr::directory()),
            paths::STATS_USAGE => Some(HookFileAttr::readonly_file()),
            _ if path.starts_with(paths::SNAPSHOTS) => Some(HookFileAttr::directory()),
            _ => None,
        }
    }

    /// List directory contents for hook paths.
    pub async fn read_dir(&self, path: &str) -> HookResult {
        if !Self::is_hook_path(path) {
            return HookResult::NotAHook;
        }

        let entries = match path {
            TARBOX_HOOK_PATH => vec!["layers", "snapshots", "stats"],
            paths::LAYERS => vec!["current", "list", "new", "switch", "drop", "tree", "diff"],
            paths::SNAPSHOTS => {
                // List all layers as snapshot directories
                let manager = LayerManager::new(self.pool, self.tenant_id);
                let layer_names: Vec<String> = match manager.list_layers().await {
                    Ok(layers) => layers.iter().map(|l| l.layer_name.clone()).collect(),
                    Err(_) => vec![],
                };
                let output = layer_names.join("\n");
                return HookResult::Content(output);
            }
            paths::STATS => vec!["usage"],
            _ => return HookResult::Error(HookError::InvalidPath(path.to_string())),
        };

        let output = entries.join("\n");
        HookResult::Content(output)
    }

    // --- Read handlers ---

    async fn read_current_layer(&self) -> HookResult {
        let manager = LayerManager::new(self.pool, self.tenant_id);

        match manager.get_current_layer().await {
            Ok(layer) => {
                let info = LayerInfo::from_layer(&layer, true);
                match serde_json::to_string_pretty(&info) {
                    Ok(json) => HookResult::Content(json),
                    Err(e) => HookResult::Error(HookError::Internal(e.to_string())),
                }
            }
            Err(LayerManagerError::NoCurrentLayer) => {
                HookResult::Content("No current layer set\n".to_string())
            }
            Err(e) => HookResult::Error(HookError::LayerError(e)),
        }
    }

    async fn read_layer_list(&self) -> HookResult {
        let manager = LayerManager::new(self.pool, self.tenant_id);

        match manager.list_layers().await {
            Ok(layers) => {
                let current_id = manager.get_current_layer_id().await.ok().flatten();
                let infos: Vec<LayerInfo> = layers
                    .iter()
                    .map(|l| {
                        let is_current = current_id.map(|id| id == l.layer_id).unwrap_or(false);
                        LayerInfo::from_layer(l, is_current)
                    })
                    .collect();

                match serde_json::to_string_pretty(&infos) {
                    Ok(json) => HookResult::Content(json),
                    Err(e) => HookResult::Error(HookError::Internal(e.to_string())),
                }
            }
            Err(e) => HookResult::Error(HookError::LayerError(e)),
        }
    }

    async fn read_layer_tree(&self) -> HookResult {
        let manager = LayerManager::new(self.pool, self.tenant_id);

        match manager.list_layers().await {
            Ok(layers) => {
                let current_id = manager.get_current_layer_id().await.ok().flatten();

                // Build tree representation
                let mut output = String::new();

                // Sort layers by parent chain (base first)
                let mut sorted_layers = layers.clone();
                sorted_layers.sort_by(|a, b| {
                    let a_has_parent = a.parent_layer_id.is_some();
                    let b_has_parent = b.parent_layer_id.is_some();
                    a_has_parent.cmp(&b_has_parent)
                });

                for layer in &sorted_layers {
                    let is_current = current_id.map(|id| id == layer.layer_id).unwrap_or(false);
                    let marker = if is_current { " [current]" } else { "" };
                    let prefix = if layer.parent_layer_id.is_some() { "├─ " } else { "" };
                    output.push_str(&format!("{}{}{}\n", prefix, layer.layer_name, marker));
                }

                HookResult::Content(output)
            }
            Err(e) => HookResult::Error(HookError::LayerError(e)),
        }
    }

    async fn read_current_diff(&self) -> HookResult {
        let manager = LayerManager::new(self.pool, self.tenant_id);

        match manager.get_current_layer().await {
            Ok(layer) => {
                let entries = match manager.get_layer_entries(layer.layer_id).await {
                    Ok(e) => e,
                    Err(e) => return HookResult::Error(HookError::LayerError(e)),
                };

                let mut output = String::new();
                output.push_str(&format!("Layer: {} ({})\n", layer.layer_name, layer.layer_id));
                output.push_str(&format!("Changes: {} files\n\n", entries.len()));

                for entry in entries {
                    let change_char = match entry.change_type {
                        crate::storage::ChangeType::Add => 'A',
                        crate::storage::ChangeType::Modify => 'M',
                        crate::storage::ChangeType::Delete => 'D',
                    };
                    output.push_str(&format!("{}  {}\n", change_char, entry.path));
                }

                HookResult::Content(output)
            }
            Err(LayerManagerError::NoCurrentLayer) => {
                HookResult::Content("No current layer set\n".to_string())
            }
            Err(e) => HookResult::Error(HookError::LayerError(e)),
        }
    }

    async fn read_stats_usage(&self) -> HookResult {
        let manager = LayerManager::new(self.pool, self.tenant_id);

        match manager.list_layers().await {
            Ok(layers) => {
                let total_size: i64 = layers.iter().map(|l| l.total_size).sum();
                let total_files: i32 = layers.iter().map(|l| l.file_count).sum();

                let stats = serde_json::json!({
                    "layer_count": layers.len(),
                    "total_size": total_size,
                    "total_files": total_files,
                    "tenant_id": self.tenant_id.to_string(),
                });

                match serde_json::to_string_pretty(&stats) {
                    Ok(json) => HookResult::Content(json),
                    Err(e) => HookResult::Error(HookError::Internal(e.to_string())),
                }
            }
            Err(e) => HookResult::Error(HookError::LayerError(e)),
        }
    }

    async fn handle_snapshot_read(&self, path: &str) -> HookResult {
        // Extract layer name from path: /.tarbox/snapshots/<layer-name>/...
        let suffix = path.strip_prefix(paths::SNAPSHOTS).unwrap_or("");
        let parts: Vec<&str> = suffix.trim_start_matches('/').split('/').collect();

        if parts.is_empty() || parts[0].is_empty() {
            return self.read_dir(paths::SNAPSHOTS).await;
        }

        let layer_name = parts[0];
        let manager = LayerManager::new(self.pool, self.tenant_id);

        // Find layer by name
        let layers = match manager.list_layers().await {
            Ok(l) => l,
            Err(e) => return HookResult::Error(HookError::LayerError(e)),
        };

        let layer = match layers.iter().find(|l| l.layer_name == layer_name) {
            Some(l) => l,
            None => {
                return HookResult::Error(HookError::InvalidPath(format!(
                    "Layer not found: {}",
                    layer_name
                )));
            }
        };

        // For now, just return layer info
        // Full snapshot file browsing would require more implementation
        let info = LayerInfo::from_layer(layer, false);
        match serde_json::to_string_pretty(&info) {
            Ok(json) => HookResult::Content(json),
            Err(e) => HookResult::Error(HookError::Internal(e.to_string())),
        }
    }

    // --- Write handlers ---

    async fn write_new_layer(&self, input: &str) -> HookResult {
        let manager = LayerManager::new(self.pool, self.tenant_id);

        // Try to parse as JSON first
        let (name, description, confirm) = if input.starts_with('{') {
            match serde_json::from_str::<CreateLayerInput>(input) {
                Ok(parsed) => (parsed.name, parsed.description, parsed.confirm),
                Err(e) => {
                    return HookResult::Error(HookError::InvalidInput(format!(
                        "Invalid JSON: {}",
                        e
                    )));
                }
            }
        } else {
            // Simple text input - just the layer name
            (input.to_string(), None, false)
        };

        match manager.create_checkpoint_with_confirm(&name, description.as_deref(), confirm).await {
            Ok(layer) => HookResult::WriteSuccess {
                message: format!("Created layer '{}' ({})\n", layer.layer_name, layer.layer_id),
            },
            Err(LayerManagerError::HistoricalLayerNeedsConfirmation {
                current_layer,
                future_layers,
            }) => {
                let mut msg =
                    format!("Warning: You are at a historical layer ({}).\n", current_layer);
                msg.push_str("Creating a new layer will delete future layers:\n");
                for layer in &future_layers {
                    msg.push_str(&format!("  - {}\n", layer.layer_name));
                }
                msg.push_str("\nTo proceed, write JSON with confirm flag:\n");
                msg.push_str(&format!("{{\"name\": \"{}\", \"confirm\": true}}\n", name));
                HookResult::Error(HookError::InvalidInput(msg))
            }
            Err(e) => HookResult::Error(HookError::LayerError(e)),
        }
    }

    async fn write_switch_layer(&self, input: &str) -> HookResult {
        let manager = LayerManager::new(self.pool, self.tenant_id);

        // Try to parse as JSON first
        let layer_ref = if input.starts_with('{') {
            match serde_json::from_str::<SwitchLayerInput>(input) {
                Ok(parsed) => parsed.layer,
                Err(e) => {
                    return HookResult::Error(HookError::InvalidInput(format!(
                        "Invalid JSON: {}",
                        e
                    )));
                }
            }
        } else {
            input.to_string()
        };

        // Try to parse as UUID first, then as name
        let layer_id = if let Ok(uuid) = layer_ref.parse::<uuid::Uuid>() {
            uuid
        } else {
            // Find by name
            let layers = match manager.list_layers().await {
                Ok(l) => l,
                Err(e) => return HookResult::Error(HookError::LayerError(e)),
            };

            match layers.iter().find(|l| l.layer_name == layer_ref) {
                Some(l) => l.layer_id,
                None => {
                    return HookResult::Error(HookError::InvalidInput(format!(
                        "Layer not found: {}",
                        layer_ref
                    )));
                }
            }
        };

        match manager.switch_to_layer(layer_id).await {
            Ok(layer) => HookResult::WriteSuccess {
                message: format!("Switched to layer '{}' ({})\n", layer.layer_name, layer_id),
            },
            Err(e) => HookResult::Error(HookError::LayerError(e)),
        }
    }

    async fn write_drop_layer(&self, input: &str) -> HookResult {
        let manager = LayerManager::new(self.pool, self.tenant_id);

        // Try to parse as JSON first
        let (layer_ref, _force) = if input.starts_with('{') {
            match serde_json::from_str::<DropLayerInput>(input) {
                Ok(parsed) => (parsed.layer, parsed.force),
                Err(e) => {
                    return HookResult::Error(HookError::InvalidInput(format!(
                        "Invalid JSON: {}",
                        e
                    )));
                }
            }
        } else {
            (input.to_string(), false)
        };

        // Special case: "current" means current layer
        let layer_id = if layer_ref == "current" {
            match manager.get_current_layer_id().await {
                Ok(Some(id)) => id,
                Ok(None) => {
                    return HookResult::Error(HookError::InvalidInput(
                        "No current layer set".to_string(),
                    ));
                }
                Err(e) => return HookResult::Error(HookError::LayerError(e)),
            }
        } else if let Ok(uuid) = layer_ref.parse::<uuid::Uuid>() {
            uuid
        } else {
            // Find by name
            let layers = match manager.list_layers().await {
                Ok(l) => l,
                Err(e) => return HookResult::Error(HookError::LayerError(e)),
            };

            match layers.iter().find(|l| l.layer_name == layer_ref) {
                Some(l) => l.layer_id,
                None => {
                    return HookResult::Error(HookError::InvalidInput(format!(
                        "Layer not found: {}",
                        layer_ref
                    )));
                }
            }
        };

        match manager.delete_layer(layer_id).await {
            Ok(()) => HookResult::WriteSuccess { message: format!("Deleted layer {}\n", layer_id) },
            Err(LayerManagerError::HasChildLayers(id)) => {
                HookResult::Error(HookError::InvalidInput(format!(
                    "Layer {} has child layers. Use {{\"layer\": \"{}\", \"force\": true}} to delete anyway.",
                    id, layer_ref
                )))
            }
            Err(e) => HookResult::Error(HookError::LayerError(e)),
        }
    }
}

/// File attributes for hook files.
#[derive(Debug, Clone)]
pub struct HookFileAttr {
    pub is_dir: bool,
    pub mode: u32,
    pub size: u64,
}

impl HookFileAttr {
    pub fn directory() -> Self {
        Self { is_dir: true, mode: 0o755, size: 4096 }
    }

    pub fn readonly_file() -> Self {
        Self {
            is_dir: false,
            mode: 0o444,
            size: 0, // Size is dynamic
        }
    }

    pub fn writeonly_file() -> Self {
        Self { is_dir: false, mode: 0o200, size: 0 }
    }

    pub fn readwrite_file() -> Self {
        Self { is_dir: false, mode: 0o644, size: 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_hook_path() {
        assert!(HooksHandler::is_hook_path("/.tarbox"));
        assert!(HooksHandler::is_hook_path("/.tarbox/layers"));
        assert!(HooksHandler::is_hook_path("/.tarbox/layers/current"));
        assert!(!HooksHandler::is_hook_path("/data"));
        assert!(!HooksHandler::is_hook_path("/.tar"));
    }

    #[test]
    fn test_is_hook_path_edge_cases() {
        assert!(HooksHandler::is_hook_path("/.tarbox/"));
        assert!(HooksHandler::is_hook_path("/.tarbox/stats"));
        assert!(HooksHandler::is_hook_path("/.tarbox/stats/usage"));
        assert!(HooksHandler::is_hook_path("/.tarbox/snapshots"));
        assert!(!HooksHandler::is_hook_path("/"));
        assert!(!HooksHandler::is_hook_path(""));
        assert!(!HooksHandler::is_hook_path("/tarbox"));
        assert!(!HooksHandler::is_hook_path("/.tarbo"));
        assert!(!HooksHandler::is_hook_path("/home/.tarbox"));
    }

    #[test]
    fn test_hook_file_attr() {
        let dir = HookFileAttr::directory();
        assert!(dir.is_dir);
        assert_eq!(dir.mode, 0o755);

        let readonly = HookFileAttr::readonly_file();
        assert!(!readonly.is_dir);
        assert_eq!(readonly.mode, 0o444);

        let writeonly = HookFileAttr::writeonly_file();
        assert!(!writeonly.is_dir);
        assert_eq!(writeonly.mode, 0o200);
    }

    #[test]
    fn test_hook_file_attr_readwrite() {
        let rw = HookFileAttr::readwrite_file();
        assert!(!rw.is_dir);
        assert_eq!(rw.mode, 0o644);
        assert_eq!(rw.size, 0);
    }

    #[test]
    fn test_hook_file_attr_directory_size() {
        let dir = HookFileAttr::directory();
        assert_eq!(dir.size, 4096);
    }

    #[test]
    fn test_layer_info_serialization() {
        let layer = Layer {
            layer_id: uuid::Uuid::new_v4(),
            tenant_id: uuid::Uuid::new_v4(),
            parent_layer_id: None,
            layer_name: "test-layer".to_string(),
            description: Some("Test description".to_string()),
            file_count: 10,
            total_size: 1024,
            status: crate::storage::LayerStatus::Active,
            is_readonly: false,
            tags: None,
            created_at: chrono::Utc::now(),
            created_by: "test".to_string(),
        };

        let info = LayerInfo::from_layer(&layer, true);
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("test-layer"));
        assert!(json.contains("\"is_current\":true"));
    }

    #[test]
    fn test_layer_info_not_current() {
        let layer = Layer {
            layer_id: uuid::Uuid::new_v4(),
            tenant_id: uuid::Uuid::new_v4(),
            parent_layer_id: Some(uuid::Uuid::new_v4()),
            layer_name: "child-layer".to_string(),
            description: None,
            file_count: 5,
            total_size: 512,
            status: crate::storage::LayerStatus::Active,
            is_readonly: true,
            tags: None,
            created_at: chrono::Utc::now(),
            created_by: "user".to_string(),
        };

        let info = LayerInfo::from_layer(&layer, false);
        assert_eq!(info.name, "child-layer");
        assert!(!info.is_current);
        assert!(info.is_readonly);
        assert!(info.parent_id.is_some());
        assert!(info.description.is_none());
    }

    #[test]
    fn test_layer_info_fields() {
        let layer_id = uuid::Uuid::new_v4();
        let parent_id = uuid::Uuid::new_v4();
        let layer = Layer {
            layer_id,
            tenant_id: uuid::Uuid::new_v4(),
            parent_layer_id: Some(parent_id),
            layer_name: "my-layer".to_string(),
            description: Some("A layer".to_string()),
            file_count: 100,
            total_size: 10240,
            status: crate::storage::LayerStatus::Active,
            is_readonly: false,
            tags: None,
            created_at: chrono::Utc::now(),
            created_by: "admin".to_string(),
        };

        let info = LayerInfo::from_layer(&layer, true);
        assert_eq!(info.layer_id, layer_id.to_string());
        assert_eq!(info.parent_id, Some(parent_id.to_string()));
        assert_eq!(info.file_count, 100);
        assert_eq!(info.total_size, 10240);
        assert_eq!(info.description, Some("A layer".to_string()));
    }

    #[test]
    fn test_hook_result_variants() {
        let content = HookResult::Content("test content".to_string());
        assert!(matches!(content, HookResult::Content(_)));

        let success = HookResult::WriteSuccess { message: "ok".to_string() };
        assert!(matches!(success, HookResult::WriteSuccess { .. }));

        let error = HookResult::Error(HookError::InvalidPath("/bad".to_string()));
        assert!(matches!(error, HookResult::Error(_)));

        let not_hook = HookResult::NotAHook;
        assert!(matches!(not_hook, HookResult::NotAHook));
    }

    #[test]
    fn test_hook_error_display() {
        let err = HookError::InvalidPath("/test".to_string());
        assert!(err.to_string().contains("/test"));

        let err = HookError::PermissionDenied("no access".to_string());
        assert!(err.to_string().contains("no access"));

        let err = HookError::InvalidInput("bad data".to_string());
        assert!(err.to_string().contains("bad data"));

        let err = HookError::Internal("oops".to_string());
        assert!(err.to_string().contains("oops"));
    }

    #[test]
    fn test_create_layer_input_deserialization() {
        let json = r#"{"name": "my-layer", "description": "test", "confirm": true}"#;
        let input: CreateLayerInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.name, "my-layer");
        assert_eq!(input.description, Some("test".to_string()));
        assert!(input.confirm);
    }

    #[test]
    fn test_create_layer_input_minimal() {
        let json = r#"{"name": "simple"}"#;
        let input: CreateLayerInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.name, "simple");
        assert!(input.description.is_none());
        assert!(!input.confirm);
    }

    #[test]
    fn test_switch_layer_input_deserialization() {
        let json = r#"{"layer": "my-layer"}"#;
        let input: SwitchLayerInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.layer, "my-layer");
    }

    #[test]
    fn test_drop_layer_input_deserialization() {
        let json = r#"{"layer": "old-layer", "force": true}"#;
        let input: DropLayerInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.layer, "old-layer");
        assert!(input.force);
    }

    #[test]
    fn test_drop_layer_input_minimal() {
        let json = r#"{"layer": "layer-name"}"#;
        let input: DropLayerInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.layer, "layer-name");
        assert!(!input.force);
    }

    #[test]
    fn test_tarbox_hook_path_constant() {
        assert_eq!(TARBOX_HOOK_PATH, "/.tarbox");
    }

    #[test]
    fn test_paths_constants() {
        assert_eq!(paths::LAYERS, "/.tarbox/layers");
        assert_eq!(paths::LAYERS_CURRENT, "/.tarbox/layers/current");
        assert_eq!(paths::LAYERS_LIST, "/.tarbox/layers/list");
        assert_eq!(paths::LAYERS_NEW, "/.tarbox/layers/new");
        assert_eq!(paths::LAYERS_SWITCH, "/.tarbox/layers/switch");
        assert_eq!(paths::LAYERS_DROP, "/.tarbox/layers/drop");
        assert_eq!(paths::LAYERS_TREE, "/.tarbox/layers/tree");
        assert_eq!(paths::LAYERS_DIFF, "/.tarbox/layers/diff");
        assert_eq!(paths::SNAPSHOTS, "/.tarbox/snapshots");
        assert_eq!(paths::STATS, "/.tarbox/stats");
        assert_eq!(paths::STATS_USAGE, "/.tarbox/stats/usage");
    }
}

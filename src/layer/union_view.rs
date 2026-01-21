//! Union view module.
//!
//! Provides a unified view of files across multiple layers,
//! implementing the union filesystem semantics.

use anyhow::Result;
use sqlx::PgPool;
use std::collections::HashMap;

use crate::storage::{ChangeType, Layer, LayerOperations, LayerRepository};
use crate::types::{InodeId, LayerId, TenantId};

/// Represents the state of a file in the union view.
#[derive(Debug, Clone)]
pub enum FileState {
    /// File exists with data from specified layer.
    Exists { layer_id: LayerId, inode_id: InodeId },
    /// File has been deleted (tombstone).
    Deleted { deleted_in_layer: LayerId },
    /// File does not exist in any layer.
    NotFound,
}

impl FileState {
    /// Returns true if the file exists.
    pub fn exists(&self) -> bool {
        matches!(self, FileState::Exists { .. })
    }

    /// Returns the inode ID if the file exists.
    pub fn inode_id(&self) -> Option<InodeId> {
        match self {
            FileState::Exists { inode_id, .. } => Some(*inode_id),
            _ => None,
        }
    }

    /// Returns the layer ID where the file was found or deleted.
    pub fn layer_id(&self) -> Option<LayerId> {
        match self {
            FileState::Exists { layer_id, .. } => Some(*layer_id),
            FileState::Deleted { deleted_in_layer } => Some(*deleted_in_layer),
            FileState::NotFound => None,
        }
    }
}

/// Union view provides a merged view of files across layers.
pub struct UnionView<'a> {
    pool: &'a PgPool,
    tenant_id: TenantId,
    /// The layer chain from current layer to base (current first).
    layer_chain: Vec<Layer>,
}

impl<'a> UnionView<'a> {
    /// Create a union view from a specific layer.
    pub async fn from_layer(
        pool: &'a PgPool,
        tenant_id: TenantId,
        layer_id: LayerId,
    ) -> Result<Self> {
        let layer_ops = LayerOperations::new(pool);
        let layer_chain = layer_ops.get_layer_chain(tenant_id, layer_id).await?;

        Ok(Self { pool, tenant_id, layer_chain })
    }

    /// Create a union view from the current layer.
    pub async fn from_current(pool: &'a PgPool, tenant_id: TenantId) -> Result<Option<Self>> {
        let layer_ops = LayerOperations::new(pool);

        let current_layer_id = match layer_ops.get_current_layer(tenant_id).await? {
            Some(id) => id,
            None => return Ok(None),
        };

        Ok(Some(Self::from_layer(pool, tenant_id, current_layer_id).await?))
    }

    /// Get the current layer ID.
    pub fn current_layer_id(&self) -> Option<LayerId> {
        self.layer_chain.first().map(|l| l.layer_id)
    }

    /// Get the layer chain (current to base).
    pub fn layer_chain(&self) -> &[Layer] {
        &self.layer_chain
    }

    /// Lookup a file by path in the union view.
    ///
    /// Traverses layers from current to base, returning the first match.
    /// If a Delete entry is found, returns Deleted state.
    pub async fn lookup_file(&self, path: &str) -> Result<FileState> {
        let layer_ops = LayerOperations::new(self.pool);

        for layer in &self.layer_chain {
            let entries = layer_ops.list_entries(self.tenant_id, layer.layer_id).await?;

            // Find entry for this path
            for entry in entries {
                if entry.path == path {
                    match entry.change_type {
                        ChangeType::Delete => {
                            return Ok(FileState::Deleted { deleted_in_layer: layer.layer_id });
                        }
                        ChangeType::Add | ChangeType::Modify => {
                            return Ok(FileState::Exists {
                                layer_id: layer.layer_id,
                                inode_id: entry.inode_id,
                            });
                        }
                    }
                }
            }
        }

        // If not found in any layer entries, return NotFound
        // Files created before layering would be handled by resolving from base layer
        Ok(FileState::NotFound)
    }

    /// List all files in a directory across all layers.
    ///
    /// Merges directory contents from all layers, respecting delete markers.
    pub async fn list_directory(&self, dir_path: &str) -> Result<Vec<DirectoryEntry>> {
        let layer_ops = LayerOperations::new(self.pool);
        let mut result_map: HashMap<String, DirectoryEntry> = HashMap::new();

        // Traverse from oldest layer to newest (reverse order)
        for layer in self.layer_chain.iter().rev() {
            let entries = layer_ops.list_entries(self.tenant_id, layer.layer_id).await?;

            for entry in entries {
                // Check if this entry is in the target directory
                if let Some(parent) = get_parent_path(&entry.path)
                    && (parent == dir_path || (dir_path == "/" && parent.is_empty()))
                {
                    let name = get_filename(&entry.path);
                    match entry.change_type {
                        ChangeType::Delete => {
                            result_map.remove(&name);
                        }
                        ChangeType::Add | ChangeType::Modify => {
                            result_map.insert(
                                name.clone(),
                                DirectoryEntry {
                                    name,
                                    inode_id: entry.inode_id,
                                    layer_id: layer.layer_id,
                                },
                            );
                        }
                    }
                }
            }
        }

        Ok(result_map.into_values().collect())
    }

    /// Get the history of a file across layers.
    pub async fn get_file_history(&self, path: &str) -> Result<Vec<FileVersion>> {
        let layer_ops = LayerOperations::new(self.pool);
        let mut history = Vec::new();

        for layer in &self.layer_chain {
            let entries = layer_ops.list_entries(self.tenant_id, layer.layer_id).await?;

            for entry in entries {
                if entry.path == path {
                    history.push(FileVersion {
                        layer_id: layer.layer_id,
                        layer_name: layer.layer_name.clone(),
                        change_type: entry.change_type,
                        inode_id: entry.inode_id,
                        size_delta: entry.size_delta,
                        created_at: entry.created_at,
                    });
                }
            }
        }

        Ok(history)
    }

    /// Find the layer where a file was last modified.
    pub async fn find_file_layer(&self, path: &str) -> Result<Option<LayerId>> {
        let state = self.lookup_file(path).await?;
        Ok(state.layer_id())
    }

    /// Check if the file exists in the union view.
    pub async fn file_exists(&self, path: &str) -> Result<bool> {
        let state = self.lookup_file(path).await?;
        Ok(state.exists())
    }
}

/// A directory entry in the union view.
#[derive(Debug, Clone)]
pub struct DirectoryEntry {
    pub name: String,
    pub inode_id: InodeId,
    pub layer_id: LayerId,
}

/// A version of a file in the layer history.
#[derive(Debug, Clone)]
pub struct FileVersion {
    pub layer_id: LayerId,
    pub layer_name: String,
    pub change_type: ChangeType,
    pub inode_id: InodeId,
    pub size_delta: Option<i64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Get the parent path of a given path.
fn get_parent_path(path: &str) -> Option<String> {
    let path = path.trim_end_matches('/');
    if path.is_empty() || path == "/" {
        return None;
    }

    match path.rfind('/') {
        Some(0) => Some("/".to_string()),
        Some(pos) => Some(path[..pos].to_string()),
        None => Some(String::new()),
    }
}

/// Get the filename from a path.
fn get_filename(path: &str) -> String {
    let path = path.trim_end_matches('/');
    match path.rfind('/') {
        Some(pos) => path[pos + 1..].to_string(),
        None => path.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_parent_path() {
        assert_eq!(get_parent_path("/foo/bar"), Some("/foo".to_string()));
        assert_eq!(get_parent_path("/foo"), Some("/".to_string()));
        assert_eq!(get_parent_path("/"), None);
        assert_eq!(get_parent_path("/foo/bar/baz"), Some("/foo/bar".to_string()));
    }

    #[test]
    fn test_get_filename() {
        assert_eq!(get_filename("/foo/bar"), "bar");
        assert_eq!(get_filename("/foo"), "foo");
        assert_eq!(get_filename("foo"), "foo");
        assert_eq!(get_filename("/foo/bar/"), "bar");
    }

    #[test]
    fn test_file_state() {
        let exists = FileState::Exists { layer_id: uuid::Uuid::new_v4(), inode_id: 1 };
        assert!(exists.exists());
        assert!(exists.inode_id().is_some());

        let deleted = FileState::Deleted { deleted_in_layer: uuid::Uuid::new_v4() };
        assert!(!deleted.exists());
        assert!(deleted.inode_id().is_none());

        let not_found = FileState::NotFound;
        assert!(!not_found.exists());
        assert!(not_found.layer_id().is_none());
    }

    #[test]
    fn test_file_state_exists_layer_id() {
        let layer_id = uuid::Uuid::new_v4();
        let state = FileState::Exists { layer_id, inode_id: 123 };
        assert_eq!(state.layer_id(), Some(layer_id));
        assert_eq!(state.inode_id(), Some(123));
    }

    #[test]
    fn test_file_state_deleted_layer_id() {
        let layer_id = uuid::Uuid::new_v4();
        let state = FileState::Deleted { deleted_in_layer: layer_id };
        assert_eq!(state.layer_id(), Some(layer_id));
        assert_eq!(state.inode_id(), None);
        assert!(!state.exists());
    }

    #[test]
    fn test_file_state_not_found() {
        let state = FileState::NotFound;
        assert_eq!(state.layer_id(), None);
        assert_eq!(state.inode_id(), None);
        assert!(!state.exists());
    }

    #[test]
    fn test_get_parent_path_trailing_slash() {
        assert_eq!(get_parent_path("/foo/bar/"), Some("/foo".to_string()));
        assert_eq!(get_parent_path("/foo/"), Some("/".to_string()));
    }

    #[test]
    fn test_get_parent_path_no_slash() {
        assert_eq!(get_parent_path("foo"), Some(String::new()));
    }

    #[test]
    fn test_get_parent_path_empty() {
        assert_eq!(get_parent_path(""), None);
    }

    #[test]
    fn test_get_filename_root() {
        assert_eq!(get_filename("/"), "");
    }

    #[test]
    fn test_get_filename_empty() {
        assert_eq!(get_filename(""), "");
    }

    #[test]
    fn test_get_filename_deep_path() {
        assert_eq!(get_filename("/a/b/c/d/e/f"), "f");
    }

    #[test]
    fn test_directory_entry_construction() {
        let entry = DirectoryEntry {
            name: "test.txt".to_string(),
            inode_id: 42,
            layer_id: uuid::Uuid::new_v4(),
        };
        assert_eq!(entry.name, "test.txt");
        assert_eq!(entry.inode_id, 42);
    }

    #[test]
    fn test_file_version_construction() {
        let layer_id = uuid::Uuid::new_v4();
        let version = FileVersion {
            layer_id,
            layer_name: "v1.0".to_string(),
            change_type: ChangeType::Add,
            inode_id: 100,
            size_delta: Some(1024),
            created_at: chrono::Utc::now(),
        };
        assert_eq!(version.layer_name, "v1.0");
        assert_eq!(version.inode_id, 100);
        assert_eq!(version.size_delta, Some(1024));
    }

    #[test]
    fn test_file_version_with_modify() {
        let version = FileVersion {
            layer_id: uuid::Uuid::new_v4(),
            layer_name: "update".to_string(),
            change_type: ChangeType::Modify,
            inode_id: 200,
            size_delta: Some(-512),
            created_at: chrono::Utc::now(),
        };
        assert!(matches!(version.change_type, ChangeType::Modify));
        assert_eq!(version.size_delta, Some(-512));
    }

    #[test]
    fn test_file_version_with_delete() {
        let version = FileVersion {
            layer_id: uuid::Uuid::new_v4(),
            layer_name: "cleanup".to_string(),
            change_type: ChangeType::Delete,
            inode_id: 300,
            size_delta: None,
            created_at: chrono::Utc::now(),
        };
        assert!(matches!(version.change_type, ChangeType::Delete));
        assert!(version.size_delta.is_none());
    }
}

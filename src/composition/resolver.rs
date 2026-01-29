use anyhow::{Result, anyhow};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::storage::models::mount_entry::{MountEntry, MountSource};

/// Resolved path information
#[derive(Debug, Clone)]
pub struct ResolvedPath {
    pub mount_entry: MountEntry,
    pub relative_path: PathBuf,
    pub source: ResolvedSource,
}

/// Resolved source (where to actually read/write data)
#[derive(Debug, Clone)]
pub enum ResolvedSource {
    /// Host filesystem
    Host { full_path: PathBuf },

    /// Layer from a tenant
    Layer { tenant_id: Uuid, layer_id: Uuid, path: PathBuf },

    /// Working layer
    WorkingLayer { path: PathBuf },
}

/// Path resolver trait
pub trait PathResolver {
    /// Resolve a virtual path to its mount entry and relative path
    fn resolve_path(&self, mounts: &[MountEntry], path: &Path) -> Result<ResolvedPath>;

    /// Validate that adding a new mount won't create conflicts
    fn validate_no_conflict(
        &self,
        existing: &[MountEntry],
        new_path: &Path,
        is_file: bool,
    ) -> Result<()>;
}

/// Default path resolver implementation
pub struct DefaultPathResolver;

impl PathResolver for DefaultPathResolver {
    fn resolve_path(&self, mounts: &[MountEntry], path: &Path) -> Result<ResolvedPath> {
        // Find matching mount entry
        let mount = self.find_mount(mounts, path)?;

        // Compute relative path
        let relative_path = if mount.is_file {
            // File mount: no relative path
            PathBuf::from("")
        } else {
            // Directory mount: strip the virtual path prefix
            path.strip_prefix(&mount.virtual_path)
                .map_err(|_| anyhow!("Path resolution error"))?
                .to_path_buf()
        };

        // Resolve source
        let source = self.resolve_source(mount, &relative_path)?;

        Ok(ResolvedPath { mount_entry: mount.clone(), relative_path, source })
    }

    fn validate_no_conflict(
        &self,
        existing: &[MountEntry],
        new_path: &Path,
        is_file: bool,
    ) -> Result<()> {
        for entry in existing {
            // Check exact conflict
            if entry.virtual_path == new_path {
                return Err(anyhow!(
                    "MountPathConflict: path '{}' already exists",
                    new_path.display()
                ));
            }

            // Check nesting (only for directory mounts)
            if !is_file && !entry.is_file {
                // Check if new path is nested under existing
                if new_path.starts_with(&entry.virtual_path) {
                    return Err(anyhow!(
                        "MountPathConflict: path '{}' is nested under existing mount '{}'",
                        new_path.display(),
                        entry.virtual_path.display()
                    ));
                }

                // Check if existing path is nested under new
                if entry.virtual_path.starts_with(new_path) {
                    return Err(anyhow!(
                        "MountPathConflict: existing mount '{}' is nested under new path '{}'",
                        entry.virtual_path.display(),
                        new_path.display()
                    ));
                }
            }
        }

        Ok(())
    }
}

impl DefaultPathResolver {
    pub fn new() -> Self {
        Self
    }

    /// Find the mount entry that matches the given path
    fn find_mount<'a>(&self, mounts: &'a [MountEntry], path: &Path) -> Result<&'a MountEntry> {
        // First check for exact file mount match
        for mount in mounts {
            if mount.is_file && mount.virtual_path == path {
                return Ok(mount);
            }
        }

        // Then check for directory mount prefix match
        // Find the longest matching prefix
        let mut best_match: Option<&MountEntry> = None;
        let mut best_match_len = 0;

        for mount in mounts {
            if !mount.is_file && path.starts_with(&mount.virtual_path) {
                let match_len = mount.virtual_path.as_os_str().len();
                if match_len > best_match_len {
                    best_match = Some(mount);
                    best_match_len = match_len;
                }
            }
        }

        best_match.ok_or_else(|| anyhow!("No mount entry found for path '{}'", path.display()))
    }

    /// Resolve the mount source to actual data location
    fn resolve_source(&self, mount: &MountEntry, relative_path: &Path) -> Result<ResolvedSource> {
        match &mount.source {
            MountSource::Host { path: host_path } => {
                let full_path =
                    if mount.is_file { host_path.clone() } else { host_path.join(relative_path) };
                Ok(ResolvedSource::Host { full_path })
            }

            MountSource::Layer { source_mount_id: _, layer_id, subpath } => {
                // For now, we'll need the tenant_id from elsewhere
                // This will be properly implemented when integrated with storage layer
                let path = if let Some(sub) = subpath {
                    sub.join(relative_path)
                } else {
                    relative_path.to_path_buf()
                };

                // Note: tenant_id should be resolved from source_mount_id
                // For now, use a placeholder
                Ok(ResolvedSource::Layer {
                    tenant_id: Uuid::nil(),
                    layer_id: layer_id.unwrap_or(Uuid::nil()),
                    path,
                })
            }

            MountSource::Published { publish_name: _, subpath } => {
                // Will be resolved via LayerPublisher in Task 20
                let path = if let Some(sub) = subpath {
                    sub.join(relative_path)
                } else {
                    relative_path.to_path_buf()
                };

                // Placeholder - will be resolved properly
                Ok(ResolvedSource::Layer { tenant_id: Uuid::nil(), layer_id: Uuid::nil(), path })
            }

            MountSource::WorkingLayer => {
                let path = relative_path.to_path_buf();
                Ok(ResolvedSource::WorkingLayer { path })
            }
        }
    }
}

impl Default for DefaultPathResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::mount_entry::{MountMode, MountSource};
    use chrono::Utc;

    fn create_test_mount(name: &str, path: &str, is_file: bool, source: MountSource) -> MountEntry {
        MountEntry {
            mount_entry_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            name: name.to_string(),
            virtual_path: PathBuf::from(path),
            source,
            mode: MountMode::ReadOnly,
            is_file,
            enabled: true,
            current_layer_id: None,
            metadata: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_validate_no_conflict_empty() {
        let resolver = DefaultPathResolver::new();
        let result = resolver.validate_no_conflict(&[], &PathBuf::from("/data"), false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_no_conflict_exact_duplicate() {
        let resolver = DefaultPathResolver::new();
        let existing = vec![create_test_mount("data", "/data", false, MountSource::WorkingLayer)];
        let result = resolver.validate_no_conflict(&existing, &PathBuf::from("/data"), false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[test]
    fn test_validate_no_conflict_nested_under() {
        let resolver = DefaultPathResolver::new();
        let existing = vec![create_test_mount("data", "/data", false, MountSource::WorkingLayer)];
        let result =
            resolver.validate_no_conflict(&existing, &PathBuf::from("/data/models"), false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("nested under"));
    }

    #[test]
    fn test_validate_no_conflict_parent_of_existing() {
        let resolver = DefaultPathResolver::new();
        let existing =
            vec![create_test_mount("data", "/data/models", false, MountSource::WorkingLayer)];
        let result = resolver.validate_no_conflict(&existing, &PathBuf::from("/data"), false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("nested under new path"));
    }

    #[test]
    fn test_validate_no_conflict_parallel_paths() {
        let resolver = DefaultPathResolver::new();
        let existing = vec![
            create_test_mount("data", "/data", false, MountSource::WorkingLayer),
            create_test_mount("models", "/models", false, MountSource::WorkingLayer),
        ];
        let result = resolver.validate_no_conflict(&existing, &PathBuf::from("/workspace"), false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_no_conflict_file_mounts() {
        let resolver = DefaultPathResolver::new();
        let existing =
            vec![create_test_mount("config", "/config.yaml", true, MountSource::WorkingLayer)];
        // File mounts don't conflict with directory mounts at same path prefix
        let result = resolver.validate_no_conflict(&existing, &PathBuf::from("/config"), false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_find_mount_file_exact_match() {
        let resolver = DefaultPathResolver::new();
        let mounts =
            vec![create_test_mount("config", "/config.yaml", true, MountSource::WorkingLayer)];
        let result = resolver.find_mount(&mounts, &PathBuf::from("/config.yaml"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name, "config");
    }

    #[test]
    fn test_find_mount_directory_prefix() {
        let resolver = DefaultPathResolver::new();
        let mounts = vec![create_test_mount("data", "/data", false, MountSource::WorkingLayer)];
        let result = resolver.find_mount(&mounts, &PathBuf::from("/data/models/bert.bin"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name, "data");
    }

    #[test]
    fn test_find_mount_longest_prefix() {
        let resolver = DefaultPathResolver::new();
        let mounts = vec![
            create_test_mount("root", "/", false, MountSource::WorkingLayer),
            create_test_mount("data", "/data", false, MountSource::WorkingLayer),
            create_test_mount("models", "/data/models", false, MountSource::WorkingLayer),
        ];
        let result = resolver.find_mount(&mounts, &PathBuf::from("/data/models/bert.bin"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name, "models");
    }

    #[test]
    fn test_find_mount_no_match() {
        let resolver = DefaultPathResolver::new();
        let mounts = vec![create_test_mount("data", "/data", false, MountSource::WorkingLayer)];
        let result = resolver.find_mount(&mounts, &PathBuf::from("/models/bert.bin"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No mount entry"));
    }

    #[test]
    fn test_resolve_path_working_layer() {
        let resolver = DefaultPathResolver::new();
        let mounts =
            vec![create_test_mount("workspace", "/workspace", false, MountSource::WorkingLayer)];
        let result = resolver.resolve_path(&mounts, &PathBuf::from("/workspace/test.txt"));
        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert_eq!(resolved.mount_entry.name, "workspace");
        assert_eq!(resolved.relative_path, PathBuf::from("test.txt"));
    }

    #[test]
    fn test_resolve_path_host_source() {
        let resolver = DefaultPathResolver::new();
        let mounts = vec![create_test_mount(
            "usr",
            "/usr",
            false,
            MountSource::Host { path: PathBuf::from("/host/usr") },
        )];
        let result = resolver.resolve_path(&mounts, &PathBuf::from("/usr/bin/ls"));
        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert_eq!(resolved.relative_path, PathBuf::from("bin/ls"));
        match resolved.source {
            ResolvedSource::Host { full_path } => {
                assert_eq!(full_path, PathBuf::from("/host/usr/bin/ls"));
            }
            _ => panic!("Expected Host source"),
        }
    }

    #[test]
    fn test_resolve_path_file_mount() {
        let resolver = DefaultPathResolver::new();
        let mounts = vec![create_test_mount(
            "config",
            "/config.yaml",
            true,
            MountSource::Host { path: PathBuf::from("/host/config.yaml") },
        )];
        let result = resolver.resolve_path(&mounts, &PathBuf::from("/config.yaml"));
        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert_eq!(resolved.relative_path, PathBuf::from(""));
        match resolved.source {
            ResolvedSource::Host { full_path } => {
                assert_eq!(full_path, PathBuf::from("/host/config.yaml"));
            }
            _ => panic!("Expected Host source"),
        }
    }
}

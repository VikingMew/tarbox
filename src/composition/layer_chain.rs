use anyhow::{Result, anyhow};
use std::sync::Arc;
use uuid::Uuid;

use crate::storage::models::Layer;
use crate::storage::traits::{LayerRepository, MountEntryRepository};

/// Result of a snapshot operation
#[derive(Debug, Clone)]
pub struct SnapshotResult {
    pub mount_name: String,
    pub layer_id: Option<Uuid>,
    pub skipped: bool,
    pub reason: Option<String>,
}

/// Layer chain for a mount point
#[derive(Debug, Clone)]
pub struct LayerChain {
    pub mount_entry_id: Uuid,
    pub mount_name: String,
    pub layers: Vec<Layer>,
    pub working_layer: Layer,
}

/// Layer chain manager for mount-level operations
pub struct LayerChainManager {
    pub layer_repo: Arc<dyn LayerRepository>,
    pub mount_entry_repo: Arc<dyn MountEntryRepository>,
}

impl LayerChainManager {
    pub fn new(
        layer_repo: Arc<dyn LayerRepository>,
        mount_entry_repo: Arc<dyn MountEntryRepository>,
    ) -> Self {
        Self { layer_repo, mount_entry_repo }
    }

    /// Initialize layer chain for a new WorkingLayer mount
    ///
    /// Creates two layers:
    /// 1. Base layer (parent_layer_id = None, is_working = false)
    /// 2. Working layer (parent_layer_id = base.layer_id, is_working = true)
    pub async fn initialize_layer_chain(
        &self,
        tenant_id: Uuid,
        mount_entry_id: Uuid,
    ) -> Result<()> {
        let (_base_layer, _working_layer) =
            self.layer_repo.create_initial_layers(tenant_id, mount_entry_id).await?;

        Ok(())
    }

    /// Get the complete layer chain for a mount point
    pub async fn get_layer_chain(&self, mount_entry_id: Uuid) -> Result<LayerChain> {
        // Get mount entry
        let mount = self
            .mount_entry_repo
            .get_mount_entry(mount_entry_id)
            .await?
            .ok_or_else(|| anyhow!("Mount entry not found"))?;

        // Get all layers for this mount
        let mut layers = self.layer_repo.get_mount_layers(mount_entry_id).await?;

        // Find working layer
        let working_layer = layers
            .iter()
            .find(|l| l.is_working)
            .ok_or_else(|| anyhow!("Working layer not found for mount"))?
            .clone();

        // Sort layers by created_at for historical order
        layers.sort_by_key(|l| l.created_at);

        Ok(LayerChain { mount_entry_id, mount_name: mount.name, layers, working_layer })
    }

    /// Snapshot a single mount point
    ///
    /// Converts the current working layer to a snapshot and creates a new working layer
    pub async fn snapshot(
        &self,
        tenant_id: Uuid,
        mount_name: &str,
        snapshot_name: &str,
        description: Option<String>,
    ) -> Result<Layer> {
        // Get mount entry
        let mount = self
            .mount_entry_repo
            .get_mount_entry_by_name(tenant_id, mount_name)
            .await?
            .ok_or_else(|| anyhow!("Mount '{}' not found", mount_name))?;

        // Note: mount.source verification should be done here if needed
        // For now, we trust that the mount exists and is valid

        // Create snapshot
        self.layer_repo.create_snapshot(mount.mount_entry_id, snapshot_name, description).await
    }

    /// Snapshot multiple mount points
    pub async fn snapshot_multiple(
        &self,
        tenant_id: Uuid,
        mount_names: &[String],
        snapshot_name: &str,
        skip_unchanged: bool,
    ) -> Result<Vec<SnapshotResult>> {
        self.layer_repo.batch_snapshot(tenant_id, mount_names, snapshot_name, skip_unchanged).await
    }

    /// Snapshot all WorkingLayer mounts for a tenant
    pub async fn snapshot_all(
        &self,
        tenant_id: Uuid,
        snapshot_name: &str,
        skip_unchanged: bool,
    ) -> Result<Vec<SnapshotResult>> {
        // Get all mounts for tenant
        let mounts = self.mount_entry_repo.list_mount_entries(tenant_id).await?;

        // Filter to WorkingLayer mounts
        let working_layer_mounts: Vec<String> = mounts
            .into_iter()
            .filter(|m| {
                matches!(m.source, crate::storage::models::mount_entry::MountSource::WorkingLayer)
            })
            .map(|m| m.name)
            .collect();

        if working_layer_mounts.is_empty() {
            return Ok(vec![]);
        }

        self.snapshot_multiple(tenant_id, &working_layer_mounts, snapshot_name, skip_unchanged)
            .await
    }

    /// Check if a mount point has uncommitted changes
    pub async fn has_changes(&self, mount_entry_id: Uuid) -> Result<bool> {
        // Get working layer
        let working_layer = self
            .layer_repo
            .get_working_layer(mount_entry_id)
            .await?
            .ok_or_else(|| anyhow!("Working layer not found"))?;

        // Check if working layer has any entries
        Ok(working_layer.file_count > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::LayerStatus;
    use crate::storage::models::mount_entry::{MountEntry, MountMode, MountSource};
    use crate::storage::traits::{MockLayerRepository, MockMountEntryRepository};
    use chrono::Utc;
    use std::path::PathBuf;

    fn create_mock_layer(layer_id: Uuid, mount_entry_id: Uuid, is_working: bool) -> Layer {
        Layer {
            layer_id,
            tenant_id: Uuid::new_v4(),
            parent_layer_id: None,
            layer_name: "test".to_string(),
            description: None,
            file_count: 0,
            total_size: 0,
            status: LayerStatus::Active,
            is_readonly: false,
            tags: None,
            created_at: Utc::now(),
            created_by: "system".to_string(),
            mount_entry_id: Some(mount_entry_id),
            is_working,
        }
    }

    fn create_mock_mount(tenant_id: Uuid, name: &str, mount_id: Uuid) -> MountEntry {
        MountEntry {
            mount_entry_id: mount_id,
            tenant_id,
            name: name.to_string(),
            virtual_path: PathBuf::from(format!("/{}", name)),
            source: MountSource::WorkingLayer,
            mode: MountMode::ReadWrite,
            is_file: false,
            enabled: true,
            current_layer_id: Some(Uuid::new_v4()),
            metadata: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_initialize_layer_chain() {
        let tenant_id = Uuid::new_v4();
        let mount_id = Uuid::new_v4();

        let mut mock_layer_repo = MockLayerRepository::new();
        mock_layer_repo.expect_create_initial_layers().times(1).returning(move |_, _| {
            let base = create_mock_layer(Uuid::new_v4(), mount_id, false);
            let working = create_mock_layer(Uuid::new_v4(), mount_id, true);
            Ok((base, working))
        });

        let mock_mount_repo = MockMountEntryRepository::new();

        let manager = LayerChainManager::new(Arc::new(mock_layer_repo), Arc::new(mock_mount_repo));

        let result = manager.initialize_layer_chain(tenant_id, mount_id).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_layer_chain() {
        let tenant_id = Uuid::new_v4();
        let mount_id = Uuid::new_v4();
        let mount_name = "memory";

        let mut mock_layer_repo = MockLayerRepository::new();
        let base_layer = create_mock_layer(Uuid::new_v4(), mount_id, false);
        let working_layer = create_mock_layer(Uuid::new_v4(), mount_id, true);
        let layers = vec![base_layer.clone(), working_layer.clone()];

        mock_layer_repo.expect_get_mount_layers().times(1).returning(move |_| Ok(layers.clone()));

        let mut mock_mount_repo = MockMountEntryRepository::new();
        mock_mount_repo
            .expect_get_mount_entry()
            .times(1)
            .returning(move |_| Ok(Some(create_mock_mount(tenant_id, mount_name, mount_id))));

        let manager = LayerChainManager::new(Arc::new(mock_layer_repo), Arc::new(mock_mount_repo));

        let result = manager.get_layer_chain(mount_id).await;
        assert!(result.is_ok());
        let chain = result.unwrap();
        assert_eq!(chain.mount_name, mount_name);
        assert_eq!(chain.layers.len(), 2);
        assert!(chain.working_layer.is_working);
    }

    #[tokio::test]
    async fn test_get_layer_chain_mount_not_found() {
        let mount_id = Uuid::new_v4();

        let mock_layer_repo = MockLayerRepository::new();
        let mut mock_mount_repo = MockMountEntryRepository::new();
        mock_mount_repo.expect_get_mount_entry().times(1).returning(|_| Ok(None));

        let manager = LayerChainManager::new(Arc::new(mock_layer_repo), Arc::new(mock_mount_repo));

        let result = manager.get_layer_chain(mount_id).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_snapshot() {
        let tenant_id = Uuid::new_v4();
        let mount_id = Uuid::new_v4();
        let mount_name = "memory";

        let mut mock_layer_repo = MockLayerRepository::new();
        mock_layer_repo
            .expect_create_snapshot()
            .times(1)
            .returning(move |_, _, _| Ok(create_mock_layer(Uuid::new_v4(), mount_id, true)));

        let mut mock_mount_repo = MockMountEntryRepository::new();
        mock_mount_repo
            .expect_get_mount_entry_by_name()
            .times(1)
            .returning(move |_, _| Ok(Some(create_mock_mount(tenant_id, mount_name, mount_id))));

        let manager = LayerChainManager::new(Arc::new(mock_layer_repo), Arc::new(mock_mount_repo));

        let result = manager.snapshot(tenant_id, mount_name, "snap1", None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_snapshot_mount_not_found() {
        let tenant_id = Uuid::new_v4();

        let mock_layer_repo = MockLayerRepository::new();
        let mut mock_mount_repo = MockMountEntryRepository::new();
        mock_mount_repo.expect_get_mount_entry_by_name().times(1).returning(|_, _| Ok(None));

        let manager = LayerChainManager::new(Arc::new(mock_layer_repo), Arc::new(mock_mount_repo));

        let result = manager.snapshot(tenant_id, "nonexistent", "snap1", None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_has_changes_with_changes() {
        let mount_id = Uuid::new_v4();

        let mut mock_layer_repo = MockLayerRepository::new();
        mock_layer_repo.expect_get_working_layer().times(1).returning(move |_| {
            let mut layer = create_mock_layer(Uuid::new_v4(), mount_id, true);
            layer.file_count = 5;
            Ok(Some(layer))
        });

        let mock_mount_repo = MockMountEntryRepository::new();

        let manager = LayerChainManager::new(Arc::new(mock_layer_repo), Arc::new(mock_mount_repo));

        let result = manager.has_changes(mount_id).await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_has_changes_no_changes() {
        let mount_id = Uuid::new_v4();

        let mut mock_layer_repo = MockLayerRepository::new();
        mock_layer_repo.expect_get_working_layer().times(1).returning(move |_| {
            let layer = create_mock_layer(Uuid::new_v4(), mount_id, true);
            Ok(Some(layer))
        });

        let mock_mount_repo = MockMountEntryRepository::new();

        let manager = LayerChainManager::new(Arc::new(mock_layer_repo), Arc::new(mock_mount_repo));

        let result = manager.has_changes(mount_id).await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }
}

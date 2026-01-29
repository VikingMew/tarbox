use anyhow::Result;
use async_trait::async_trait;
#[cfg(any(test, feature = "mockall"))]
use mockall::automock;

use crate::types::{BlockId, InodeId, LayerId, TenantId};
use chrono::{DateTime, Utc};

use super::models::{
    AuditLog, AuditStats, CreateAuditLogInput, CreateBlockInput, CreateInodeInput,
    CreateLayerEntryInput, CreateLayerInput, CreateTenantInput, CreateTextBlockInput,
    CreateTextMetadataInput, DataBlock, Inode, Layer, LayerEntry, QueryAuditLogsInput, Tenant,
    TextBlock, TextFileMetadata, TextLineMap, UpdateInodeInput,
};

#[cfg_attr(any(test, feature = "mockall"), automock)]
#[async_trait]
pub trait TenantRepository: Send + Sync {
    async fn create(&self, input: CreateTenantInput) -> Result<Tenant>;
    async fn get_by_id(&self, tenant_id: TenantId) -> Result<Option<Tenant>>;
    async fn get_by_name(&self, tenant_name: &str) -> Result<Option<Tenant>>;
    async fn list(&self) -> Result<Vec<Tenant>>;
    async fn delete(&self, tenant_id: TenantId) -> Result<bool>;
}

#[cfg_attr(any(test, feature = "mockall"), automock)]
#[async_trait]
pub trait InodeRepository: Send + Sync {
    async fn create(&self, input: CreateInodeInput) -> Result<Inode>;
    async fn get(&self, tenant_id: TenantId, inode_id: InodeId) -> Result<Option<Inode>>;
    async fn get_by_parent_and_name(
        &self,
        tenant_id: TenantId,
        parent_id: InodeId,
        name: &str,
    ) -> Result<Option<Inode>>;
    async fn update(
        &self,
        tenant_id: TenantId,
        inode_id: InodeId,
        input: UpdateInodeInput,
    ) -> Result<Inode>;
    async fn delete(&self, tenant_id: TenantId, inode_id: InodeId) -> Result<bool>;
    async fn list_children(&self, tenant_id: TenantId, parent_id: InodeId) -> Result<Vec<Inode>>;
}

#[cfg_attr(any(test, feature = "mockall"), automock)]
#[async_trait]
pub trait BlockRepository: Send + Sync {
    async fn create(&self, input: CreateBlockInput) -> Result<DataBlock>;
    async fn get(
        &self,
        tenant_id: TenantId,
        inode_id: InodeId,
        block_index: i32,
    ) -> Result<Option<DataBlock>>;
    async fn list(&self, tenant_id: TenantId, inode_id: InodeId) -> Result<Vec<DataBlock>>;
    async fn delete(&self, tenant_id: TenantId, inode_id: InodeId) -> Result<u64>;
}

#[cfg_attr(any(test, feature = "mockall"), automock)]
#[async_trait]
pub trait AuditLogRepository: Send + Sync {
    async fn create(&self, input: CreateAuditLogInput) -> Result<AuditLog>;
    async fn batch_create(&self, inputs: Vec<CreateAuditLogInput>) -> Result<u64>;
    async fn query(&self, input: QueryAuditLogsInput) -> Result<Vec<AuditLog>>;
    async fn aggregate_stats(
        &self,
        tenant_id: TenantId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<AuditStats>;
}

#[cfg_attr(any(test, feature = "mockall"), automock)]
#[async_trait]
pub trait LayerRepository: Send + Sync {
    async fn create(&self, input: CreateLayerInput) -> Result<Layer>;
    async fn get(&self, tenant_id: TenantId, layer_id: LayerId) -> Result<Option<Layer>>;
    async fn list(&self, tenant_id: TenantId) -> Result<Vec<Layer>>;
    async fn get_layer_chain(&self, tenant_id: TenantId, layer_id: LayerId) -> Result<Vec<Layer>>;
    async fn delete(&self, tenant_id: TenantId, layer_id: LayerId) -> Result<bool>;

    async fn add_entry(&self, input: CreateLayerEntryInput) -> Result<LayerEntry>;
    async fn list_entries(&self, tenant_id: TenantId, layer_id: LayerId)
    -> Result<Vec<LayerEntry>>;

    async fn get_current_layer(&self, tenant_id: TenantId) -> Result<Option<LayerId>>;
    async fn set_current_layer(&self, tenant_id: TenantId, layer_id: LayerId) -> Result<()>;

    // Mount-level layer chains (Task 21)
    async fn create_initial_layers(
        &self,
        tenant_id: uuid::Uuid,
        mount_entry_id: uuid::Uuid,
    ) -> Result<(Layer, Layer)>;

    async fn get_mount_layers(&self, mount_entry_id: uuid::Uuid) -> Result<Vec<Layer>>;

    async fn get_working_layer(&self, mount_entry_id: uuid::Uuid) -> Result<Option<Layer>>;

    async fn create_snapshot(
        &self,
        mount_entry_id: uuid::Uuid,
        name: &str,
        description: Option<String>,
    ) -> Result<Layer>;

    async fn batch_snapshot(
        &self,
        tenant_id: uuid::Uuid,
        mount_names: &[String],
        name: &str,
        skip_unchanged: bool,
    ) -> Result<Vec<crate::composition::SnapshotResult>>;
}

#[cfg_attr(any(test, feature = "mockall"), automock)]
#[async_trait]
pub trait TextBlockRepository: Send + Sync {
    async fn create_block(&self, input: CreateTextBlockInput) -> Result<TextBlock>;
    async fn get_block(&self, block_id: BlockId) -> Result<Option<TextBlock>>;
    async fn get_block_by_hash(&self, content_hash: &str) -> Result<Option<TextBlock>>;
    async fn increment_ref_count(&self, block_id: BlockId) -> Result<()>;
    async fn decrement_ref_count(&self, block_id: BlockId) -> Result<i32>;

    async fn create_metadata(&self, input: CreateTextMetadataInput) -> Result<TextFileMetadata>;
    async fn get_metadata(
        &self,
        tenant_id: TenantId,
        inode_id: InodeId,
        layer_id: LayerId,
    ) -> Result<Option<TextFileMetadata>>;

    async fn create_line_mappings(
        &self,
        tenant_id: TenantId,
        inode_id: InodeId,
        layer_id: LayerId,
        mappings: Vec<(i32, BlockId, i32)>,
    ) -> Result<u64>;
    async fn get_line_mappings(
        &self,
        tenant_id: TenantId,
        inode_id: InodeId,
        layer_id: LayerId,
    ) -> Result<Vec<TextLineMap>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn test_create_tenant_input_construction() {
        let input = CreateTenantInput { tenant_name: "test-tenant".to_string() };
        assert_eq!(input.tenant_name, "test-tenant");
    }

    #[test]
    fn test_create_inode_input_construction() {
        let tenant_id = Uuid::new_v4();
        let input = CreateInodeInput {
            tenant_id,
            parent_id: Some(1),
            name: "test.txt".to_string(),
            inode_type: super::super::models::InodeType::File,
            mode: 0o644,
            uid: 1000,
            gid: 1000,
        };
        assert_eq!(input.name, "test.txt");
        assert_eq!(input.mode, 0o644);
    }

    #[test]
    fn test_update_inode_input_all_none() {
        let input = UpdateInodeInput {
            size: None,
            mode: None,
            uid: None,
            gid: None,
            atime: None,
            mtime: None,
            ctime: None,
        };
        assert!(input.size.is_none());
        assert!(input.mode.is_none());
        assert!(input.uid.is_none());
        assert!(input.gid.is_none());
    }

    #[test]
    fn test_update_inode_input_partial() {
        let input = UpdateInodeInput {
            size: Some(2048),
            mode: Some(0o755),
            uid: None,
            gid: None,
            atime: None,
            mtime: None,
            ctime: None,
        };
        assert_eq!(input.size, Some(2048));
        assert_eq!(input.mode, Some(0o755));
        assert!(input.uid.is_none());
    }

    #[test]
    fn test_create_block_input_construction() {
        let tenant_id = Uuid::new_v4();
        let input =
            CreateBlockInput { tenant_id, inode_id: 123, block_index: 0, data: vec![1, 2, 3, 4] };
        assert_eq!(input.block_index, 0);
        assert_eq!(input.data.len(), 4);
        assert_eq!(input.inode_id, 123);
    }

    #[test]
    fn test_tenant_struct_fields() {
        let tenant_id = Uuid::new_v4();
        let now = Utc::now();
        let tenant = Tenant {
            tenant_id,
            tenant_name: "test".to_string(),
            root_inode_id: 1,
            created_at: now,
            updated_at: now,
        };
        assert_eq!(tenant.root_inode_id, 1);
        assert_eq!(tenant.tenant_name, "test");
    }

    #[test]
    fn test_inode_struct_construction() {
        let tenant_id = Uuid::new_v4();
        let now = Utc::now();
        let inode = Inode {
            inode_id: 42,
            tenant_id,
            parent_id: Some(1),
            name: "file.txt".to_string(),
            inode_type: super::super::models::InodeType::File,
            mode: 0o644,
            uid: 1000,
            gid: 1000,
            size: 1024,
            atime: now,
            mtime: now,
            ctime: now,
        };
        assert_eq!(inode.inode_id, 42);
        assert_eq!(inode.size, 1024);
    }

    #[test]
    fn test_datablock_struct_construction() {
        let tenant_id = Uuid::new_v4();
        let block_id = Uuid::new_v4();
        let now = Utc::now();
        let data = vec![0u8; 4096];
        let block = DataBlock {
            block_id,
            tenant_id,
            inode_id: 100,
            block_index: 0,
            size: data.len() as i32,
            data,
            content_hash: "hash123".to_string(),
            created_at: now,
        };
        assert_eq!(block.inode_id, 100);
        assert_eq!(block.data.len(), 4096);
        assert_eq!(block.size, 4096);
    }
}

// ============================================================================
// Mount Entry Repository (Filesystem Composition - Task 19)
// ============================================================================

use super::models::mount_entry::{CreateMountEntry, MountEntry, UpdateMountEntry};
use super::models::published_mount::{
    PublishMountInput, PublishedMount, PublishedMountFilter, ResolvedPublished, UpdatePublishInput,
};
use std::path::Path;
use uuid::Uuid;

#[cfg_attr(any(test, feature = "mockall"), automock)]
#[async_trait]
pub trait MountEntryRepository: Send + Sync {
    /// Create a new mount entry
    async fn create_mount_entry(
        &self,
        tenant_id: Uuid,
        input: CreateMountEntry,
    ) -> Result<MountEntry>;

    /// Get a mount entry by ID
    async fn get_mount_entry(&self, mount_entry_id: Uuid) -> Result<Option<MountEntry>>;

    /// Get a mount entry by tenant and name
    async fn get_mount_entry_by_name(
        &self,
        tenant_id: Uuid,
        name: &str,
    ) -> Result<Option<MountEntry>>;

    /// Get a mount entry by tenant and path (exact match)
    async fn get_mount_entry_by_path(
        &self,
        tenant_id: Uuid,
        path: &Path,
    ) -> Result<Option<MountEntry>>;

    /// List all mount entries for a tenant
    async fn list_mount_entries(&self, tenant_id: Uuid) -> Result<Vec<MountEntry>>;

    /// Update a mount entry
    async fn update_mount_entry(
        &self,
        mount_entry_id: Uuid,
        input: UpdateMountEntry,
    ) -> Result<MountEntry>;

    /// Delete a mount entry
    async fn delete_mount_entry(&self, mount_entry_id: Uuid) -> Result<bool>;

    /// Batch set mount entries (replace all for a tenant)
    async fn set_mount_entries(
        &self,
        tenant_id: Uuid,
        entries: Vec<CreateMountEntry>,
    ) -> Result<Vec<MountEntry>>;

    /// Check if a path conflicts with existing mounts
    async fn check_path_conflict(
        &self,
        tenant_id: Uuid,
        path: &Path,
        is_file: bool,
        exclude_id: Option<Uuid>,
    ) -> Result<bool>;
}

// ============================================================================
// Published Mount Repository (Filesystem Composition - Task 20)
// ============================================================================

#[cfg_attr(any(test, feature = "mockall"), automock)]
#[async_trait]
pub trait PublishedMountRepository: Send + Sync {
    /// Publish a mount
    async fn publish_mount(&self, input: PublishMountInput) -> Result<PublishedMount>;

    /// Unpublish a mount
    async fn unpublish_mount(&self, mount_entry_id: Uuid) -> Result<bool>;

    /// Get published mount by name
    async fn get_published_by_name(&self, publish_name: &str) -> Result<Option<PublishedMount>>;

    /// Get publish info for a mount entry
    async fn get_publish_info(&self, mount_entry_id: Uuid) -> Result<Option<PublishedMount>>;

    /// List published mounts (global)
    async fn list_published_mounts(
        &self,
        filter: PublishedMountFilter,
    ) -> Result<Vec<PublishedMount>>;

    /// List published mounts for a tenant
    async fn list_tenant_published_mounts(&self, tenant_id: Uuid) -> Result<Vec<PublishedMount>>;

    /// Update publish information
    async fn update_publish(
        &self,
        publish_id: Uuid,
        input: UpdatePublishInput,
    ) -> Result<PublishedMount>;

    /// Check if a tenant has access to a published mount
    async fn check_access(&self, publish_name: &str, accessor_tenant_id: Uuid) -> Result<bool>;

    /// Add a tenant to the allow list
    async fn add_allowed_tenant(&self, publish_id: Uuid, tenant_id: Uuid) -> Result<()>;

    /// Remove a tenant from the allow list
    async fn remove_allowed_tenant(&self, publish_id: Uuid, tenant_id: Uuid) -> Result<()>;

    /// Resolve published mount to actual layer (for working_layer, returns current working layer)
    async fn resolve_published(
        &self,
        publish_name: &str,
        accessor_tenant_id: Uuid,
    ) -> Result<ResolvedPublished>;
}

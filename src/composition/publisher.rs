use anyhow::{Result, anyhow};
use std::sync::Arc;
use uuid::Uuid;

use crate::storage::models::mount_entry::MountSource;
use crate::storage::models::published_mount::{
    PublishMountInput, PublishedMount, ResolvedPublished,
};
use crate::storage::traits::{MountEntryRepository, PublishedMountRepository};

/// Layer publisher service
pub struct LayerPublisher {
    pub published_mount_repo: Arc<dyn PublishedMountRepository>,
    pub mount_entry_repo: Arc<dyn MountEntryRepository>,
}

impl LayerPublisher {
    pub fn new(
        published_mount_repo: Arc<dyn PublishedMountRepository>,
        mount_entry_repo: Arc<dyn MountEntryRepository>,
    ) -> Self {
        Self { published_mount_repo, mount_entry_repo }
    }

    /// Publish a mount point
    ///
    /// Validates:
    /// - Mount point exists and belongs to the caller
    /// - Mount point is WorkingLayer type
    /// - Publish name is globally unique
    pub async fn publish(
        &self,
        tenant_id: Uuid,
        mount_name: &str,
        input: PublishMountInput,
    ) -> Result<PublishedMount> {
        // Get mount entry
        let mount = self
            .mount_entry_repo
            .get_mount_entry_by_name(tenant_id, mount_name)
            .await?
            .ok_or_else(|| anyhow!("Mount '{}' not found", mount_name))?;

        // Verify ownership
        if mount.tenant_id != tenant_id {
            return Err(anyhow!("Mount does not belong to this tenant"));
        }

        // Verify mount_entry_id matches
        if mount.mount_entry_id != input.mount_entry_id {
            return Err(anyhow!("Mount entry ID mismatch"));
        }

        // Verify it's a WorkingLayer mount
        if !matches!(mount.source, MountSource::WorkingLayer) {
            return Err(anyhow!(
                "Only WorkingLayer mounts can be published (mount '{}' has different source type)",
                mount_name
            ));
        }

        // Publish via repository
        self.published_mount_repo.publish_mount(input).await
    }

    /// Unpublish a mount point
    pub async fn unpublish(&self, tenant_id: Uuid, mount_name: &str) -> Result<()> {
        // Get mount entry
        let mount = self
            .mount_entry_repo
            .get_mount_entry_by_name(tenant_id, mount_name)
            .await?
            .ok_or_else(|| anyhow!("Mount '{}' not found", mount_name))?;

        // Verify ownership
        if mount.tenant_id != tenant_id {
            return Err(anyhow!("Mount does not belong to this tenant"));
        }

        // Unpublish
        let deleted = self.published_mount_repo.unpublish_mount(mount.mount_entry_id).await?;

        if !deleted {
            return Err(anyhow!("Mount '{}' is not published", mount_name));
        }

        Ok(())
    }

    /// Resolve a published mount to actual layer
    ///
    /// For working_layer type, returns the current working layer ID
    pub async fn resolve_published(
        &self,
        publish_name: &str,
        accessor_tenant_id: Uuid,
    ) -> Result<ResolvedPublished> {
        self.published_mount_repo.resolve_published(publish_name, accessor_tenant_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::mount_entry::MountMode;
    use crate::storage::models::published_mount::{PublishScope, PublishTarget};
    use crate::storage::traits::{MockMountEntryRepository, MockPublishedMountRepository};
    use chrono::Utc;
    use std::path::PathBuf;

    fn create_mock_mount(
        tenant_id: Uuid,
        name: &str,
        mount_id: Uuid,
    ) -> crate::storage::models::mount_entry::MountEntry {
        crate::storage::models::mount_entry::MountEntry {
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
    async fn test_publish_success() {
        let tenant_id = Uuid::new_v4();
        let mount_id = Uuid::new_v4();
        let mount_name = "memory";

        let mut mock_mount_repo = MockMountEntryRepository::new();
        mock_mount_repo
            .expect_get_mount_entry_by_name()
            .times(1)
            .returning(move |_, _| Ok(Some(create_mock_mount(tenant_id, mount_name, mount_id))));

        let mut mock_publish_repo = MockPublishedMountRepository::new();
        mock_publish_repo.expect_publish_mount().times(1).returning(|input| {
            Ok(PublishedMount {
                publish_id: Uuid::new_v4(),
                mount_entry_id: input.mount_entry_id,
                tenant_id: Uuid::new_v4(),
                publish_name: input.publish_name,
                description: input.description,
                target: input.target,
                scope: input.scope,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            })
        });

        let publisher = LayerPublisher::new(Arc::new(mock_publish_repo), Arc::new(mock_mount_repo));

        let input = PublishMountInput {
            mount_entry_id: mount_id,
            publish_name: "test-publish".to_string(),
            description: None,
            target: PublishTarget::WorkingLayer,
            scope: PublishScope::Public,
        };

        let result = publisher.publish(tenant_id, mount_name, input).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_publish_mount_not_found() {
        let tenant_id = Uuid::new_v4();
        let mount_id = Uuid::new_v4();

        let mut mock_mount_repo = MockMountEntryRepository::new();
        mock_mount_repo.expect_get_mount_entry_by_name().times(1).returning(|_, _| Ok(None));

        let mock_publish_repo = MockPublishedMountRepository::new();

        let publisher = LayerPublisher::new(Arc::new(mock_publish_repo), Arc::new(mock_mount_repo));

        let input = PublishMountInput {
            mount_entry_id: mount_id,
            publish_name: "test-publish".to_string(),
            description: None,
            target: PublishTarget::WorkingLayer,
            scope: PublishScope::Public,
        };

        let result = publisher.publish(tenant_id, "nonexistent", input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_publish_wrong_tenant() {
        let tenant_id = Uuid::new_v4();
        let wrong_tenant = Uuid::new_v4();
        let mount_id = Uuid::new_v4();
        let mount_name = "memory";

        let mut mock_mount_repo = MockMountEntryRepository::new();
        mock_mount_repo
            .expect_get_mount_entry_by_name()
            .times(1)
            .returning(move |_, _| Ok(Some(create_mock_mount(wrong_tenant, mount_name, mount_id))));

        let mock_publish_repo = MockPublishedMountRepository::new();

        let publisher = LayerPublisher::new(Arc::new(mock_publish_repo), Arc::new(mock_mount_repo));

        let input = PublishMountInput {
            mount_entry_id: mount_id,
            publish_name: "test-publish".to_string(),
            description: None,
            target: PublishTarget::WorkingLayer,
            scope: PublishScope::Public,
        };

        let result = publisher.publish(tenant_id, mount_name, input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not belong"));
    }

    #[tokio::test]
    async fn test_unpublish_success() {
        let tenant_id = Uuid::new_v4();
        let mount_id = Uuid::new_v4();
        let mount_name = "memory";

        let mut mock_mount_repo = MockMountEntryRepository::new();
        mock_mount_repo
            .expect_get_mount_entry_by_name()
            .times(1)
            .returning(move |_, _| Ok(Some(create_mock_mount(tenant_id, mount_name, mount_id))));

        let mut mock_publish_repo = MockPublishedMountRepository::new();
        mock_publish_repo.expect_unpublish_mount().times(1).returning(|_| Ok(true));

        let publisher = LayerPublisher::new(Arc::new(mock_publish_repo), Arc::new(mock_mount_repo));

        let result = publisher.unpublish(tenant_id, mount_name).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_unpublish_not_published() {
        let tenant_id = Uuid::new_v4();
        let mount_id = Uuid::new_v4();
        let mount_name = "memory";

        let mut mock_mount_repo = MockMountEntryRepository::new();
        mock_mount_repo
            .expect_get_mount_entry_by_name()
            .times(1)
            .returning(move |_, _| Ok(Some(create_mock_mount(tenant_id, mount_name, mount_id))));

        let mut mock_publish_repo = MockPublishedMountRepository::new();
        mock_publish_repo.expect_unpublish_mount().times(1).returning(|_| Ok(false));

        let publisher = LayerPublisher::new(Arc::new(mock_publish_repo), Arc::new(mock_mount_repo));

        let result = publisher.unpublish(tenant_id, mount_name).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not published"));
    }

    #[tokio::test]
    async fn test_resolve_published() {
        let accessor = Uuid::new_v4();
        let publish_name = "test-publish";

        let mut mock_publish_repo = MockPublishedMountRepository::new();
        mock_publish_repo.expect_resolve_published().times(1).returning(|_, _| {
            Ok(ResolvedPublished {
                mount_entry_id: Uuid::new_v4(),
                owner_tenant_id: Uuid::new_v4(),
                layer_id: Uuid::new_v4(),
                is_working_layer: true,
            })
        });

        let mock_mount_repo = MockMountEntryRepository::new();

        let publisher = LayerPublisher::new(Arc::new(mock_publish_repo), Arc::new(mock_mount_repo));

        let result = publisher.resolve_published(publish_name, accessor).await;
        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert!(resolved.is_working_layer);
    }
}

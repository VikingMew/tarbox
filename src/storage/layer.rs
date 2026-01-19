use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::types::{LayerId, TenantId};

use super::models::{CreateLayerEntryInput, CreateLayerInput, Layer, LayerEntry, LayerStatus};
use super::traits::LayerRepository;

pub struct LayerOperations<'a> {
    pool: &'a PgPool,
}

impl<'a> LayerOperations<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl<'a> LayerRepository for LayerOperations<'a> {
    async fn create(&self, input: CreateLayerInput) -> Result<Layer> {
        let layer_id = Uuid::new_v4();

        let layer = sqlx::query_as::<_, Layer>(
            r#"
            INSERT INTO layers (
                layer_id, tenant_id, parent_layer_id, layer_name, description,
                status, is_readonly, tags, created_by
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING layer_id, tenant_id, parent_layer_id, layer_name, description,
                      file_count, total_size, status, is_readonly, tags,
                      created_at, created_by
            "#,
        )
        .bind(layer_id)
        .bind(input.tenant_id)
        .bind(input.parent_layer_id)
        .bind(&input.layer_name)
        .bind(&input.description)
        .bind(LayerStatus::Active)
        .bind(false) // is_readonly
        .bind(&input.tags)
        .bind(&input.created_by)
        .fetch_one(self.pool)
        .await?;

        tracing::info!(
            layer_id = %layer_id,
            tenant_id = %input.tenant_id,
            layer_name = %input.layer_name,
            "Created new layer"
        );

        Ok(layer)
    }

    async fn get(&self, tenant_id: TenantId, layer_id: LayerId) -> Result<Option<Layer>> {
        let layer = sqlx::query_as::<_, Layer>(
            r#"
            SELECT layer_id, tenant_id, parent_layer_id, layer_name, description,
                   file_count, total_size, status, is_readonly, tags,
                   created_at, created_by
            FROM layers
            WHERE tenant_id = $1 AND layer_id = $2
            "#,
        )
        .bind(tenant_id)
        .bind(layer_id)
        .fetch_optional(self.pool)
        .await?;

        Ok(layer)
    }

    async fn list(&self, tenant_id: TenantId) -> Result<Vec<Layer>> {
        let layers = sqlx::query_as::<_, Layer>(
            r#"
            SELECT layer_id, tenant_id, parent_layer_id, layer_name, description,
                   file_count, total_size, status, is_readonly, tags,
                   created_at, created_by
            FROM layers
            WHERE tenant_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(tenant_id)
        .fetch_all(self.pool)
        .await?;

        Ok(layers)
    }

    async fn get_layer_chain(&self, tenant_id: TenantId, layer_id: LayerId) -> Result<Vec<Layer>> {
        let layers = sqlx::query_as::<_, Layer>(
            r#"
            WITH RECURSIVE layer_chain AS (
                SELECT layer_id, tenant_id, parent_layer_id, layer_name, description,
                       file_count, total_size, status, is_readonly, tags,
                       created_at, created_by, 0 as depth
                FROM layers
                WHERE layer_id = $2 AND tenant_id = $1

                UNION ALL

                SELECT l.layer_id, l.tenant_id, l.parent_layer_id, l.layer_name, l.description,
                       l.file_count, l.total_size, l.status, l.is_readonly, l.tags,
                       l.created_at, l.created_by, lc.depth + 1
                FROM layers l
                INNER JOIN layer_chain lc ON l.layer_id = lc.parent_layer_id
                WHERE l.tenant_id = $1
            )
            SELECT layer_id, tenant_id, parent_layer_id, layer_name, description,
                   file_count, total_size, status, is_readonly, tags,
                   created_at, created_by
            FROM layer_chain
            ORDER BY depth
            "#,
        )
        .bind(tenant_id)
        .bind(layer_id)
        .fetch_all(self.pool)
        .await?;

        Ok(layers)
    }

    async fn delete(&self, tenant_id: TenantId, layer_id: LayerId) -> Result<bool> {
        let result = sqlx::query(
            r#"
            DELETE FROM layers
            WHERE tenant_id = $1 AND layer_id = $2
            "#,
        )
        .bind(tenant_id)
        .bind(layer_id)
        .execute(self.pool)
        .await?;

        let deleted = result.rows_affected() > 0;

        if deleted {
            tracing::info!(
                layer_id = %layer_id,
                tenant_id = %tenant_id,
                "Deleted layer"
            );
        }

        Ok(deleted)
    }

    async fn add_entry(&self, input: CreateLayerEntryInput) -> Result<LayerEntry> {
        let entry_id = Uuid::new_v4();

        let entry = sqlx::query_as::<_, LayerEntry>(
            r#"
            INSERT INTO layer_entries (
                entry_id, layer_id, tenant_id, inode_id, path,
                change_type, size_delta, text_changes
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING entry_id, layer_id, tenant_id, inode_id, path,
                      change_type, size_delta, text_changes, created_at
            "#,
        )
        .bind(entry_id)
        .bind(input.layer_id)
        .bind(input.tenant_id)
        .bind(input.inode_id)
        .bind(&input.path)
        .bind(input.change_type)
        .bind(input.size_delta)
        .bind(&input.text_changes)
        .fetch_one(self.pool)
        .await?;

        tracing::debug!(
            entry_id = %entry_id,
            layer_id = %input.layer_id,
            path = %input.path,
            "Added layer entry"
        );

        Ok(entry)
    }

    async fn list_entries(
        &self,
        tenant_id: TenantId,
        layer_id: LayerId,
    ) -> Result<Vec<LayerEntry>> {
        let entries = sqlx::query_as::<_, LayerEntry>(
            r#"
            SELECT entry_id, layer_id, tenant_id, inode_id, path,
                   change_type, size_delta, text_changes, created_at
            FROM layer_entries
            WHERE tenant_id = $1 AND layer_id = $2
            ORDER BY created_at
            "#,
        )
        .bind(tenant_id)
        .bind(layer_id)
        .fetch_all(self.pool)
        .await?;

        Ok(entries)
    }

    async fn get_current_layer(&self, tenant_id: TenantId) -> Result<Option<LayerId>> {
        let layer_id = sqlx::query_as::<_, (LayerId,)>(
            r#"
            SELECT current_layer_id
            FROM tenant_current_layer
            WHERE tenant_id = $1
            "#,
        )
        .bind(tenant_id)
        .fetch_optional(self.pool)
        .await?
        .map(|row| row.0);

        Ok(layer_id)
    }

    async fn set_current_layer(&self, tenant_id: TenantId, layer_id: LayerId) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO tenant_current_layer (tenant_id, current_layer_id)
            VALUES ($1, $2)
            ON CONFLICT (tenant_id)
            DO UPDATE SET current_layer_id = $2, updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(tenant_id)
        .bind(layer_id)
        .execute(self.pool)
        .await?;

        tracing::info!(
            tenant_id = %tenant_id,
            layer_id = %layer_id,
            "Set current layer for tenant"
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{ChangeType, LayerStatus};

    #[test]
    fn test_layer_status_variants() {
        let active = LayerStatus::Active;
        let creating = LayerStatus::Creating;
        let deleting = LayerStatus::Deleting;
        let archived = LayerStatus::Archived;

        assert_eq!(format!("{:?}", active), "Active");
        assert_eq!(format!("{:?}", creating), "Creating");
        assert_eq!(format!("{:?}", deleting), "Deleting");
        assert_eq!(format!("{:?}", archived), "Archived");
    }

    #[test]
    fn test_change_type_variants() {
        let add = ChangeType::Add;
        let modify = ChangeType::Modify;
        let delete = ChangeType::Delete;

        assert_eq!(format!("{:?}", add), "Add");
        assert_eq!(format!("{:?}", modify), "Modify");
        assert_eq!(format!("{:?}", delete), "Delete");
    }

    #[test]
    fn test_create_layer_input_validation() {
        let tenant_id = uuid::Uuid::new_v4();
        let input = CreateLayerInput {
            tenant_id,
            parent_layer_id: None,
            layer_name: "base".to_string(),
            description: Some("Base layer".to_string()),
            created_by: "system".to_string(),
            tags: None,
        };

        assert_eq!(input.layer_name, "base");
        assert_eq!(input.created_by, "system");
        assert!(input.parent_layer_id.is_none());
        assert!(input.description.is_some());
    }

    #[test]
    fn test_create_layer_entry_input() {
        let tenant_id = uuid::Uuid::new_v4();
        let layer_id = uuid::Uuid::new_v4();
        let inode_id = 123i64;

        let input = CreateLayerEntryInput {
            layer_id,
            tenant_id,
            inode_id,
            path: "/test.txt".to_string(),
            change_type: ChangeType::Add,
            size_delta: Some(1024),
            text_changes: None,
        };

        assert_eq!(input.path, "/test.txt");
        assert_eq!(input.size_delta.unwrap(), 1024);
        assert!(matches!(input.change_type, ChangeType::Add));
    }

    #[test]
    fn test_layer_entry_change_types() {
        let add_entry = CreateLayerEntryInput {
            layer_id: uuid::Uuid::new_v4(),
            tenant_id: uuid::Uuid::new_v4(),
            inode_id: 1,
            path: "/new.txt".to_string(),
            change_type: ChangeType::Add,
            size_delta: Some(100),
            text_changes: None,
        };

        let modify_entry =
            CreateLayerEntryInput { change_type: ChangeType::Modify, ..add_entry.clone() };

        let delete_entry = CreateLayerEntryInput {
            change_type: ChangeType::Delete,
            size_delta: Some(-100),
            ..add_entry.clone()
        };

        assert!(matches!(add_entry.change_type, ChangeType::Add));
        assert!(matches!(modify_entry.change_type, ChangeType::Modify));
        assert!(matches!(delete_entry.change_type, ChangeType::Delete));
    }
}

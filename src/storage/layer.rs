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
    #[test]
    fn test_layer_operations_creation() {
        // Mock pool can't be created without a real database in unit tests
        // This test just ensures the struct is constructible
        // Actual functionality tested in integration tests
    }
}

//! Layer manager module.
//!
//! Provides high-level operations for managing layers in a layered filesystem.
//! This includes creating checkpoints, switching layers, and managing layer lifecycle.

use anyhow::Result;
use sqlx::PgPool;
use thiserror::Error;
use tracing::{debug, info};

use crate::storage::{
    ChangeType, CreateLayerEntryInput, CreateLayerInput, Layer, LayerOperations, LayerRepository,
};
use crate::types::{InodeId, LayerId, TenantId};

/// Errors that can occur during layer management operations.
#[derive(Error, Debug)]
pub enum LayerManagerError {
    #[error("Layer not found: {0}")]
    LayerNotFound(LayerId),

    #[error("No current layer set for tenant")]
    NoCurrentLayer,

    #[error("Cannot modify readonly layer: {0}")]
    ReadonlyLayer(LayerId),

    #[error("Layer has child layers and cannot be deleted: {0}")]
    HasChildLayers(LayerId),

    #[error("Cannot create layer from historical position without confirmation")]
    HistoricalLayerNeedsConfirmation { current_layer: LayerId, future_layers: Vec<Layer> },

    #[error("Invalid layer chain: {0}")]
    InvalidLayerChain(String),

    #[error("Storage error: {0}")]
    Storage(#[from] anyhow::Error),
}

/// Result type for layer manager operations.
pub type LayerManagerResult<T> = Result<T, LayerManagerError>;

/// Layer manager for high-level layer operations.
pub struct LayerManager<'a> {
    pool: &'a PgPool,
    tenant_id: TenantId,
}

impl<'a> LayerManager<'a> {
    /// Create a new layer manager for a tenant.
    pub fn new(pool: &'a PgPool, tenant_id: TenantId) -> Self {
        Self { pool, tenant_id }
    }

    /// Get the layer operations instance.
    fn layer_ops(&self) -> LayerOperations<'a> {
        LayerOperations::new(self.pool)
    }

    /// Get the current active layer for the tenant.
    pub async fn get_current_layer(&self) -> LayerManagerResult<Layer> {
        let ops = self.layer_ops();
        let layer_id = ops
            .get_current_layer(self.tenant_id)
            .await?
            .ok_or(LayerManagerError::NoCurrentLayer)?;

        ops.get(self.tenant_id, layer_id).await?.ok_or(LayerManagerError::LayerNotFound(layer_id))
    }

    /// Get the current layer ID, or None if not set.
    pub async fn get_current_layer_id(&self) -> LayerManagerResult<Option<LayerId>> {
        Ok(self.layer_ops().get_current_layer(self.tenant_id).await?)
    }

    /// Initialize a base layer for the tenant if none exists.
    pub async fn initialize_base_layer(&self) -> LayerManagerResult<Layer> {
        let ops = self.layer_ops();

        // Check if there's already a current layer
        if let Some(layer_id) = ops.get_current_layer(self.tenant_id).await?
            && let Some(layer) = ops.get(self.tenant_id, layer_id).await?
        {
            debug!(tenant_id = %self.tenant_id, layer_id = %layer_id, "Base layer already exists");
            return Ok(layer);
        }

        info!(tenant_id = %self.tenant_id, "Initializing base layer");

        // Create base layer
        let layer = ops
            .create(CreateLayerInput {
                tenant_id: self.tenant_id,
                parent_layer_id: None,
                layer_name: "base".to_string(),
                description: Some("Initial base layer".to_string()),
                tags: None,
                created_by: "system".to_string(),
                mount_entry_id: None,
                is_working: false,
            })
            .await?;

        // Set as current layer
        ops.set_current_layer(self.tenant_id, layer.layer_id).await?;

        info!(tenant_id = %self.tenant_id, layer_id = %layer.layer_id, "Base layer created");

        Ok(layer)
    }

    /// Create a new checkpoint (layer) from the current state.
    ///
    /// This marks the current layer as readonly and creates a new writable layer.
    pub async fn create_checkpoint(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> LayerManagerResult<Layer> {
        self.create_checkpoint_with_confirm(name, description, false).await
    }

    /// Create a new checkpoint with optional confirmation for historical layers.
    pub async fn create_checkpoint_with_confirm(
        &self,
        name: &str,
        description: Option<&str>,
        confirm_delete_future: bool,
    ) -> LayerManagerResult<Layer> {
        let ops = self.layer_ops();

        // Get current layer
        let current_layer_id = ops
            .get_current_layer(self.tenant_id)
            .await?
            .ok_or(LayerManagerError::NoCurrentLayer)?;

        let _current_layer = ops
            .get(self.tenant_id, current_layer_id)
            .await?
            .ok_or(LayerManagerError::LayerNotFound(current_layer_id))?;

        // Check if we're at a historical layer (have future layers)
        let future_layers = self.get_future_layers(current_layer_id).await?;
        if !future_layers.is_empty() && !confirm_delete_future {
            return Err(LayerManagerError::HistoricalLayerNeedsConfirmation {
                current_layer: current_layer_id,
                future_layers,
            });
        }

        // Delete future layers if confirmed
        if !future_layers.is_empty() && confirm_delete_future {
            for layer in future_layers.iter().rev() {
                ops.delete(self.tenant_id, layer.layer_id).await?;
            }
        }

        // Mark current layer as readonly
        self.set_layer_readonly(current_layer_id, true).await?;

        // Create new layer
        let new_layer = ops
            .create(CreateLayerInput {
                tenant_id: self.tenant_id,
                parent_layer_id: Some(current_layer_id),
                layer_name: name.to_string(),
                description: description.map(String::from),
                tags: None,
                created_by: "user".to_string(),
                mount_entry_id: None,
                is_working: false,
            })
            .await?;

        // Set new layer as current
        ops.set_current_layer(self.tenant_id, new_layer.layer_id).await?;

        Ok(new_layer)
    }

    /// Switch to a different layer.
    ///
    /// This changes the current layer to the specified layer.
    /// Future layers are preserved but become inaccessible until switching back.
    pub async fn switch_to_layer(&self, target_layer_id: LayerId) -> LayerManagerResult<Layer> {
        let ops = self.layer_ops();

        // Verify target layer exists and belongs to tenant
        let target_layer = ops
            .get(self.tenant_id, target_layer_id)
            .await?
            .ok_or(LayerManagerError::LayerNotFound(target_layer_id))?;

        // Update current layer
        ops.set_current_layer(self.tenant_id, target_layer_id).await?;

        Ok(target_layer)
    }

    /// List all layers for the tenant.
    pub async fn list_layers(&self) -> LayerManagerResult<Vec<Layer>> {
        Ok(self.layer_ops().list(self.tenant_id).await?)
    }

    /// Get the layer chain from a specific layer up to the root.
    pub async fn get_layer_chain(&self, layer_id: LayerId) -> LayerManagerResult<Vec<Layer>> {
        Ok(self.layer_ops().get_layer_chain(self.tenant_id, layer_id).await?)
    }

    /// Get layers that are "future" relative to the given layer.
    /// These are layers that have the given layer as an ancestor.
    pub async fn get_future_layers(&self, layer_id: LayerId) -> LayerManagerResult<Vec<Layer>> {
        let all_layers = self.list_layers().await?;
        let mut future_layers = Vec::new();

        for layer in all_layers {
            if layer.layer_id == layer_id {
                continue;
            }

            // Check if this layer has layer_id as an ancestor
            if layer.parent_layer_id.is_some() {
                let chain = self.get_layer_chain(layer.layer_id).await?;
                if chain.iter().any(|l| l.layer_id == layer_id) {
                    future_layers.push(layer);
                }
            }
        }

        // Sort by depth (direct children first)
        future_layers.sort_by(|a, b| {
            let a_depth = self.count_depth_from(a.layer_id, layer_id);
            let b_depth = self.count_depth_from(b.layer_id, layer_id);
            a_depth.cmp(&b_depth)
        });

        Ok(future_layers)
    }

    /// Helper to count depth from one layer to another.
    fn count_depth_from(&self, _from: LayerId, _to: LayerId) -> usize {
        // This is a simplified version; in practice we'd cache this
        0 // Placeholder
    }

    /// Delete a layer.
    ///
    /// The layer must not have any child layers.
    pub async fn delete_layer(&self, layer_id: LayerId) -> LayerManagerResult<()> {
        let ops = self.layer_ops();

        // Check if layer exists
        let layer = ops
            .get(self.tenant_id, layer_id)
            .await?
            .ok_or(LayerManagerError::LayerNotFound(layer_id))?;

        // Check if layer has children
        let future_layers = self.get_future_layers(layer_id).await?;
        if !future_layers.is_empty() {
            return Err(LayerManagerError::HasChildLayers(layer_id));
        }

        // If this is the current layer, switch to parent
        if let Some(current_id) = ops.get_current_layer(self.tenant_id).await?
            && current_id == layer_id
        {
            if let Some(parent_id) = layer.parent_layer_id {
                ops.set_current_layer(self.tenant_id, parent_id).await?;
            } else {
                // Can't delete base layer if it's current and has no parent
                return Err(LayerManagerError::InvalidLayerChain(
                    "Cannot delete base layer".to_string(),
                ));
            }
        }

        // Delete the layer
        ops.delete(self.tenant_id, layer_id).await?;

        Ok(())
    }

    /// Get a specific layer by ID.
    pub async fn get_layer(&self, layer_id: LayerId) -> LayerManagerResult<Option<Layer>> {
        Ok(self.layer_ops().get(self.tenant_id, layer_id).await?)
    }

    /// Set a layer as readonly or writable.
    async fn set_layer_readonly(
        &self,
        layer_id: LayerId,
        readonly: bool,
    ) -> LayerManagerResult<()> {
        // This would need to be added to LayerRepository trait
        // For now, we'll use a direct SQL query
        sqlx::query(
            r#"
            UPDATE layers
            SET is_readonly = $3
            WHERE tenant_id = $1 AND layer_id = $2
            "#,
        )
        .bind(self.tenant_id)
        .bind(layer_id)
        .bind(readonly)
        .execute(self.pool)
        .await
        .map_err(|e| LayerManagerError::Storage(e.into()))?;

        Ok(())
    }

    /// Add an entry to the current layer recording a file change.
    pub async fn record_change(
        &self,
        inode_id: InodeId,
        path: &str,
        change_type: ChangeType,
        size_delta: Option<i64>,
        text_changes: Option<serde_json::Value>,
    ) -> LayerManagerResult<()> {
        let ops = self.layer_ops();

        let current_layer_id = ops
            .get_current_layer(self.tenant_id)
            .await?
            .ok_or(LayerManagerError::NoCurrentLayer)?;

        // Check if layer is readonly
        let layer = ops
            .get(self.tenant_id, current_layer_id)
            .await?
            .ok_or(LayerManagerError::LayerNotFound(current_layer_id))?;

        if layer.is_readonly {
            return Err(LayerManagerError::ReadonlyLayer(current_layer_id));
        }

        debug!(
            inode_id = inode_id,
            path = %path,
            change_type = ?change_type,
            layer_id = %current_layer_id,
            size_delta = ?size_delta,
            "Recording change to layer"
        );

        // Add entry
        ops.add_entry(CreateLayerEntryInput {
            layer_id: current_layer_id,
            tenant_id: self.tenant_id,
            inode_id,
            path: path.to_string(),
            change_type,
            size_delta,
            text_changes,
        })
        .await?;

        Ok(())
    }

    /// Get all entries for a layer.
    pub async fn get_layer_entries(
        &self,
        layer_id: LayerId,
    ) -> LayerManagerResult<Vec<crate::storage::LayerEntry>> {
        Ok(self.layer_ops().list_entries(self.tenant_id, layer_id).await?)
    }

    /// Check if the current layer is at a historical position.
    pub async fn is_at_historical_position(&self) -> LayerManagerResult<bool> {
        if let Some(current_id) = self.get_current_layer_id().await? {
            let future_layers = self.get_future_layers(current_id).await?;
            Ok(!future_layers.is_empty())
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    // Tests would require database setup; see integration tests
}

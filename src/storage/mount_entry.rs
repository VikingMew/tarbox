use anyhow::{Result, anyhow};
use async_trait::async_trait;
use chrono::Utc;
use sqlx::PgPool;
use std::path::Path;
use uuid::Uuid;

use super::models::mount_entry::{
    CreateMountEntry, MountEntry, MountMode, MountSource, UpdateMountEntry,
};
use super::traits::MountEntryRepository;
use crate::composition::resolver::{DefaultPathResolver, PathResolver};

pub struct PgMountEntryRepository {
    pool: PgPool,
}

impl PgMountEntryRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Convert database row to MountEntry
    fn row_to_mount_entry(&self, row: &sqlx::postgres::PgRow) -> Result<MountEntry> {
        use sqlx::Row;

        let mount_entry_id: Uuid = row.try_get("mount_entry_id")?;
        let tenant_id: Uuid = row.try_get("tenant_id")?;
        let name: String = row.try_get("name")?;
        let virtual_path: String = row.try_get("virtual_path")?;
        let is_file: bool = row.try_get("is_file")?;
        let source_type: String = row.try_get("source_type")?;
        let mode_str: String = row.try_get("mode")?;
        let enabled: bool = row.try_get("enabled")?;
        let current_layer_id: Option<Uuid> = row.try_get("current_layer_id")?;
        let metadata: Option<serde_json::Value> = row.try_get("metadata")?;
        let created_at = row.try_get("created_at")?;
        let updated_at = row.try_get("updated_at")?;

        // Parse source
        let source = match source_type.as_str() {
            "host" => {
                let host_path: String = row.try_get("host_path")?;
                MountSource::Host { path: host_path.into() }
            }
            "layer" => {
                let source_mount_id: Uuid = row.try_get("source_mount_id")?;
                let layer_id: Option<Uuid> = row.try_get("source_layer_id")?;
                let subpath: Option<String> = row.try_get("source_subpath")?;
                MountSource::Layer { source_mount_id, layer_id, subpath: subpath.map(Into::into) }
            }
            "published" => {
                let publish_name: String = row.try_get("source_publish_name")?;
                let subpath: Option<String> = row.try_get("source_subpath")?;
                MountSource::Published { publish_name, subpath: subpath.map(Into::into) }
            }
            "working_layer" => MountSource::WorkingLayer,
            _ => return Err(anyhow!("Unknown source type: {}", source_type)),
        };

        // Parse mode
        let mode = MountMode::parse_mode(&mode_str)
            .ok_or_else(|| anyhow!("Invalid mode: {}", mode_str))?;

        Ok(MountEntry {
            mount_entry_id,
            tenant_id,
            name,
            virtual_path: virtual_path.into(),
            source,
            mode,
            is_file,
            enabled,
            current_layer_id,
            metadata,
            created_at,
            updated_at,
        })
    }
}

#[async_trait]
impl MountEntryRepository for PgMountEntryRepository {
    async fn create_mount_entry(
        &self,
        tenant_id: Uuid,
        input: CreateMountEntry,
    ) -> Result<MountEntry> {
        // Validate path doesn't conflict
        if self.check_path_conflict(tenant_id, &input.virtual_path, input.is_file, None).await? {
            return Err(anyhow!(
                "MountPathConflict: path '{}' conflicts with existing mounts",
                input.virtual_path.display()
            ));
        }

        let mount_entry_id = Uuid::new_v4();
        let now = Utc::now();

        // Extract source fields
        let (
            source_type,
            host_path,
            source_mount_id,
            source_layer_id,
            source_subpath,
            source_publish_name,
        ) = match &input.source {
            MountSource::Host { path } => {
                ("host", Some(path.to_string_lossy().to_string()), None, None, None, None)
            }
            MountSource::Layer { source_mount_id, layer_id, subpath } => (
                "layer",
                None,
                Some(*source_mount_id),
                *layer_id,
                subpath.as_ref().map(|p| p.to_string_lossy().to_string()),
                None,
            ),
            MountSource::Published { publish_name, subpath } => (
                "published",
                None,
                None,
                None,
                subpath.as_ref().map(|p| p.to_string_lossy().to_string()),
                Some(publish_name.clone()),
            ),
            MountSource::WorkingLayer => ("working_layer", None, None, None, None, None),
        };

        sqlx::query(
            r#"
            INSERT INTO mount_entries (
                mount_entry_id, tenant_id, name, virtual_path, is_file,
                source_type, host_path, source_mount_id, source_layer_id,
                source_subpath, source_publish_name, mode, enabled,
                metadata, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            "#,
        )
        .bind(mount_entry_id)
        .bind(tenant_id)
        .bind(&input.name)
        .bind(input.virtual_path.to_string_lossy().to_string())
        .bind(input.is_file)
        .bind(source_type)
        .bind(host_path)
        .bind(source_mount_id)
        .bind(source_layer_id)
        .bind(source_subpath)
        .bind(source_publish_name)
        .bind(input.mode.as_str())
        .bind(true) // enabled
        .bind(&input.metadata)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(MountEntry {
            mount_entry_id,
            tenant_id,
            name: input.name,
            virtual_path: input.virtual_path,
            source: input.source,
            mode: input.mode,
            is_file: input.is_file,
            enabled: true,
            current_layer_id: None,
            metadata: input.metadata,
            created_at: now,
            updated_at: now,
        })
    }

    async fn get_mount_entry(&self, mount_entry_id: Uuid) -> Result<Option<MountEntry>> {
        let row = sqlx::query("SELECT * FROM mount_entries WHERE mount_entry_id = $1")
            .bind(mount_entry_id)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(r) => Ok(Some(self.row_to_mount_entry(&r)?)),
            None => Ok(None),
        }
    }

    async fn get_mount_entry_by_name(
        &self,
        tenant_id: Uuid,
        name: &str,
    ) -> Result<Option<MountEntry>> {
        let row = sqlx::query("SELECT * FROM mount_entries WHERE tenant_id = $1 AND name = $2")
            .bind(tenant_id)
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(r) => Ok(Some(self.row_to_mount_entry(&r)?)),
            None => Ok(None),
        }
    }

    async fn get_mount_entry_by_path(
        &self,
        tenant_id: Uuid,
        path: &Path,
    ) -> Result<Option<MountEntry>> {
        let row =
            sqlx::query("SELECT * FROM mount_entries WHERE tenant_id = $1 AND virtual_path = $2")
                .bind(tenant_id)
                .bind(path.to_string_lossy().to_string())
                .fetch_optional(&self.pool)
                .await?;

        match row {
            Some(r) => Ok(Some(self.row_to_mount_entry(&r)?)),
            None => Ok(None),
        }
    }

    async fn list_mount_entries(&self, tenant_id: Uuid) -> Result<Vec<MountEntry>> {
        let rows = sqlx::query("SELECT * FROM mount_entries WHERE tenant_id = $1 ORDER BY name")
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await?;

        rows.iter().map(|r| self.row_to_mount_entry(r)).collect()
    }

    async fn update_mount_entry(
        &self,
        mount_entry_id: Uuid,
        input: UpdateMountEntry,
    ) -> Result<MountEntry> {
        let now = Utc::now();

        // Build dynamic UPDATE query
        let mut updates = Vec::new();
        let mut bind_count = 1;

        if input.mode.is_some() {
            updates.push(format!("mode = ${}", bind_count));
            bind_count += 1;
        }
        if input.enabled.is_some() {
            updates.push(format!("enabled = ${}", bind_count));
            bind_count += 1;
        }
        if input.metadata.is_some() {
            updates.push(format!("metadata = ${}", bind_count));
            bind_count += 1;
        }

        updates.push(format!("updated_at = ${}", bind_count));

        if updates.is_empty() {
            return Err(anyhow!("No fields to update"));
        }

        let query_str = format!(
            "UPDATE mount_entries SET {} WHERE mount_entry_id = ${} RETURNING *",
            updates.join(", "),
            bind_count + 1
        );

        let mut query = sqlx::query(&query_str);

        if let Some(mode) = input.mode {
            query = query.bind(mode.as_str());
        }
        if let Some(enabled) = input.enabled {
            query = query.bind(enabled);
        }
        if let Some(metadata) = &input.metadata {
            query = query.bind(metadata);
        }

        query = query.bind(now).bind(mount_entry_id);

        let row = query.fetch_one(&self.pool).await?;

        self.row_to_mount_entry(&row)
    }

    async fn delete_mount_entry(&self, mount_entry_id: Uuid) -> Result<bool> {
        let result = sqlx::query("DELETE FROM mount_entries WHERE mount_entry_id = $1")
            .bind(mount_entry_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn set_mount_entries(
        &self,
        tenant_id: Uuid,
        entries: Vec<CreateMountEntry>,
    ) -> Result<Vec<MountEntry>> {
        // Validate no conflicts within the new entries
        let resolver = DefaultPathResolver::new();
        let mut temp_entries = Vec::new();

        for entry in &entries {
            resolver.validate_no_conflict(&temp_entries, &entry.virtual_path, entry.is_file)?;
            // Create temporary entry for validation
            temp_entries.push(MountEntry {
                mount_entry_id: Uuid::new_v4(),
                tenant_id,
                name: entry.name.clone(),
                virtual_path: entry.virtual_path.clone(),
                source: entry.source.clone(),
                mode: entry.mode,
                is_file: entry.is_file,
                enabled: true,
                current_layer_id: None,
                metadata: entry.metadata.clone(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            });
        }

        // Begin transaction
        let mut tx = self.pool.begin().await?;

        // Delete all existing mounts for this tenant
        sqlx::query("DELETE FROM mount_entries WHERE tenant_id = $1")
            .bind(tenant_id)
            .execute(&mut *tx)
            .await?;

        // Insert new mounts
        let mut result = Vec::new();
        for entry in entries {
            let mount_entry_id = Uuid::new_v4();
            let now = Utc::now();

            let (
                source_type,
                host_path,
                source_mount_id,
                source_layer_id,
                source_subpath,
                source_publish_name,
            ) = match &entry.source {
                MountSource::Host { path } => {
                    ("host", Some(path.to_string_lossy().to_string()), None, None, None, None)
                }
                MountSource::Layer { source_mount_id, layer_id, subpath } => (
                    "layer",
                    None,
                    Some(*source_mount_id),
                    *layer_id,
                    subpath.as_ref().map(|p| p.to_string_lossy().to_string()),
                    None,
                ),
                MountSource::Published { publish_name, subpath } => (
                    "published",
                    None,
                    None,
                    None,
                    subpath.as_ref().map(|p| p.to_string_lossy().to_string()),
                    Some(publish_name.clone()),
                ),
                MountSource::WorkingLayer => ("working_layer", None, None, None, None, None),
            };

            sqlx::query(
                r#"
                INSERT INTO mount_entries (
                    mount_entry_id, tenant_id, name, virtual_path, is_file,
                    source_type, host_path, source_mount_id, source_layer_id,
                    source_subpath, source_publish_name, mode, enabled,
                    metadata, created_at, updated_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
                "#,
            )
            .bind(mount_entry_id)
            .bind(tenant_id)
            .bind(&entry.name)
            .bind(entry.virtual_path.to_string_lossy().to_string())
            .bind(entry.is_file)
            .bind(source_type)
            .bind(host_path)
            .bind(source_mount_id)
            .bind(source_layer_id)
            .bind(source_subpath)
            .bind(source_publish_name)
            .bind(entry.mode.as_str())
            .bind(true)
            .bind(&entry.metadata)
            .bind(now)
            .bind(now)
            .execute(&mut *tx)
            .await?;

            result.push(MountEntry {
                mount_entry_id,
                tenant_id,
                name: entry.name,
                virtual_path: entry.virtual_path,
                source: entry.source,
                mode: entry.mode,
                is_file: entry.is_file,
                enabled: true,
                current_layer_id: None,
                metadata: entry.metadata,
                created_at: now,
                updated_at: now,
            });
        }

        // Commit transaction
        tx.commit().await?;

        Ok(result)
    }

    async fn check_path_conflict(
        &self,
        tenant_id: Uuid,
        path: &Path,
        is_file: bool,
        exclude_id: Option<Uuid>,
    ) -> Result<bool> {
        let existing = self.list_mount_entries(tenant_id).await?;

        // Filter out excluded entry
        let filtered: Vec<_> = if let Some(id) = exclude_id {
            existing.into_iter().filter(|e| e.mount_entry_id != id).collect()
        } else {
            existing
        };

        let resolver = DefaultPathResolver::new();
        match resolver.validate_no_conflict(&filtered, path, is_file) {
            Ok(_) => Ok(false),
            Err(_) => Ok(true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mount_mode_parsing() {
        assert_eq!(MountMode::parse_mode("ro"), Some(MountMode::ReadOnly));
        assert_eq!(MountMode::parse_mode("rw"), Some(MountMode::ReadWrite));
        assert_eq!(MountMode::parse_mode("cow"), Some(MountMode::CopyOnWrite));
        assert_eq!(MountMode::parse_mode("invalid"), None);
    }
}

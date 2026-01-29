use anyhow::{Result, anyhow};
use async_trait::async_trait;
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use super::models::published_mount::{
    PublishMountInput, PublishScope, PublishTarget, PublishedMount, PublishedMountFilter,
    ResolvedPublished, UpdatePublishInput,
};
use super::traits::PublishedMountRepository;

pub struct PgPublishedMountRepository {
    pool: PgPool,
}

impl PgPublishedMountRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Convert database row to PublishedMount
    fn row_to_published_mount(&self, row: &sqlx::postgres::PgRow) -> Result<PublishedMount> {
        use sqlx::Row;

        let publish_id: Uuid = row.try_get("publish_id")?;
        let mount_entry_id: Uuid = row.try_get("mount_entry_id")?;
        let tenant_id: Uuid = row.try_get("tenant_id")?;
        let publish_name: String = row.try_get("publish_name")?;
        let description: Option<String> = row.try_get("description")?;
        let target_type: String = row.try_get("target_type")?;
        let layer_id: Option<Uuid> = row.try_get("layer_id")?;
        let scope_str: String = row.try_get("scope")?;
        let allowed_tenants: Option<Vec<Uuid>> = row.try_get("allowed_tenants")?;
        let created_at = row.try_get("created_at")?;
        let updated_at = row.try_get("updated_at")?;

        // Parse target
        let target = match target_type.as_str() {
            "layer" => PublishTarget::Layer(
                layer_id.ok_or_else(|| anyhow!("layer_id required for layer target"))?,
            ),
            "working_layer" => PublishTarget::WorkingLayer,
            _ => return Err(anyhow!("Unknown target type: {}", target_type)),
        };

        // Parse scope
        let scope = match scope_str.as_str() {
            "public" => PublishScope::Public,
            "allow_list" => {
                PublishScope::AllowList { tenants: allowed_tenants.unwrap_or_default() }
            }
            _ => return Err(anyhow!("Unknown scope: {}", scope_str)),
        };

        Ok(PublishedMount {
            publish_id,
            mount_entry_id,
            tenant_id,
            publish_name,
            description,
            target,
            scope,
            created_at,
            updated_at,
        })
    }
}

#[async_trait]
impl PublishedMountRepository for PgPublishedMountRepository {
    async fn publish_mount(&self, input: PublishMountInput) -> Result<PublishedMount> {
        // Check if publish_name already exists
        if self.get_published_by_name(&input.publish_name).await?.is_some() {
            return Err(anyhow!(
                "PublishNameExists: '{}' is already published",
                input.publish_name
            ));
        }

        // Check if mount_entry is already published
        if self.get_publish_info(input.mount_entry_id).await?.is_some() {
            return Err(anyhow!("Mount entry is already published, unpublish first"));
        }

        // Get tenant_id from mount_entry
        let mount: (Uuid,) =
            sqlx::query_as("SELECT tenant_id FROM mount_entries WHERE mount_entry_id = $1")
                .bind(input.mount_entry_id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or_else(|| anyhow!("Mount entry not found"))?;

        let publish_id = Uuid::new_v4();
        let now = Utc::now();

        let (target_type, layer_id) = match &input.target {
            PublishTarget::Layer(id) => ("layer", Some(*id)),
            PublishTarget::WorkingLayer => ("working_layer", None),
        };

        let (scope_str, allowed_tenants) = match &input.scope {
            PublishScope::Public => ("public", None),
            PublishScope::AllowList { tenants } => ("allow_list", Some(tenants.as_slice())),
        };

        sqlx::query(
            r#"
            INSERT INTO published_mounts (
                publish_id, mount_entry_id, tenant_id, publish_name, description,
                target_type, layer_id, scope, allowed_tenants, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
        )
        .bind(publish_id)
        .bind(input.mount_entry_id)
        .bind(mount.0)
        .bind(&input.publish_name)
        .bind(&input.description)
        .bind(target_type)
        .bind(layer_id)
        .bind(scope_str)
        .bind(allowed_tenants)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(PublishedMount {
            publish_id,
            mount_entry_id: input.mount_entry_id,
            tenant_id: mount.0,
            publish_name: input.publish_name,
            description: input.description,
            target: input.target,
            scope: input.scope,
            created_at: now,
            updated_at: now,
        })
    }

    async fn unpublish_mount(&self, mount_entry_id: Uuid) -> Result<bool> {
        let result = sqlx::query("DELETE FROM published_mounts WHERE mount_entry_id = $1")
            .bind(mount_entry_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn get_published_by_name(&self, publish_name: &str) -> Result<Option<PublishedMount>> {
        let row = sqlx::query("SELECT * FROM published_mounts WHERE publish_name = $1")
            .bind(publish_name)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(r) => Ok(Some(self.row_to_published_mount(&r)?)),
            None => Ok(None),
        }
    }

    async fn get_publish_info(&self, mount_entry_id: Uuid) -> Result<Option<PublishedMount>> {
        let row = sqlx::query("SELECT * FROM published_mounts WHERE mount_entry_id = $1")
            .bind(mount_entry_id)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(r) => Ok(Some(self.row_to_published_mount(&r)?)),
            None => Ok(None),
        }
    }

    async fn list_published_mounts(
        &self,
        filter: PublishedMountFilter,
    ) -> Result<Vec<PublishedMount>> {
        let mut query = String::from("SELECT * FROM published_mounts WHERE 1=1");
        let mut bindings: Vec<String> = Vec::new();

        if let Some(scope) = &filter.scope
            && scope == "public"
        {
            query.push_str(" AND scope = 'public'");
        }
        // "all" means no filter

        if let Some(owner) = filter.owner_tenant_id {
            bindings.push(owner.to_string());
            query.push_str(&format!(" AND tenant_id = ${}", bindings.len()));
        }

        query.push_str(" ORDER BY created_at DESC");

        let mut sql_query = sqlx::query(&query);
        for binding in bindings {
            sql_query = sql_query.bind(Uuid::parse_str(&binding)?);
        }

        let rows = sql_query.fetch_all(&self.pool).await?;

        rows.iter().map(|r| self.row_to_published_mount(r)).collect()
    }

    async fn list_tenant_published_mounts(&self, tenant_id: Uuid) -> Result<Vec<PublishedMount>> {
        let rows = sqlx::query(
            "SELECT * FROM published_mounts WHERE tenant_id = $1 ORDER BY created_at DESC",
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(|r| self.row_to_published_mount(r)).collect()
    }

    async fn update_publish(
        &self,
        publish_id: Uuid,
        input: UpdatePublishInput,
    ) -> Result<PublishedMount> {
        let now = Utc::now();

        let mut updates = Vec::new();
        let mut bind_count = 1;

        if input.description.is_some() {
            updates.push(format!("description = ${}", bind_count));
            bind_count += 1;
        }

        if input.scope.is_some() {
            updates.push(format!("scope = ${}", bind_count));
            bind_count += 1;
            updates.push(format!("allowed_tenants = ${}", bind_count));
            bind_count += 1;
        }

        updates.push(format!("updated_at = ${}", bind_count));

        if updates.is_empty() {
            return Err(anyhow!("No fields to update"));
        }

        let query_str = format!(
            "UPDATE published_mounts SET {} WHERE publish_id = ${} RETURNING *",
            updates.join(", "),
            bind_count + 1
        );

        let mut query = sqlx::query(&query_str);

        if let Some(desc) = &input.description {
            query = query.bind(desc);
        }

        if let Some(scope) = &input.scope {
            let (scope_str, allowed_tenants) = match scope {
                PublishScope::Public => ("public", None),
                PublishScope::AllowList { tenants } => ("allow_list", Some(tenants.as_slice())),
            };
            query = query.bind(scope_str).bind(allowed_tenants);
        }

        query = query.bind(now).bind(publish_id);

        let row = query.fetch_one(&self.pool).await?;

        self.row_to_published_mount(&row)
    }

    async fn check_access(&self, publish_name: &str, accessor_tenant_id: Uuid) -> Result<bool> {
        let published = self
            .get_published_by_name(publish_name)
            .await?
            .ok_or_else(|| anyhow!("Published mount not found: {}", publish_name))?;

        // Owner always has access
        if published.tenant_id == accessor_tenant_id {
            return Ok(true);
        }

        // Check scope
        match published.scope {
            PublishScope::Public => Ok(true),
            PublishScope::AllowList { tenants } => Ok(tenants.contains(&accessor_tenant_id)),
        }
    }

    async fn add_allowed_tenant(&self, publish_id: Uuid, tenant_id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE published_mounts
            SET allowed_tenants = array_append(allowed_tenants, $1),
                updated_at = NOW()
            WHERE publish_id = $2
              AND scope = 'allow_list'
              AND NOT ($1 = ANY(allowed_tenants))
            "#,
        )
        .bind(tenant_id)
        .bind(publish_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn remove_allowed_tenant(&self, publish_id: Uuid, tenant_id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE published_mounts
            SET allowed_tenants = array_remove(allowed_tenants, $1),
                updated_at = NOW()
            WHERE publish_id = $2 AND scope = 'allow_list'
            "#,
        )
        .bind(tenant_id)
        .bind(publish_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn resolve_published(
        &self,
        publish_name: &str,
        accessor_tenant_id: Uuid,
    ) -> Result<ResolvedPublished> {
        // Check access first
        if !self.check_access(publish_name, accessor_tenant_id).await? {
            return Err(anyhow!("AccessDenied: tenant does not have access to '{}'", publish_name));
        }

        let published = self
            .get_published_by_name(publish_name)
            .await?
            .ok_or_else(|| anyhow!("Published mount not found"))?;

        let layer_id = match &published.target {
            PublishTarget::Layer(id) => *id,
            PublishTarget::WorkingLayer => {
                // Get current working layer from mount_entry
                let mount: (Option<Uuid>,) = sqlx::query_as(
                    "SELECT current_layer_id FROM mount_entries WHERE mount_entry_id = $1",
                )
                .bind(published.mount_entry_id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or_else(|| anyhow!("Mount entry not found"))?;

                mount.0.ok_or_else(|| anyhow!("Working layer not initialized for this mount"))?
            }
        };

        Ok(ResolvedPublished {
            mount_entry_id: published.mount_entry_id,
            owner_tenant_id: published.tenant_id,
            layer_id,
            is_working_layer: matches!(published.target, PublishTarget::WorkingLayer),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_publish_target_type_conversion() {
        let layer_target = PublishTarget::Layer(Uuid::new_v4());
        assert!(matches!(layer_target, PublishTarget::Layer(_)));

        let working_target = PublishTarget::WorkingLayer;
        assert!(matches!(working_target, PublishTarget::WorkingLayer));
    }

    #[test]
    fn test_publish_scope_public() {
        let scope = PublishScope::Public;
        assert!(matches!(scope, PublishScope::Public));
    }

    #[test]
    fn test_publish_scope_allow_list() {
        let tenants = vec![Uuid::new_v4(), Uuid::new_v4()];
        let scope = PublishScope::AllowList { tenants: tenants.clone() };
        match scope {
            PublishScope::AllowList { tenants: t } => assert_eq!(t.len(), 2),
            _ => panic!("Expected AllowList"),
        }
    }
}

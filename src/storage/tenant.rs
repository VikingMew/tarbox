use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::types::{InodeId, TenantId};

use super::models::{CreateTenantInput, Tenant};
use super::traits::TenantRepository;

pub struct TenantOperations<'a> {
    pool: &'a PgPool,
}

impl<'a> TenantOperations<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl<'a> TenantRepository for TenantOperations<'a> {
    async fn create(&self, input: CreateTenantInput) -> Result<Tenant> {
        let tenant_id = Uuid::new_v4();

        let mut tx = self.pool.begin().await?;

        // Create tenant first with a placeholder root_inode_id
        // We'll update it after creating the root inode
        sqlx::query(
            r#"
            INSERT INTO tenants (tenant_id, tenant_name, root_inode_id)
            VALUES ($1, $2, 0)
            "#,
        )
        .bind(tenant_id)
        .bind(&input.tenant_name)
        .execute(&mut *tx)
        .await?;

        // Create root inode (let BIGSERIAL generate the ID)
        let root_inode = sqlx::query_as::<_, (InodeId,)>(
            r#"
            INSERT INTO inodes (tenant_id, parent_id, name, inode_type, mode, uid, gid, size)
            VALUES ($1, NULL, $2, $3, $4, $5, $6, $7)
            RETURNING inode_id
            "#,
        )
        .bind(tenant_id)
        .bind("/")
        .bind("dir")
        .bind(0o755_i32)
        .bind(0_i32)
        .bind(0_i32)
        .bind(4096_i64)
        .fetch_one(&mut *tx)
        .await?;

        let root_inode_id = root_inode.0;

        // Update tenant with the actual root_inode_id
        let tenant = sqlx::query_as::<_, Tenant>(
            r#"
            UPDATE tenants
            SET root_inode_id = $2
            WHERE tenant_id = $1
            RETURNING tenant_id, tenant_name, root_inode_id, created_at, updated_at
            "#,
        )
        .bind(tenant_id)
        .bind(root_inode_id)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        tracing::info!(
            tenant_id = %tenant.tenant_id,
            tenant_name = %tenant.tenant_name,
            root_inode_id = root_inode_id,
            "Created new tenant with root inode"
        );

        Ok(tenant)
    }

    async fn get_by_id(&self, tenant_id: TenantId) -> Result<Option<Tenant>> {
        let tenant = sqlx::query_as::<_, Tenant>(
            r#"
            SELECT tenant_id, tenant_name, root_inode_id, created_at, updated_at
            FROM tenants
            WHERE tenant_id = $1
            "#,
        )
        .bind(tenant_id)
        .fetch_optional(self.pool)
        .await?;

        Ok(tenant)
    }

    async fn get_by_name(&self, tenant_name: &str) -> Result<Option<Tenant>> {
        let tenant = sqlx::query_as::<_, Tenant>(
            r#"
            SELECT tenant_id, tenant_name, root_inode_id, created_at, updated_at
            FROM tenants
            WHERE tenant_name = $1
            "#,
        )
        .bind(tenant_name)
        .fetch_optional(self.pool)
        .await?;

        Ok(tenant)
    }

    async fn list(&self) -> Result<Vec<Tenant>> {
        let tenants = sqlx::query_as::<_, Tenant>(
            r#"
            SELECT tenant_id, tenant_name, root_inode_id, created_at, updated_at
            FROM tenants
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(self.pool)
        .await?;

        Ok(tenants)
    }

    async fn delete(&self, tenant_id: TenantId) -> Result<bool> {
        let result = sqlx::query("DELETE FROM tenants WHERE tenant_id = $1")
            .bind(tenant_id)
            .execute(self.pool)
            .await?;

        let deleted = result.rows_affected() > 0;

        if deleted {
            tracing::info!(tenant_id = %tenant_id, "Deleted tenant");
        }

        Ok(deleted)
    }
}

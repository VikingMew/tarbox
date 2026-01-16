use anyhow::Result;
use chrono::Utc;
use sqlx::PgPool;

use crate::types::{InodeId, TenantId};

use super::models::{CreateInodeInput, Inode, UpdateInodeInput};

pub struct InodeOperations<'a> {
    pool: &'a PgPool,
}

impl<'a> InodeOperations<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, input: CreateInodeInput) -> Result<Inode> {
        let inode = sqlx::query_as::<_, Inode>(
            r#"
            INSERT INTO inodes (tenant_id, parent_id, name, inode_type, mode, uid, gid, size)
            VALUES ($1, $2, $3, $4, $5, $6, $7, 0)
            RETURNING inode_id, tenant_id, parent_id, name, inode_type, mode, uid, gid, size,
                      atime, mtime, ctime
            "#,
        )
        .bind(input.tenant_id)
        .bind(input.parent_id)
        .bind(&input.name)
        .bind(input.inode_type)
        .bind(input.mode)
        .bind(input.uid)
        .bind(input.gid)
        .fetch_one(self.pool)
        .await?;

        tracing::debug!(
            tenant_id = %inode.tenant_id,
            inode_id = inode.inode_id,
            name = %inode.name,
            inode_type = ?inode.inode_type,
            "Created inode"
        );

        Ok(inode)
    }

    pub async fn get(&self, tenant_id: TenantId, inode_id: InodeId) -> Result<Option<Inode>> {
        let inode = sqlx::query_as::<_, Inode>(
            r#"
            SELECT inode_id, tenant_id, parent_id, name, inode_type, mode, uid, gid, size,
                   atime, mtime, ctime
            FROM inodes
            WHERE tenant_id = $1 AND inode_id = $2
            "#,
        )
        .bind(tenant_id)
        .bind(inode_id)
        .fetch_optional(self.pool)
        .await?;

        Ok(inode)
    }

    pub async fn get_by_parent_and_name(
        &self,
        tenant_id: TenantId,
        parent_id: InodeId,
        name: &str,
    ) -> Result<Option<Inode>> {
        let inode = sqlx::query_as::<_, Inode>(
            r#"
            SELECT inode_id, tenant_id, parent_id, name, inode_type, mode, uid, gid, size,
                   atime, mtime, ctime
            FROM inodes
            WHERE tenant_id = $1 AND parent_id = $2 AND name = $3
            "#,
        )
        .bind(tenant_id)
        .bind(parent_id)
        .bind(name)
        .fetch_optional(self.pool)
        .await?;

        Ok(inode)
    }

    pub async fn update(
        &self,
        tenant_id: TenantId,
        inode_id: InodeId,
        input: UpdateInodeInput,
    ) -> Result<Inode> {
        let now = Utc::now();

        let mut query = String::from("UPDATE inodes SET ");
        let mut updates = Vec::new();
        let mut param_count = 3;

        if input.size.is_some() {
            updates.push(format!("size = ${}", param_count));
            param_count += 1;
        }
        if input.mode.is_some() {
            updates.push(format!("mode = ${}", param_count));
            param_count += 1;
        }
        if input.uid.is_some() {
            updates.push(format!("uid = ${}", param_count));
            param_count += 1;
        }
        if input.gid.is_some() {
            updates.push(format!("gid = ${}", param_count));
            param_count += 1;
        }
        if input.atime.is_some() {
            updates.push(format!("atime = ${}", param_count));
            param_count += 1;
        }
        if input.mtime.is_some() {
            updates.push(format!("mtime = ${}", param_count));
            param_count += 1;
        }
        if input.ctime.is_some() {
            updates.push(format!("ctime = ${}", param_count));
            param_count += 1;
        }

        if updates.is_empty() {
            updates.push(format!("ctime = ${}", param_count));
        }

        query.push_str(&updates.join(", "));
        query.push_str(" WHERE tenant_id = $1 AND inode_id = $2 RETURNING inode_id, tenant_id, parent_id, name, inode_type, mode, uid, gid, size, atime, mtime, ctime");

        let mut q = sqlx::query_as::<_, Inode>(&query).bind(tenant_id).bind(inode_id);

        if let Some(size) = input.size {
            q = q.bind(size);
        }
        if let Some(mode) = input.mode {
            q = q.bind(mode);
        }
        if let Some(uid) = input.uid {
            q = q.bind(uid);
        }
        if let Some(gid) = input.gid {
            q = q.bind(gid);
        }
        if let Some(atime) = input.atime {
            q = q.bind(atime);
        }
        if let Some(mtime) = input.mtime {
            q = q.bind(mtime);
        }
        if let Some(ctime) = input.ctime {
            q = q.bind(ctime);
        } else if updates.is_empty() {
            q = q.bind(now);
        }

        let inode = q.fetch_one(self.pool).await?;

        tracing::debug!(
            tenant_id = %tenant_id,
            inode_id = inode_id,
            "Updated inode"
        );

        Ok(inode)
    }

    pub async fn delete(&self, tenant_id: TenantId, inode_id: InodeId) -> Result<bool> {
        let result = sqlx::query("DELETE FROM inodes WHERE tenant_id = $1 AND inode_id = $2")
            .bind(tenant_id)
            .bind(inode_id)
            .execute(self.pool)
            .await?;

        let deleted = result.rows_affected() > 0;

        if deleted {
            tracing::debug!(tenant_id = %tenant_id, inode_id = inode_id, "Deleted inode");
        }

        Ok(deleted)
    }

    pub async fn list_children(
        &self,
        tenant_id: TenantId,
        parent_id: InodeId,
    ) -> Result<Vec<Inode>> {
        let children = sqlx::query_as::<_, Inode>(
            r#"
            SELECT inode_id, tenant_id, parent_id, name, inode_type, mode, uid, gid, size,
                   atime, mtime, ctime
            FROM inodes
            WHERE tenant_id = $1 AND parent_id = $2
            ORDER BY name
            "#,
        )
        .bind(tenant_id)
        .bind(parent_id)
        .fetch_all(self.pool)
        .await?;

        Ok(children)
    }
}

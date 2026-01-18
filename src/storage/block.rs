use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::types::{BlockId, InodeId, TenantId};

use super::models::{CreateBlockInput, DataBlock};
use super::traits::BlockRepository;

pub struct BlockOperations<'a> {
    pool: &'a PgPool,
}

impl<'a> BlockOperations<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, input: CreateBlockInput) -> Result<DataBlock> {
        let block_id = Uuid::new_v4();
        let size = input.data.len() as i32;
        let content_hash = compute_content_hash(&input.data);

        let block = sqlx::query_as::<_, DataBlock>(
            r#"
            INSERT INTO data_blocks (block_id, tenant_id, inode_id, block_index, data, size, content_hash)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING block_id, tenant_id, inode_id, block_index, data, size, content_hash, created_at
            "#,
        )
        .bind(block_id)
        .bind(input.tenant_id)
        .bind(input.inode_id)
        .bind(input.block_index)
        .bind(&input.data)
        .bind(size)
        .bind(&content_hash)
        .fetch_one(self.pool)
        .await
        ?;

        tracing::debug!(
            tenant_id = %block.tenant_id,
            inode_id = block.inode_id,
            block_id = %block.block_id,
            block_index = block.block_index,
            size = block.size,
            "Created data block"
        );

        Ok(block)
    }

    pub async fn get(
        &self,
        tenant_id: TenantId,
        inode_id: InodeId,
        block_index: i32,
    ) -> Result<Option<DataBlock>> {
        let block = sqlx::query_as::<_, DataBlock>(
            r#"
            SELECT block_id, tenant_id, inode_id, block_index, data, size, content_hash, created_at
            FROM data_blocks
            WHERE tenant_id = $1 AND inode_id = $2 AND block_index = $3
            "#,
        )
        .bind(tenant_id)
        .bind(inode_id)
        .bind(block_index)
        .fetch_optional(self.pool)
        .await?;

        Ok(block)
    }

    pub async fn get_by_id(&self, block_id: BlockId) -> Result<Option<DataBlock>> {
        let block = sqlx::query_as::<_, DataBlock>(
            r#"
            SELECT block_id, tenant_id, inode_id, block_index, data, size, content_hash, created_at
            FROM data_blocks
            WHERE block_id = $1
            "#,
        )
        .bind(block_id)
        .fetch_optional(self.pool)
        .await?;

        Ok(block)
    }

    pub async fn list(&self, tenant_id: TenantId, inode_id: InodeId) -> Result<Vec<DataBlock>> {
        let blocks = sqlx::query_as::<_, DataBlock>(
            r#"
            SELECT block_id, tenant_id, inode_id, block_index, data, size, content_hash, created_at
            FROM data_blocks
            WHERE tenant_id = $1 AND inode_id = $2
            ORDER BY block_index
            "#,
        )
        .bind(tenant_id)
        .bind(inode_id)
        .fetch_all(self.pool)
        .await?;

        Ok(blocks)
    }

    pub async fn delete(&self, tenant_id: TenantId, inode_id: InodeId) -> Result<u64> {
        let result = sqlx::query("DELETE FROM data_blocks WHERE tenant_id = $1 AND inode_id = $2")
            .bind(tenant_id)
            .bind(inode_id)
            .execute(self.pool)
            .await?;

        let count = result.rows_affected();

        if count > 0 {
            tracing::debug!(
                tenant_id = %tenant_id,
                inode_id = inode_id,
                count = count,
                "Deleted data blocks"
            );
        }

        Ok(count)
    }

    pub async fn delete_block(
        &self,
        tenant_id: TenantId,
        inode_id: InodeId,
        block_index: i32,
    ) -> Result<bool> {
        let result = sqlx::query(
            "DELETE FROM data_blocks WHERE tenant_id = $1 AND inode_id = $2 AND block_index = $3",
        )
        .bind(tenant_id)
        .bind(inode_id)
        .bind(block_index)
        .execute(self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}

pub fn compute_content_hash(data: &[u8]) -> String {
    let hash = blake3::hash(data);
    hash.to_hex().to_string()
}

// Implement BlockRepository trait for BlockOperations
#[async_trait]
impl<'a> BlockRepository for BlockOperations<'a> {
    async fn create(&self, input: CreateBlockInput) -> Result<DataBlock> {
        self.create(input).await
    }

    async fn get(
        &self,
        tenant_id: TenantId,
        inode_id: InodeId,
        block_index: i32,
    ) -> Result<Option<DataBlock>> {
        self.get(tenant_id, inode_id, block_index).await
    }

    async fn list(&self, tenant_id: TenantId, inode_id: InodeId) -> Result<Vec<DataBlock>> {
        BlockOperations::list(self, tenant_id, inode_id).await
    }

    async fn delete(&self, tenant_id: TenantId, inode_id: InodeId) -> Result<u64> {
        BlockOperations::delete(self, tenant_id, inode_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_content_hash() {
        let data = b"hello world";
        let hash = compute_content_hash(data);
        assert_eq!(hash.len(), 64);

        let hash2 = compute_content_hash(data);
        assert_eq!(hash, hash2);

        let data3 = b"different data";
        let hash3 = compute_content_hash(data3);
        assert_ne!(hash, hash3);
    }

    #[test]
    fn test_compute_content_hash_empty() {
        let hash = compute_content_hash(b"");
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_compute_content_hash_large_data() {
        let data = vec![0u8; 1024 * 1024]; // 1MB
        let hash = compute_content_hash(&data);
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_compute_content_hash_deterministic() {
        let data = b"test data for hashing";
        let hash1 = compute_content_hash(data);
        let hash2 = compute_content_hash(data);
        let hash3 = compute_content_hash(data);
        assert_eq!(hash1, hash2);
        assert_eq!(hash2, hash3);
    }

    #[test]
    fn test_compute_content_hash_different_data() {
        let data1 = b"hello";
        let data2 = b"world";

        let hash1 = compute_content_hash(data1);
        let hash2 = compute_content_hash(data2);

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_compute_content_hash_single_byte() {
        let data = b"a";
        let hash = compute_content_hash(data);
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64); // SHA256 = 64 hex chars
    }

    #[test]
    fn test_compute_content_hash_binary_data() {
        let data = vec![0u8, 255u8, 128u8, 64u8];
        let hash = compute_content_hash(&data);
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_compute_content_hash_utf8() {
        let data = "Hello 世界 🌍".as_bytes();
        let hash = compute_content_hash(data);
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_compute_content_hash_hex_format() {
        let data = b"test";
        let hash = compute_content_hash(data);

        // Check if hash is valid hex
        for c in hash.chars() {
            assert!(c.is_ascii_hexdigit());
        }
    }

    #[test]
    fn test_compute_content_hash_case() {
        let data = b"ABC";
        let hash = compute_content_hash(data);

        // SHA256 hex should be lowercase
        assert_eq!(hash, hash.to_lowercase());
    }
}

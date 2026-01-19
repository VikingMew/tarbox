use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::types::{BlockId, InodeId, LayerId, TenantId};

use super::models::{
    CreateTextBlockInput, CreateTextMetadataInput, TextBlock, TextFileMetadata, TextLineMap,
};
use super::traits::TextBlockRepository;

pub struct TextBlockOperations<'a> {
    pool: &'a PgPool,
}

impl<'a> TextBlockOperations<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Compute content hash using blake3
    fn compute_content_hash(content: &str) -> String {
        let hash = blake3::hash(content.as_bytes());
        hash.to_hex().to_string()
    }
}

#[async_trait]
impl<'a> TextBlockRepository for TextBlockOperations<'a> {
    async fn create_block(&self, input: CreateTextBlockInput) -> Result<TextBlock> {
        let content_hash = Self::compute_content_hash(&input.content);

        // Try to find existing block with same hash (deduplication)
        if let Some(existing) = self.get_block_by_hash(&content_hash).await? {
            // Block already exists, just return it (ref_count will be incremented separately)
            tracing::debug!(
                block_id = %existing.block_id,
                content_hash = %content_hash,
                "Reusing existing text block"
            );
            return Ok(existing);
        }

        let block_id = Uuid::new_v4();
        let line_count = input.content.lines().count() as i32;
        let byte_size = input.content.len() as i32;

        let block = sqlx::query_as::<_, TextBlock>(
            r#"
            INSERT INTO text_blocks (
                block_id, content_hash, content, line_count, byte_size, encoding, ref_count
            )
            VALUES ($1, $2, $3, $4, $5, $6, 0)
            RETURNING block_id, content_hash, content, line_count, byte_size, encoding,
                      ref_count, created_at, last_accessed_at
            "#,
        )
        .bind(block_id)
        .bind(&content_hash)
        .bind(&input.content)
        .bind(line_count)
        .bind(byte_size)
        .bind(&input.encoding)
        .fetch_one(self.pool)
        .await?;

        tracing::debug!(
            block_id = %block_id,
            content_hash = %content_hash,
            line_count = line_count,
            "Created new text block"
        );

        Ok(block)
    }

    async fn get_block(&self, block_id: BlockId) -> Result<Option<TextBlock>> {
        let block = sqlx::query_as::<_, TextBlock>(
            r#"
            SELECT block_id, content_hash, content, line_count, byte_size, encoding,
                   ref_count, created_at, last_accessed_at
            FROM text_blocks
            WHERE block_id = $1
            "#,
        )
        .bind(block_id)
        .fetch_optional(self.pool)
        .await?;

        Ok(block)
    }

    async fn get_block_by_hash(&self, content_hash: &str) -> Result<Option<TextBlock>> {
        let block = sqlx::query_as::<_, TextBlock>(
            r#"
            SELECT block_id, content_hash, content, line_count, byte_size, encoding,
                   ref_count, created_at, last_accessed_at
            FROM text_blocks
            WHERE content_hash = $1
            "#,
        )
        .bind(content_hash)
        .fetch_optional(self.pool)
        .await?;

        Ok(block)
    }

    async fn increment_ref_count(&self, block_id: BlockId) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE text_blocks
            SET ref_count = ref_count + 1
            WHERE block_id = $1
            "#,
        )
        .bind(block_id)
        .execute(self.pool)
        .await?;

        tracing::trace!(
            block_id = %block_id,
            "Incremented text block ref_count"
        );

        Ok(())
    }

    async fn decrement_ref_count(&self, block_id: BlockId) -> Result<i32> {
        let new_count = sqlx::query_as::<_, (i32,)>(
            r#"
            UPDATE text_blocks
            SET ref_count = GREATEST(ref_count - 1, 0)
            WHERE block_id = $1
            RETURNING ref_count
            "#,
        )
        .bind(block_id)
        .fetch_one(self.pool)
        .await?
        .0;

        tracing::trace!(
            block_id = %block_id,
            new_count = new_count,
            "Decremented text block ref_count"
        );

        Ok(new_count)
    }

    async fn create_metadata(&self, input: CreateTextMetadataInput) -> Result<TextFileMetadata> {
        let metadata = sqlx::query_as::<_, TextFileMetadata>(
            r#"
            INSERT INTO text_file_metadata (
                tenant_id, inode_id, layer_id, total_lines, encoding,
                line_ending, has_trailing_newline
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING tenant_id, inode_id, layer_id, total_lines, encoding,
                      line_ending, has_trailing_newline, created_at
            "#,
        )
        .bind(input.tenant_id)
        .bind(input.inode_id)
        .bind(input.layer_id)
        .bind(input.total_lines)
        .bind(&input.encoding)
        .bind(&input.line_ending)
        .bind(input.has_trailing_newline)
        .fetch_one(self.pool)
        .await?;

        tracing::debug!(
            tenant_id = %input.tenant_id,
            inode_id = input.inode_id,
            layer_id = %input.layer_id,
            total_lines = input.total_lines,
            "Created text file metadata"
        );

        Ok(metadata)
    }

    async fn get_metadata(
        &self,
        tenant_id: TenantId,
        inode_id: InodeId,
        layer_id: LayerId,
    ) -> Result<Option<TextFileMetadata>> {
        let metadata = sqlx::query_as::<_, TextFileMetadata>(
            r#"
            SELECT tenant_id, inode_id, layer_id, total_lines, encoding,
                   line_ending, has_trailing_newline, created_at
            FROM text_file_metadata
            WHERE tenant_id = $1 AND inode_id = $2 AND layer_id = $3
            "#,
        )
        .bind(tenant_id)
        .bind(inode_id)
        .bind(layer_id)
        .fetch_optional(self.pool)
        .await?;

        Ok(metadata)
    }

    async fn create_line_mappings(
        &self,
        tenant_id: TenantId,
        inode_id: InodeId,
        layer_id: LayerId,
        mappings: Vec<(i32, BlockId, i32)>,
    ) -> Result<u64> {
        if mappings.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await?;
        let mut inserted = 0u64;

        for (line_number, block_id, block_line_offset) in mappings {
            let result = sqlx::query(
                r#"
                INSERT INTO text_line_map (
                    tenant_id, inode_id, layer_id, line_number, block_id, block_line_offset
                )
                VALUES ($1, $2, $3, $4, $5, $6)
                "#,
            )
            .bind(tenant_id)
            .bind(inode_id)
            .bind(layer_id)
            .bind(line_number)
            .bind(block_id)
            .bind(block_line_offset)
            .execute(&mut *tx)
            .await?;

            inserted += result.rows_affected();
        }

        tx.commit().await?;

        tracing::debug!(
            tenant_id = %tenant_id,
            inode_id = inode_id,
            layer_id = %layer_id,
            count = inserted,
            "Created text line mappings"
        );

        Ok(inserted)
    }

    async fn get_line_mappings(
        &self,
        tenant_id: TenantId,
        inode_id: InodeId,
        layer_id: LayerId,
    ) -> Result<Vec<TextLineMap>> {
        let mappings = sqlx::query_as::<_, TextLineMap>(
            r#"
            SELECT tenant_id, inode_id, layer_id, line_number, block_id, block_line_offset
            FROM text_line_map
            WHERE tenant_id = $1 AND inode_id = $2 AND layer_id = $3
            ORDER BY line_number
            "#,
        )
        .bind(tenant_id)
        .bind(inode_id)
        .bind(layer_id)
        .fetch_all(self.pool)
        .await?;

        Ok(mappings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_content_hash() {
        let content = "Hello, world!\n";
        let hash = TextBlockOperations::compute_content_hash(content);

        // blake3 hash should be 64 hex characters
        assert_eq!(hash.len(), 64);

        // Same content should produce same hash
        let hash2 = TextBlockOperations::compute_content_hash(content);
        assert_eq!(hash, hash2);

        // Different content should produce different hash
        let hash3 = TextBlockOperations::compute_content_hash("Different content\n");
        assert_ne!(hash, hash3);
    }

    #[test]
    fn test_content_hash_deterministic() {
        let content = "Hello, Rust!\n";
        let hash1 = TextBlockOperations::compute_content_hash(content);
        let hash2 = TextBlockOperations::compute_content_hash(content);
        let hash3 = TextBlockOperations::compute_content_hash(content);

        // Should produce same hash every time
        assert_eq!(hash1, hash2);
        assert_eq!(hash2, hash3);
    }

    #[test]
    fn test_content_hash_empty_string() {
        let empty = "";
        let hash = TextBlockOperations::compute_content_hash(empty);

        // Should still produce valid 64-char hash
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_content_hash_unicode() {
        let unicode = "你好世界 🌍\n";
        let hash = TextBlockOperations::compute_content_hash(unicode);

        assert_eq!(hash.len(), 64);

        // Same unicode should produce same hash
        let hash2 = TextBlockOperations::compute_content_hash(unicode);
        assert_eq!(hash, hash2);
    }

    #[test]
    fn test_content_hash_newline_sensitivity() {
        let with_newline = "line\n";
        let without_newline = "line";

        let hash1 = TextBlockOperations::compute_content_hash(with_newline);
        let hash2 = TextBlockOperations::compute_content_hash(without_newline);

        // Different content should produce different hashes
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_create_text_block_input_validation() {
        let input = CreateTextBlockInput {
            content: "Test content\n".to_string(),
            encoding: "utf-8".to_string(),
        };

        assert_eq!(input.content, "Test content\n");
        assert_eq!(input.encoding, "utf-8");
    }

    #[test]
    fn test_create_text_metadata_input() {
        let tenant_id = uuid::Uuid::new_v4();
        let layer_id = uuid::Uuid::new_v4();

        let input = CreateTextMetadataInput {
            tenant_id,
            inode_id: 123,
            layer_id,
            total_lines: 10,
            encoding: "utf-8".to_string(),
            line_ending: "LF".to_string(),
            has_trailing_newline: true,
        };

        assert_eq!(input.total_lines, 10);
        assert_eq!(input.encoding, "utf-8");
        assert_eq!(input.line_ending, "LF");
        assert!(input.has_trailing_newline);
    }

    #[test]
    fn test_text_line_map_construction() {
        let _tenant_id = uuid::Uuid::new_v4();
        let _layer_id = uuid::Uuid::new_v4();
        let block_id = uuid::Uuid::new_v4();

        // Simulate line mappings
        let mappings = vec![
            (1i32, block_id, 0i32), // Line 1, block 0, offset 0
            (2i32, block_id, 1i32), // Line 2, block 0, offset 1
            (3i32, block_id, 2i32), // Line 3, block 0, offset 2
        ];

        assert_eq!(mappings.len(), 3);
        assert_eq!(mappings[0].0, 1);
        assert_eq!(mappings[1].0, 2);
        assert_eq!(mappings[2].0, 3);
    }
}

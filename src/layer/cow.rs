//! Copy-on-Write (COW) handler module.
//!
//! Implements write-time copy semantics for both binary and text files.
//! Binary files use block-level COW, text files use line-level diff.

use anyhow::Result;
use similar::{ChangeTag, TextDiff};
use sqlx::PgPool;

use crate::layer::detection::{FileTypeDetector, FileTypeInfo, LineEnding, TextEncoding};
use crate::storage::{
    BlockOperations, ChangeType, CreateBlockInput, CreateTextBlockInput, CreateTextMetadataInput,
    TextBlockOperations, TextBlockRepository,
};
use crate::types::{InodeId, LayerId, TenantId};

/// Result of a COW operation.
#[derive(Debug)]
pub struct CowResult {
    /// The change type (Add, Modify).
    pub change_type: ChangeType,
    /// Size delta (bytes changed).
    pub size_delta: i64,
    /// Text changes if applicable.
    pub text_changes: Option<TextChanges>,
    /// Whether the file is stored as text.
    pub is_text: bool,
}

/// Text file change statistics.
#[derive(Debug, Clone)]
pub struct TextChanges {
    pub lines_added: i32,
    pub lines_deleted: i32,
    pub lines_modified: i32,
    pub total_lines: i32,
}

impl TextChanges {
    /// Convert to JSON value.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "lines_added": self.lines_added,
            "lines_deleted": self.lines_deleted,
            "lines_modified": self.lines_modified,
            "total_lines": self.total_lines
        })
    }
}

/// COW handler for managing copy-on-write operations.
pub struct CowHandler<'a> {
    pool: &'a PgPool,
    tenant_id: TenantId,
    current_layer_id: LayerId,
    detector: FileTypeDetector,
}

impl<'a> CowHandler<'a> {
    /// Create a new COW handler.
    pub fn new(pool: &'a PgPool, tenant_id: TenantId, current_layer_id: LayerId) -> Self {
        Self { pool, tenant_id, current_layer_id, detector: FileTypeDetector::new() }
    }

    /// Write data to a file with COW semantics.
    ///
    /// This detects whether the file is text or binary and uses the appropriate
    /// storage strategy.
    pub async fn write_file(
        &self,
        inode_id: InodeId,
        data: &[u8],
        old_data: Option<&[u8]>,
    ) -> Result<CowResult> {
        let file_type = self.detector.detect(data);
        let is_new = old_data.is_none();
        let old_size = old_data.map(|d| d.len()).unwrap_or(0);

        match file_type {
            FileTypeInfo::Text { encoding, line_ending, line_count } => {
                self.write_text_file(inode_id, data, old_data, encoding, line_ending, line_count)
                    .await
            }
            FileTypeInfo::Binary => self.write_binary_file(inode_id, data, is_new, old_size).await,
        }
    }

    /// Write a binary file using block-level COW.
    async fn write_binary_file(
        &self,
        inode_id: InodeId,
        data: &[u8],
        is_new: bool,
        old_size: usize,
    ) -> Result<CowResult> {
        let block_ops = BlockOperations::new(self.pool);

        // Delete old blocks (they belong to this layer)
        block_ops.delete(self.tenant_id, inode_id).await?;

        // Create new blocks
        const BLOCK_SIZE: usize = 4096;
        let chunks: Vec<&[u8]> = data.chunks(BLOCK_SIZE).collect();

        for (index, chunk) in chunks.iter().enumerate() {
            block_ops
                .create(CreateBlockInput {
                    tenant_id: self.tenant_id,
                    inode_id,
                    block_index: index as i32,
                    data: chunk.to_vec(),
                })
                .await?;
        }

        let size_delta = data.len() as i64 - old_size as i64;
        let change_type = if is_new { ChangeType::Add } else { ChangeType::Modify };

        Ok(CowResult { change_type, size_delta, text_changes: None, is_text: false })
    }

    /// Write a text file using line-level diff.
    async fn write_text_file(
        &self,
        inode_id: InodeId,
        data: &[u8],
        old_data: Option<&[u8]>,
        encoding: TextEncoding,
        line_ending: LineEnding,
        _line_count: usize,
    ) -> Result<CowResult> {
        let text_ops = TextBlockOperations::new(self.pool);

        // Convert to string
        let new_text = String::from_utf8_lossy(data);
        let old_text = old_data.map(|d| String::from_utf8_lossy(d).into_owned());

        // Split into lines (preserving line endings for accurate diff)
        let new_lines: Vec<&str> = new_text.lines().collect();
        let old_lines: Vec<&str> =
            old_text.as_ref().map(|t| t.lines().collect()).unwrap_or_default();

        let is_new = old_data.is_none();
        let total_lines = new_lines.len() as i32;

        // Calculate diff if not a new file
        let text_changes = if is_new {
            TextChanges {
                lines_added: total_lines,
                lines_deleted: 0,
                lines_modified: 0,
                total_lines,
            }
        } else {
            self.calculate_line_diff(&old_lines, &new_lines)
        };

        // Create text file metadata
        let has_trailing_newline = new_text.ends_with('\n') || new_text.ends_with("\r\n");
        text_ops
            .create_metadata(CreateTextMetadataInput {
                tenant_id: self.tenant_id,
                inode_id,
                layer_id: self.current_layer_id,
                total_lines,
                encoding: encoding.to_string(),
                line_ending: line_ending.to_string(),
                has_trailing_newline,
            })
            .await?;

        // Create text blocks and line mappings
        let mut mappings = Vec::new();
        for (line_num, line) in new_lines.iter().enumerate() {
            // Try to find existing block with same content
            let content_hash = compute_text_hash(line);
            let block_id = match text_ops.get_block_by_hash(&content_hash).await? {
                Some(existing) => {
                    // Reuse existing block, increment ref count
                    text_ops.increment_ref_count(existing.block_id).await?;
                    existing.block_id
                }
                None => {
                    // Create new block
                    let block = text_ops
                        .create_block(CreateTextBlockInput {
                            content: line.to_string(),
                            encoding: encoding.to_string(),
                        })
                        .await?;
                    block.block_id
                }
            };

            mappings.push((line_num as i32, block_id, 0)); // line_offset is 0 for single-line blocks
        }

        // Store line mappings
        text_ops
            .create_line_mappings(self.tenant_id, inode_id, self.current_layer_id, mappings)
            .await?;

        let size_delta = data.len() as i64 - old_data.map(|d| d.len()).unwrap_or(0) as i64;
        let change_type = if is_new { ChangeType::Add } else { ChangeType::Modify };

        Ok(CowResult { change_type, size_delta, text_changes: Some(text_changes), is_text: true })
    }

    /// Calculate line-level diff between old and new content.
    fn calculate_line_diff(&self, old_lines: &[&str], new_lines: &[&str]) -> TextChanges {
        let diff = TextDiff::from_slices(old_lines, new_lines);

        let mut lines_added: usize = 0;
        let mut lines_deleted: usize = 0;

        for change in diff.iter_all_changes() {
            match change.tag() {
                ChangeTag::Delete => lines_deleted += 1,
                ChangeTag::Insert => lines_added += 1,
                ChangeTag::Equal => {}
            }
        }

        // Consider consecutive delete+insert pairs as modifications
        // This is a simplification; a more sophisticated approach would
        // track actual line positions
        let modifications = lines_added.min(lines_deleted);
        let lines_modified = modifications as i32;
        let lines_added = (lines_added - modifications) as i32;
        let lines_deleted = (lines_deleted - modifications) as i32;

        TextChanges {
            lines_added,
            lines_deleted,
            lines_modified,
            total_lines: new_lines.len() as i32,
        }
    }

    /// Read a text file by reconstructing from text blocks.
    pub async fn read_text_file(
        &self,
        inode_id: InodeId,
        layer_id: LayerId,
    ) -> Result<Option<String>> {
        let text_ops = TextBlockOperations::new(self.pool);

        // Get metadata
        let metadata = match text_ops.get_metadata(self.tenant_id, inode_id, layer_id).await? {
            Some(m) => m,
            None => return Ok(None),
        };

        // Get line mappings
        let mappings = text_ops.get_line_mappings(self.tenant_id, inode_id, layer_id).await?;

        // Sort by line number
        let mut mappings = mappings;
        mappings.sort_by_key(|m| m.line_number);

        // Reconstruct file content
        let mut lines = Vec::with_capacity(mappings.len());
        for mapping in mappings {
            if let Some(block) = text_ops.get_block(mapping.block_id).await? {
                lines.push(block.content);
            }
        }

        // Join with appropriate line ending
        let line_ending = match metadata.line_ending.as_str() {
            "crlf" => "\r\n",
            "cr" => "\r",
            _ => "\n",
        };

        let mut result = lines.join(line_ending);
        if metadata.has_trailing_newline && !result.is_empty() {
            result.push_str(line_ending);
        }

        Ok(Some(result))
    }

    /// Delete text file data from a layer.
    pub async fn delete_text_file(&self, inode_id: InodeId, layer_id: LayerId) -> Result<()> {
        let text_ops = TextBlockOperations::new(self.pool);

        // Get line mappings to decrement ref counts
        let mappings = text_ops.get_line_mappings(self.tenant_id, inode_id, layer_id).await?;

        for mapping in mappings {
            text_ops.decrement_ref_count(mapping.block_id).await?;
        }

        // Note: The actual deletion of line mappings and metadata would be handled
        // by the layer deletion process

        Ok(())
    }
}

/// Compute a hash for text content.
fn compute_text_hash(content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Generate a diff between two text contents.
#[cfg(test)]
pub fn generate_diff(old_content: &str, new_content: &str) -> String {
    let diff = TextDiff::from_lines(old_content, new_content);
    let mut output = String::new();

    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            ChangeTag::Delete => "-",
            ChangeTag::Insert => "+",
            ChangeTag::Equal => " ",
        };
        output.push_str(sign);
        output.push_str(change.value());
        if change.missing_newline() {
            output.push_str("\n\\ No newline at end of file\n");
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_changes_to_json() {
        let changes =
            TextChanges { lines_added: 5, lines_deleted: 2, lines_modified: 3, total_lines: 100 };
        let json = changes.to_json();
        assert_eq!(json["lines_added"], 5);
        assert_eq!(json["lines_deleted"], 2);
        assert_eq!(json["lines_modified"], 3);
        assert_eq!(json["total_lines"], 100);
    }

    #[test]
    fn test_generate_diff() {
        let old = "line1\nline2\nline3\n";
        let new = "line1\nmodified\nline3\nline4\n";
        let diff = generate_diff(old, new);
        assert!(diff.contains("-line2"));
        assert!(diff.contains("+modified"));
        assert!(diff.contains("+line4"));
    }

    #[test]
    fn test_compute_text_hash() {
        let hash1 = compute_text_hash("hello");
        let hash2 = compute_text_hash("hello");
        let hash3 = compute_text_hash("world");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }
}

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::types::{BlockId, InodeId, LayerId, TenantId};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Tenant {
    pub tenant_id: TenantId,
    pub tenant_name: String,
    pub root_inode_id: InodeId,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTenantInput {
    pub tenant_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
pub enum InodeType {
    File,
    Dir,
    Symlink,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Inode {
    pub inode_id: InodeId,
    pub tenant_id: TenantId,
    pub parent_id: Option<InodeId>,
    pub name: String,
    pub inode_type: InodeType,
    pub mode: i32,
    pub uid: i32,
    pub gid: i32,
    pub size: i64,
    pub atime: DateTime<Utc>,
    pub mtime: DateTime<Utc>,
    pub ctime: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateInodeInput {
    pub tenant_id: TenantId,
    pub parent_id: Option<InodeId>,
    pub name: String,
    pub inode_type: InodeType,
    pub mode: i32,
    pub uid: i32,
    pub gid: i32,
}

#[derive(Debug, Clone)]
pub struct UpdateInodeInput {
    pub size: Option<i64>,
    pub mode: Option<i32>,
    pub uid: Option<i32>,
    pub gid: Option<i32>,
    pub atime: Option<DateTime<Utc>>,
    pub mtime: Option<DateTime<Utc>>,
    pub ctime: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DataBlock {
    pub block_id: BlockId,
    pub tenant_id: TenantId,
    pub inode_id: InodeId,
    pub block_index: i32,
    pub data: Vec<u8>,
    pub size: i32,
    pub content_hash: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateBlockInput {
    pub tenant_id: TenantId,
    pub inode_id: InodeId,
    pub block_index: i32,
    pub data: Vec<u8>,
}

// ============================================================================
// Audit Log Models
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AuditLog {
    pub log_id: i64,
    pub tenant_id: TenantId,
    pub inode_id: Option<InodeId>,
    pub operation: String,
    pub uid: i32,
    pub gid: i32,
    pub pid: Option<i32>,
    pub path: String,
    pub success: bool,
    pub error_code: Option<i32>,
    pub error_message: Option<String>,
    pub bytes_read: Option<i64>,
    pub bytes_written: Option<i64>,
    pub duration_ms: Option<i32>,
    pub text_changes: Option<serde_json::Value>,
    pub is_native_mount: bool,
    pub native_source_path: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub log_date: chrono::NaiveDate,
}

#[derive(Debug, Clone)]
pub struct CreateAuditLogInput {
    pub tenant_id: TenantId,
    pub inode_id: Option<InodeId>,
    pub operation: String,
    pub uid: i32,
    pub gid: i32,
    pub pid: Option<i32>,
    pub path: String,
    pub success: bool,
    pub error_code: Option<i32>,
    pub error_message: Option<String>,
    pub bytes_read: Option<i64>,
    pub bytes_written: Option<i64>,
    pub duration_ms: Option<i32>,
    pub text_changes: Option<serde_json::Value>,
    pub is_native_mount: bool,
    pub native_source_path: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct QueryAuditLogsInput {
    pub tenant_id: TenantId,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub operation: Option<String>,
    pub uid: Option<i32>,
    pub path_pattern: Option<String>,
    pub success: Option<bool>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditStats {
    pub total_operations: i64,
    pub successful_operations: i64,
    pub failed_operations: i64,
    pub total_bytes_read: i64,
    pub total_bytes_written: i64,
    pub avg_duration_ms: Option<f64>,
}

// ============================================================================
// Layer Models
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
pub enum LayerStatus {
    Active,
    Creating,
    Deleting,
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
pub enum ChangeType {
    Add,
    Modify,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Layer {
    pub layer_id: LayerId,
    pub tenant_id: TenantId,
    pub parent_layer_id: Option<LayerId>,
    pub layer_name: String,
    pub description: Option<String>,
    pub file_count: i32,
    pub total_size: i64,
    pub status: LayerStatus,
    pub is_readonly: bool,
    pub tags: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub created_by: String,

    // Mount-level layer chains (Task 21)
    pub mount_entry_id: Option<uuid::Uuid>,
    pub is_working: bool,
}

#[derive(Debug, Clone)]
pub struct CreateLayerInput {
    pub tenant_id: TenantId,
    pub parent_layer_id: Option<LayerId>,
    pub layer_name: String,
    pub description: Option<String>,
    pub tags: Option<serde_json::Value>,
    pub created_by: String,

    // Mount-level layer chains (Task 21)
    pub mount_entry_id: Option<uuid::Uuid>,
    pub is_working: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LayerEntry {
    pub entry_id: uuid::Uuid,
    pub layer_id: LayerId,
    pub tenant_id: TenantId,
    pub inode_id: InodeId,
    pub path: String,
    pub change_type: ChangeType,
    pub size_delta: Option<i64>,
    pub text_changes: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateLayerEntryInput {
    pub layer_id: LayerId,
    pub tenant_id: TenantId,
    pub inode_id: InodeId,
    pub path: String,
    pub change_type: ChangeType,
    pub size_delta: Option<i64>,
    pub text_changes: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TenantCurrentLayer {
    pub tenant_id: TenantId,
    pub current_layer_id: LayerId,
    pub updated_at: DateTime<Utc>,
}

// ============================================================================
// Text File Optimization Models
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TextBlock {
    pub block_id: BlockId,
    pub content_hash: String,
    pub content: String,
    pub line_count: i32,
    pub byte_size: i32,
    pub encoding: String,
    pub ref_count: i32,
    pub created_at: DateTime<Utc>,
    pub last_accessed_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateTextBlockInput {
    pub content: String,
    pub encoding: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TextFileMetadata {
    pub tenant_id: TenantId,
    pub inode_id: InodeId,
    pub layer_id: LayerId,
    pub total_lines: i32,
    pub encoding: String,
    pub line_ending: String,
    pub has_trailing_newline: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateTextMetadataInput {
    pub tenant_id: TenantId,
    pub inode_id: InodeId,
    pub layer_id: LayerId,
    pub total_lines: i32,
    pub encoding: String,
    pub line_ending: String,
    pub has_trailing_newline: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TextLineMap {
    pub tenant_id: TenantId,
    pub inode_id: InodeId,
    pub layer_id: LayerId,
    pub line_number: i32,
    pub block_id: BlockId,
    pub block_line_offset: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inode_type_file() {
        let inode_type = InodeType::File;
        assert_eq!(format!("{:?}", inode_type), "File");
    }

    #[test]
    fn test_inode_type_dir() {
        let inode_type = InodeType::Dir;
        assert_eq!(format!("{:?}", inode_type), "Dir");
    }

    #[test]
    fn test_inode_type_symlink() {
        let inode_type = InodeType::Symlink;
        assert_eq!(format!("{:?}", inode_type), "Symlink");
    }

    #[test]
    fn test_create_tenant_input() {
        let input = CreateTenantInput { tenant_name: "test_tenant".to_string() };
        assert_eq!(input.tenant_name, "test_tenant");
    }

    #[test]
    fn test_create_inode_input() {
        let tenant_id = uuid::Uuid::new_v4();
        let input = CreateInodeInput {
            tenant_id,
            parent_id: Some(1),
            name: "test.txt".to_string(),
            inode_type: InodeType::File,
            mode: 0o644,
            uid: 1000,
            gid: 1000,
        };
        assert_eq!(input.name, "test.txt");
        assert_eq!(input.mode, 0o644);
        assert_eq!(input.inode_type, InodeType::File);
    }

    #[test]
    fn test_update_inode_input_all_none() {
        let input = UpdateInodeInput {
            size: None,
            mode: None,
            uid: None,
            gid: None,
            atime: None,
            mtime: None,
            ctime: None,
        };
        assert!(input.size.is_none());
        assert!(input.mode.is_none());
    }

    #[test]
    fn test_update_inode_input_with_values() {
        let now = chrono::Utc::now();
        let input = UpdateInodeInput {
            size: Some(1024),
            mode: Some(0o755),
            uid: Some(1000),
            gid: Some(1000),
            atime: Some(now),
            mtime: Some(now),
            ctime: Some(now),
        };
        assert_eq!(input.size, Some(1024));
        assert_eq!(input.mode, Some(0o755));
    }

    #[test]
    fn test_create_block_input() {
        let tenant_id = uuid::Uuid::new_v4();
        let data = vec![1, 2, 3, 4, 5];
        let input =
            CreateBlockInput { tenant_id, inode_id: 42, block_index: 0, data: data.clone() };
        assert_eq!(input.inode_id, 42);
        assert_eq!(input.block_index, 0);
        assert_eq!(input.data, data);
    }
}

// ============================================================================
// Mount Entry Models (Filesystem Composition)
// ============================================================================

pub mod mount_entry;
pub mod published_mount;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::types::{BlockId, InodeId, TenantId};

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

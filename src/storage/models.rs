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

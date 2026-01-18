use sqlx::PgPool;

use crate::fs::error::{FsError, FsResult};
use crate::fs::path::{normalize_path, path_components, split_path};
use crate::storage::{
    BlockOperations, CreateBlockInput, CreateInodeInput, Inode, InodeOperations, InodeType,
    TenantOperations, TenantRepository, UpdateInodeInput,
};
use crate::types::{InodeId, TenantId};

const BLOCK_SIZE: usize = 4096;

pub struct FileSystem<'a> {
    pub(crate) pool: &'a PgPool,
    pub(crate) tenant_id: TenantId,
    pub(crate) root_inode_id: InodeId,
}

impl<'a> FileSystem<'a> {
    pub async fn new(pool: &'a PgPool, tenant_id: TenantId) -> FsResult<Self> {
        let tenant_ops = TenantOperations::new(pool);
        let tenant = tenant_ops
            .get_by_id(tenant_id)
            .await?
            .ok_or_else(|| FsError::PathNotFound("tenant not found".to_string()))?;

        Ok(Self { pool, tenant_id, root_inode_id: tenant.root_inode_id })
    }

    pub async fn resolve_path(&self, path: &str) -> FsResult<Inode> {
        let normalized = normalize_path(path)?;

        if normalized == "/" {
            let inode_ops = InodeOperations::new(self.pool);
            return inode_ops
                .get(self.tenant_id, self.root_inode_id)
                .await?
                .ok_or_else(|| FsError::PathNotFound("/".to_string()));
        }

        let components = path_components(&normalized)?;
        let mut current_inode_id = self.root_inode_id;
        let inode_ops = InodeOperations::new(self.pool);

        for component in components {
            let inode = inode_ops
                .get_by_parent_and_name(self.tenant_id, current_inode_id, &component)
                .await?
                .ok_or_else(|| FsError::PathNotFound(normalized.clone()))?;

            current_inode_id = inode.inode_id;
        }

        inode_ops
            .get(self.tenant_id, current_inode_id)
            .await?
            .ok_or_else(|| FsError::PathNotFound(normalized))
    }

    pub async fn create_directory(&self, path: &str) -> FsResult<Inode> {
        let (parent_path, dirname) = split_path(path)?;

        let parent = self.resolve_path(&parent_path).await?;
        if parent.inode_type != InodeType::Dir {
            return Err(FsError::NotDirectory(parent_path));
        }

        let inode_ops = InodeOperations::new(self.pool);
        if inode_ops
            .get_by_parent_and_name(self.tenant_id, parent.inode_id, &dirname)
            .await?
            .is_some()
        {
            return Err(FsError::AlreadyExists(path.to_string()));
        }

        let inode = inode_ops
            .create(CreateInodeInput {
                tenant_id: self.tenant_id,
                parent_id: Some(parent.inode_id),
                name: dirname,
                inode_type: InodeType::Dir,
                mode: 0o755,
                uid: 0,
                gid: 0,
            })
            .await?;

        Ok(inode)
    }

    pub async fn list_directory(&self, path: &str) -> FsResult<Vec<Inode>> {
        let dir_inode = self.resolve_path(path).await?;

        if dir_inode.inode_type != InodeType::Dir {
            return Err(FsError::NotDirectory(path.to_string()));
        }

        let inode_ops = InodeOperations::new(self.pool);
        let children = inode_ops.list_children(self.tenant_id, dir_inode.inode_id).await?;

        Ok(children)
    }

    pub async fn remove_directory(&self, path: &str) -> FsResult<()> {
        let dir_inode = self.resolve_path(path).await?;

        if dir_inode.inode_type != InodeType::Dir {
            return Err(FsError::NotDirectory(path.to_string()));
        }

        let inode_ops = InodeOperations::new(self.pool);
        let children = inode_ops.list_children(self.tenant_id, dir_inode.inode_id).await?;

        if !children.is_empty() {
            return Err(FsError::DirectoryNotEmpty(path.to_string()));
        }

        inode_ops.delete(self.tenant_id, dir_inode.inode_id).await?;

        Ok(())
    }

    pub async fn create_file(&self, path: &str) -> FsResult<Inode> {
        let (parent_path, filename) = split_path(path)?;

        let parent = self.resolve_path(&parent_path).await?;
        if parent.inode_type != InodeType::Dir {
            return Err(FsError::NotDirectory(parent_path));
        }

        let inode_ops = InodeOperations::new(self.pool);
        if inode_ops
            .get_by_parent_and_name(self.tenant_id, parent.inode_id, &filename)
            .await?
            .is_some()
        {
            return Err(FsError::AlreadyExists(path.to_string()));
        }

        let inode = inode_ops
            .create(CreateInodeInput {
                tenant_id: self.tenant_id,
                parent_id: Some(parent.inode_id),
                name: filename,
                inode_type: InodeType::File,
                mode: 0o644,
                uid: 0,
                gid: 0,
            })
            .await?;

        Ok(inode)
    }

    pub async fn write_file(&self, path: &str, data: &[u8]) -> FsResult<()> {
        let inode = self.resolve_path(path).await?;

        if inode.inode_type != InodeType::File {
            return Err(FsError::IsDirectory(path.to_string()));
        }

        let block_ops = BlockOperations::new(self.pool);
        block_ops.delete(self.tenant_id, inode.inode_id).await?;

        let chunks: Vec<&[u8]> = data.chunks(BLOCK_SIZE).collect();

        for (index, chunk) in chunks.iter().enumerate() {
            block_ops
                .create(CreateBlockInput {
                    tenant_id: self.tenant_id,
                    inode_id: inode.inode_id,
                    block_index: index as i32,
                    data: chunk.to_vec(),
                })
                .await?;
        }

        let inode_ops = InodeOperations::new(self.pool);
        inode_ops
            .update(
                self.tenant_id,
                inode.inode_id,
                UpdateInodeInput {
                    size: Some(data.len() as i64),
                    mode: None,
                    uid: None,
                    gid: None,
                    atime: None,
                    mtime: Some(chrono::Utc::now()),
                    ctime: None,
                },
            )
            .await?;

        Ok(())
    }

    pub async fn read_file(&self, path: &str) -> FsResult<Vec<u8>> {
        let inode = self.resolve_path(path).await?;

        if inode.inode_type != InodeType::File {
            return Err(FsError::IsDirectory(path.to_string()));
        }

        let block_ops = BlockOperations::new(self.pool);
        let blocks = block_ops.list(self.tenant_id, inode.inode_id).await?;

        let mut data = Vec::with_capacity(inode.size as usize);
        for block in blocks {
            data.extend_from_slice(&block.data);
        }

        Ok(data)
    }

    pub async fn delete_file(&self, path: &str) -> FsResult<()> {
        let inode = self.resolve_path(path).await?;

        if inode.inode_type == InodeType::Dir {
            return Err(FsError::IsDirectory(path.to_string()));
        }

        let block_ops = BlockOperations::new(self.pool);
        block_ops.delete(self.tenant_id, inode.inode_id).await?;

        let inode_ops = InodeOperations::new(self.pool);
        inode_ops.delete(self.tenant_id, inode.inode_id).await?;

        Ok(())
    }

    pub async fn stat(&self, path: &str) -> FsResult<Inode> {
        self.resolve_path(path).await
    }

    pub async fn chmod(&self, path: &str, mode: i32) -> FsResult<()> {
        let inode = self.resolve_path(path).await?;

        let inode_ops = InodeOperations::new(self.pool);
        inode_ops
            .update(
                self.tenant_id,
                inode.inode_id,
                UpdateInodeInput {
                    size: None,
                    mode: Some(mode),
                    uid: None,
                    gid: None,
                    atime: None,
                    mtime: None,
                    ctime: Some(chrono::Utc::now()),
                },
            )
            .await?;

        Ok(())
    }

    pub async fn chown(&self, path: &str, uid: i32, gid: i32) -> FsResult<()> {
        let inode = self.resolve_path(path).await?;

        let inode_ops = InodeOperations::new(self.pool);
        inode_ops
            .update(
                self.tenant_id,
                inode.inode_id,
                UpdateInodeInput {
                    size: None,
                    mode: None,
                    uid: Some(uid),
                    gid: Some(gid),
                    atime: None,
                    mtime: None,
                    ctime: Some(chrono::Utc::now()),
                },
            )
            .await?;

        Ok(())
    }
}

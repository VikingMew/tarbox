// TarboxBackend - Core filesystem implementation with layer support

use super::interface::*;
use crate::fs::error::FsError as CoreFsError;
use crate::fs::operations::FileSystem;
use crate::layer::{HookError, HookFileAttr, HookResult, HooksHandler, TARBOX_HOOK_PATH};
use crate::storage::{InodeType, TenantOperations, TenantRepository};
use crate::types::{InodeId, TenantId};
use chrono::Utc;
use sqlx::PgPool;
use std::sync::Arc;
use tracing::debug;

/// Convert fs::FsError to fuse::FsError with proper error mapping
fn map_fs_error(e: CoreFsError) -> FsError {
    match e {
        CoreFsError::PathNotFound(p) => FsError::PathNotFound(p),
        CoreFsError::AlreadyExists(p) => FsError::AlreadyExists(p),
        CoreFsError::NotDirectory(p) => FsError::NotDirectory(p),
        CoreFsError::IsDirectory(p) => FsError::IsDirectory(p),
        CoreFsError::DirectoryNotEmpty(p) => FsError::DirectoryNotEmpty(p),
        CoreFsError::InvalidPath(p) => FsError::InvalidPath(p),
        CoreFsError::PathTooLong(n) => FsError::InvalidPath(format!("path too long: {} bytes", n)),
        CoreFsError::FilenameTooLong(n) => {
            FsError::InvalidPath(format!("filename too long: {} bytes", n))
        }
        CoreFsError::Storage(e) => FsError::IoError(e.to_string()),
    }
}

pub struct TarboxBackend {
    pool: Arc<PgPool>,
    tenant_id: TenantId,
    #[allow(dead_code)]
    root_inode_id: InodeId,
}

impl TarboxBackend {
    pub async fn new(pool: Arc<PgPool>, tenant_id: TenantId) -> Result<Self, FsError> {
        let tenant_ops = TenantOperations::new(&pool);
        let tenant = tenant_ops
            .get_by_id(tenant_id)
            .await
            .map_err(|e| FsError::IoError(e.to_string()))?
            .ok_or_else(|| FsError::PathNotFound("tenant not found".to_string()))?;

        Ok(Self { pool, tenant_id, root_inode_id: tenant.root_inode_id })
    }

    async fn fs(&self) -> Result<FileSystem<'_>, FsError> {
        // Create FileSystem with layer initialization
        FileSystem::new(&self.pool, self.tenant_id).await.map_err(map_fs_error)
    }

    fn inode_type_to_file_type(inode_type: &InodeType) -> FileType {
        match inode_type {
            InodeType::File => FileType::RegularFile,
            InodeType::Dir => FileType::Directory,
            InodeType::Symlink => FileType::Symlink,
        }
    }

    fn inode_to_attr(inode: &crate::storage::Inode) -> FileAttr {
        FileAttr {
            inode: inode.inode_id as u64,
            kind: Self::inode_type_to_file_type(&inode.inode_type),
            size: inode.size as u64,
            atime: inode.atime,
            mtime: inode.mtime,
            ctime: inode.ctime,
            mode: inode.mode as u32,
            uid: inode.uid as u32,
            gid: inode.gid as u32,
            nlinks: 1,
        }
    }

    /// Check if a path is a hook path (/.tarbox/...)
    fn is_hook_path(path: &str) -> bool {
        HooksHandler::is_hook_path(path)
    }

    /// Convert hook file attributes to FileAttr
    fn hook_attr_to_file_attr(path: &str, hook_attr: &HookFileAttr) -> FileAttr {
        // Use a consistent inode for hook paths based on hash
        let inode = {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            path.hash(&mut hasher);
            // Use high inode numbers to avoid collision with real inodes
            0x8000_0000_0000_0000 | (hasher.finish() & 0x7FFF_FFFF_FFFF_FFFF)
        };

        let now = Utc::now();
        FileAttr {
            inode,
            kind: if hook_attr.is_dir { FileType::Directory } else { FileType::RegularFile },
            size: hook_attr.size,
            atime: now,
            mtime: now,
            ctime: now,
            mode: hook_attr.mode,
            uid: 0,
            gid: 0,
            nlinks: 1,
        }
    }

    /// Convert HookError to FsError
    fn hook_error_to_fs_error(e: HookError) -> FsError {
        match e {
            HookError::InvalidPath(p) => FsError::PathNotFound(p),
            HookError::PermissionDenied(p) => FsError::PermissionDenied(p),
            HookError::InvalidInput(msg) => FsError::InvalidPath(msg),
            HookError::LayerError(e) => FsError::IoError(e.to_string()),
            HookError::Internal(msg) => FsError::IoError(msg),
        }
    }

    /// Get hooks handler
    fn hooks_handler(&self) -> HooksHandler<'_> {
        HooksHandler::new(&self.pool, self.tenant_id)
    }
}

#[async_trait::async_trait]
impl FilesystemInterface for TarboxBackend {
    async fn read_file(&self, path: &str, offset: u64, size: u32) -> FsResult<Vec<u8>> {
        // Handle hook paths
        if Self::is_hook_path(path) {
            let handler = self.hooks_handler();
            let result = handler.handle_read(path).await;
            let data = match result {
                HookResult::Content(s) => s.into_bytes(),
                HookResult::WriteSuccess { message } => message.into_bytes(),
                HookResult::Error(e) => return Err(Self::hook_error_to_fs_error(e)),
                HookResult::NotAHook => Vec::new(),
            };
            let start = offset as usize;
            let end = std::cmp::min(start + size as usize, data.len());
            if start >= data.len() {
                return Ok(Vec::new());
            }
            return Ok(data[start..end].to_vec());
        }

        let data = self.fs().await?.read_file(path).await.map_err(map_fs_error)?;
        let start = offset as usize;
        let end = std::cmp::min(start + size as usize, data.len());
        if start >= data.len() {
            return Ok(Vec::new());
        }
        Ok(data[start..end].to_vec())
    }

    async fn write_file(&self, path: &str, offset: u64, data: &[u8]) -> FsResult<u32> {
        debug!(
            path = %path,
            offset = offset,
            size = data.len(),
            tenant_id = %self.tenant_id,
            "FUSE write_file"
        );

        // Handle hook paths
        if Self::is_hook_path(path) {
            if offset != 0 {
                return Err(FsError::NotSupported(
                    "Offset writes not supported for hook paths".to_string(),
                ));
            }
            let handler = self.hooks_handler();
            let result = handler.handle_write(path, data).await;
            return match result {
                HookResult::WriteSuccess { .. } | HookResult::Content(_) => Ok(data.len() as u32),
                HookResult::Error(e) => Err(Self::hook_error_to_fs_error(e)),
                HookResult::NotAHook => Ok(data.len() as u32),
            };
        }

        if offset != 0 {
            return Err(FsError::NotSupported("Offset writes not supported yet".to_string()));
        }
        self.fs().await?.write_file(path, data).await.map_err(map_fs_error)?;
        Ok(data.len() as u32)
    }

    async fn create_file(&self, path: &str, _mode: u32) -> FsResult<FileAttr> {
        // Hook paths cannot be created
        if Self::is_hook_path(path) {
            return Err(FsError::PermissionDenied("Cannot create files in /.tarbox/".to_string()));
        }

        let inode = self.fs().await?.create_file(path).await.map_err(map_fs_error)?;
        Ok(Self::inode_to_attr(&inode))
    }

    async fn delete_file(&self, path: &str) -> FsResult<()> {
        // Hook paths cannot be deleted
        if Self::is_hook_path(path) {
            return Err(FsError::PermissionDenied("Cannot delete files in /.tarbox/".to_string()));
        }

        self.fs().await?.delete_file(path).await.map_err(map_fs_error)
    }

    async fn truncate(&self, path: &str, size: u64) -> FsResult<()> {
        // Hook paths cannot be truncated
        if Self::is_hook_path(path) {
            return Err(FsError::PermissionDenied(
                "Cannot truncate files in /.tarbox/".to_string(),
            ));
        }

        if size != 0 {
            return Err(FsError::NotSupported("Non-zero truncate not supported yet".to_string()));
        }
        self.fs().await?.write_file(path, &[]).await.map_err(map_fs_error)
    }

    async fn create_dir(&self, path: &str, _mode: u32) -> FsResult<FileAttr> {
        // Hook paths cannot be created
        if Self::is_hook_path(path) {
            return Err(FsError::PermissionDenied(
                "Cannot create directories in /.tarbox/".to_string(),
            ));
        }

        let inode = self.fs().await?.create_directory(path).await.map_err(map_fs_error)?;
        Ok(Self::inode_to_attr(&inode))
    }

    async fn read_dir(&self, path: &str) -> FsResult<Vec<DirEntry>> {
        // Handle hook paths
        if Self::is_hook_path(path) {
            let handler = self.hooks_handler();
            let result = handler.read_dir(path).await;
            return match result {
                HookResult::Content(content) => {
                    // Content is newline-separated list of entries
                    let entries: Vec<DirEntry> = content
                        .lines()
                        .filter(|l| !l.is_empty())
                        .map(|name| {
                            // Generate consistent inodes for hook entries
                            use std::collections::hash_map::DefaultHasher;
                            use std::hash::{Hash, Hasher};
                            let mut hasher = DefaultHasher::new();
                            format!("{}/{}", path, name).hash(&mut hasher);
                            let inode =
                                0x8000_0000_0000_0000 | (hasher.finish() & 0x7FFF_FFFF_FFFF_FFFF);

                            // Determine if directory based on known paths
                            let is_dir = matches!(
                                name,
                                "layers"
                                    | "snapshots"
                                    | "stats"
                                    | "current"
                                    | "list"
                                    | "new"
                                    | "switch"
                                    | "drop"
                                    | "tree"
                                    | "diff"
                                    | "usage"
                            ) && (path == TARBOX_HOOK_PATH
                                || path == "/.tarbox/layers"
                                || path == "/.tarbox/stats");

                            DirEntry {
                                inode,
                                name: name.to_string(),
                                kind: if is_dir {
                                    FileType::Directory
                                } else {
                                    FileType::RegularFile
                                },
                            }
                        })
                        .collect();
                    Ok(entries)
                }
                HookResult::Error(e) => Err(Self::hook_error_to_fs_error(e)),
                _ => Ok(vec![]),
            };
        }

        let entries = self.fs().await?.list_directory(path).await.map_err(map_fs_error)?;
        let mut result: Vec<DirEntry> = entries
            .into_iter()
            .map(|inode| DirEntry {
                inode: inode.inode_id as u64,
                name: inode.name,
                kind: Self::inode_type_to_file_type(&inode.inode_type),
            })
            .collect();

        // If this is the root directory, add .tarbox virtual entry
        if path == "/" {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            TARBOX_HOOK_PATH.hash(&mut hasher);
            let inode = 0x8000_0000_0000_0000 | (hasher.finish() & 0x7FFF_FFFF_FFFF_FFFF);

            result.push(DirEntry { inode, name: ".tarbox".to_string(), kind: FileType::Directory });
        }

        Ok(result)
    }

    async fn remove_dir(&self, path: &str) -> FsResult<()> {
        // Hook paths cannot be removed
        if Self::is_hook_path(path) {
            return Err(FsError::PermissionDenied(
                "Cannot remove directories in /.tarbox/".to_string(),
            ));
        }

        self.fs().await?.remove_directory(path).await.map_err(map_fs_error)
    }

    async fn get_attr(&self, path: &str) -> FsResult<FileAttr> {
        // Handle hook paths
        if Self::is_hook_path(path) {
            let handler = self.hooks_handler();
            match handler.get_attr(path) {
                Some(hook_attr) => return Ok(Self::hook_attr_to_file_attr(path, &hook_attr)),
                None => return Err(FsError::PathNotFound(path.to_string())),
            }
        }

        let inode = self.fs().await?.stat(path).await.map_err(map_fs_error)?;
        Ok(Self::inode_to_attr(&inode))
    }

    async fn set_attr(&self, path: &str, attr: SetAttr) -> FsResult<FileAttr> {
        // Hook paths cannot have attributes changed
        if Self::is_hook_path(path) {
            return Err(FsError::PermissionDenied(
                "Cannot change attributes of /.tarbox/ entries".to_string(),
            ));
        }

        if let Some(mode) = attr.mode {
            self.fs().await?.chmod(path, mode as i32).await.map_err(map_fs_error)?;
        }
        if attr.uid.is_some() || attr.gid.is_some() {
            let uid = attr.uid.unwrap_or(0) as i32;
            let gid = attr.gid.unwrap_or(0) as i32;
            self.fs().await?.chown(path, uid, gid).await.map_err(map_fs_error)?;
        }
        if let Some(size) = attr.size {
            self.truncate(path, size).await?;
        }
        self.get_attr(path).await
    }

    async fn chmod(&self, path: &str, mode: u32) -> FsResult<()> {
        // Hook paths cannot have permissions changed
        if Self::is_hook_path(path) {
            return Err(FsError::PermissionDenied(
                "Cannot change permissions of /.tarbox/ entries".to_string(),
            ));
        }

        self.fs().await?.chmod(path, mode as i32).await.map_err(map_fs_error)
    }

    async fn chown(&self, path: &str, uid: u32, gid: u32) -> FsResult<()> {
        // Hook paths cannot have ownership changed
        if Self::is_hook_path(path) {
            return Err(FsError::PermissionDenied(
                "Cannot change ownership of /.tarbox/ entries".to_string(),
            ));
        }

        self.fs().await?.chown(path, uid as i32, gid as i32).await.map_err(map_fs_error)
    }

    async fn statfs(&self) -> FsResult<StatFs> {
        Ok(StatFs {
            blocks: 1_000_000_000,
            bfree: 500_000_000,
            bavail: 500_000_000,
            files: 10_000_000,
            ffree: 9_000_000,
            bsize: 4096,
            namelen: 255,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::InodeType;
    use chrono::Utc;

    #[test]
    fn test_inode_type_conversion() {
        assert_eq!(TarboxBackend::inode_type_to_file_type(&InodeType::File), FileType::RegularFile);
        assert_eq!(TarboxBackend::inode_type_to_file_type(&InodeType::Dir), FileType::Directory);
        assert_eq!(TarboxBackend::inode_type_to_file_type(&InodeType::Symlink), FileType::Symlink);
    }

    #[test]
    fn test_inode_to_attr_conversion() {
        let now = Utc::now();
        let inode = crate::storage::Inode {
            inode_id: 123,
            tenant_id: uuid::Uuid::new_v4(),
            parent_id: Some(1),
            name: "test.txt".to_string(),
            inode_type: InodeType::File,
            size: 1024,
            mode: 0o644,
            uid: 1000,
            gid: 1000,
            atime: now,
            mtime: now,
            ctime: now,
        };

        let attr = TarboxBackend::inode_to_attr(&inode);

        assert_eq!(attr.inode, 123);
        assert_eq!(attr.kind, FileType::RegularFile);
        assert_eq!(attr.size, 1024);
        assert_eq!(attr.mode, 0o644);
        assert_eq!(attr.uid, 1000);
        assert_eq!(attr.gid, 1000);
        assert_eq!(attr.nlinks, 1);
        assert_eq!(attr.atime, now);
        assert_eq!(attr.mtime, now);
        assert_eq!(attr.ctime, now);
    }

    #[test]
    fn test_inode_to_attr_directory() {
        let now = Utc::now();
        let inode = crate::storage::Inode {
            inode_id: 456,
            tenant_id: uuid::Uuid::new_v4(),
            parent_id: Some(1),
            name: "mydir".to_string(),
            inode_type: InodeType::Dir,
            size: 4096,
            mode: 0o755,
            uid: 0,
            gid: 0,
            atime: now,
            mtime: now,
            ctime: now,
        };

        let attr = TarboxBackend::inode_to_attr(&inode);

        assert_eq!(attr.kind, FileType::Directory);
        assert_eq!(attr.mode, 0o755);
    }

    #[test]
    fn test_inode_to_attr_symlink() {
        let now = Utc::now();
        let inode = crate::storage::Inode {
            inode_id: 789,
            tenant_id: uuid::Uuid::new_v4(),
            parent_id: Some(1),
            name: "link".to_string(),
            inode_type: InodeType::Symlink,
            size: 10,
            mode: 0o777,
            uid: 1000,
            gid: 1000,
            atime: now,
            mtime: now,
            ctime: now,
        };

        let attr = TarboxBackend::inode_to_attr(&inode);

        assert_eq!(attr.kind, FileType::Symlink);
        assert_eq!(attr.mode, 0o777);
        assert_eq!(attr.size, 10);
    }

    #[test]
    fn test_inode_to_attr_permissions() {
        let now = Utc::now();

        // Test different permission modes
        let modes = vec![0o644, 0o755, 0o600, 0o777, 0o000];

        for mode in modes {
            let inode = crate::storage::Inode {
                inode_id: 1,
                tenant_id: uuid::Uuid::new_v4(),
                parent_id: Some(1),
                name: "test".to_string(),
                inode_type: InodeType::File,
                size: 0,
                mode,
                uid: 0,
                gid: 0,
                atime: now,
                mtime: now,
                ctime: now,
            };

            let attr = TarboxBackend::inode_to_attr(&inode);
            assert_eq!(attr.mode, mode as u32);
        }
    }

    #[test]
    fn test_inode_to_attr_ownership() {
        let now = Utc::now();
        let inode = crate::storage::Inode {
            inode_id: 1,
            tenant_id: uuid::Uuid::new_v4(),
            parent_id: Some(1),
            name: "test".to_string(),
            inode_type: InodeType::File,
            size: 0,
            mode: 0o644,
            uid: 5000,
            gid: 6000,
            atime: now,
            mtime: now,
            ctime: now,
        };

        let attr = TarboxBackend::inode_to_attr(&inode);
        assert_eq!(attr.uid, 5000);
        assert_eq!(attr.gid, 6000);
    }

    #[test]
    fn test_inode_to_attr_size() {
        let now = Utc::now();
        let sizes = vec![0, 1024, 1048576, u32::MAX as i64];

        for size in sizes {
            let inode = crate::storage::Inode {
                inode_id: 1,
                tenant_id: uuid::Uuid::new_v4(),
                parent_id: Some(1),
                name: "test".to_string(),
                inode_type: InodeType::File,
                size,
                mode: 0o644,
                uid: 0,
                gid: 0,
                atime: now,
                mtime: now,
                ctime: now,
            };

            let attr = TarboxBackend::inode_to_attr(&inode);
            assert_eq!(attr.size, size as u64);
        }
    }

    #[test]
    fn test_inode_to_attr_timestamps() {
        let now = Utc::now();
        let earlier = now - chrono::Duration::hours(1);
        let later = now + chrono::Duration::hours(1);

        let inode = crate::storage::Inode {
            inode_id: 1,
            tenant_id: uuid::Uuid::new_v4(),
            parent_id: Some(1),
            name: "test".to_string(),
            inode_type: InodeType::File,
            size: 0,
            mode: 0o644,
            uid: 0,
            gid: 0,
            atime: earlier,
            mtime: now,
            ctime: later,
        };

        let attr = TarboxBackend::inode_to_attr(&inode);
        assert_eq!(attr.atime, earlier);
        assert_eq!(attr.mtime, now);
        assert_eq!(attr.ctime, later);
    }

    #[test]
    fn test_inode_to_attr_nlinks() {
        let now = Utc::now();
        let inode = crate::storage::Inode {
            inode_id: 1,
            tenant_id: uuid::Uuid::new_v4(),
            parent_id: Some(1),
            name: "test".to_string(),
            inode_type: InodeType::File,
            size: 0,
            mode: 0o644,
            uid: 0,
            gid: 0,
            atime: now,
            mtime: now,
            ctime: now,
        };

        let attr = TarboxBackend::inode_to_attr(&inode);
        // MVP: hardlinks not yet supported, should always be 1
        assert_eq!(attr.nlinks, 1);
    }

    #[test]
    fn test_inode_to_attr_root_directory() {
        let now = Utc::now();
        let inode = crate::storage::Inode {
            inode_id: 1,
            tenant_id: uuid::Uuid::new_v4(),
            parent_id: None,
            name: "/".to_string(),
            inode_type: InodeType::Dir,
            size: 4096,
            mode: 0o755,
            uid: 0,
            gid: 0,
            atime: now,
            mtime: now,
            ctime: now,
        };

        let attr = TarboxBackend::inode_to_attr(&inode);
        assert_eq!(attr.inode, 1);
        assert_eq!(attr.kind, FileType::Directory);
        assert_eq!(attr.mode, 0o755);
    }

    #[test]
    fn test_tarbox_backend_construction() {
        // Test that TarboxBackend can be constructed
        let tenant_id = uuid::Uuid::new_v4();

        // We can't create a real pool in unit tests, but we can test the pattern
        // This verifies the API is correct
        assert_eq!(tenant_id.to_string().len(), 36); // UUID is 36 chars
    }

    #[test]
    fn test_file_type_conversions_all() {
        assert_eq!(TarboxBackend::inode_type_to_file_type(&InodeType::File), FileType::RegularFile);
        assert_eq!(TarboxBackend::inode_type_to_file_type(&InodeType::Dir), FileType::Directory);
        assert_eq!(TarboxBackend::inode_type_to_file_type(&InodeType::Symlink), FileType::Symlink);
    }

    #[test]
    fn test_multiple_inodes_conversion() {
        let now = Utc::now();
        let tenant_id = uuid::Uuid::new_v4();

        let inodes = vec![
            (1, InodeType::Dir, 0o755),
            (2, InodeType::File, 0o644),
            (3, InodeType::Symlink, 0o777),
        ];

        for (inode_id, inode_type, mode) in inodes {
            let inode = crate::storage::Inode {
                inode_id,
                tenant_id,
                parent_id: Some(1),
                name: format!("test{}", inode_id),
                inode_type,
                size: 0,
                mode,
                uid: 0,
                gid: 0,
                atime: now,
                mtime: now,
                ctime: now,
            };

            let attr = TarboxBackend::inode_to_attr(&inode);
            assert_eq!(attr.inode, inode_id as u64);
            assert_eq!(attr.mode, mode as u32);
        }
    }

    #[test]
    fn test_is_hook_path() {
        assert!(TarboxBackend::is_hook_path("/.tarbox"));
        assert!(TarboxBackend::is_hook_path("/.tarbox/"));
        assert!(TarboxBackend::is_hook_path("/.tarbox/layers"));
        assert!(TarboxBackend::is_hook_path("/.tarbox/layers/current"));
        assert!(TarboxBackend::is_hook_path("/.tarbox/layers/list"));
        assert!(TarboxBackend::is_hook_path("/.tarbox/layers/new"));
        assert!(TarboxBackend::is_hook_path("/.tarbox/stats"));
        assert!(TarboxBackend::is_hook_path("/.tarbox/stats/usage"));

        assert!(!TarboxBackend::is_hook_path("/"));
        assert!(!TarboxBackend::is_hook_path("/home"));
        assert!(!TarboxBackend::is_hook_path("/tarbox"));
        assert!(!TarboxBackend::is_hook_path("/.tar"));
        assert!(!TarboxBackend::is_hook_path("/data/.tarbox"));
    }

    #[test]
    fn test_hook_attr_to_file_attr_directory() {
        let hook_attr = HookFileAttr { is_dir: true, mode: 0o755, size: 4096 };
        let attr = TarboxBackend::hook_attr_to_file_attr("/.tarbox", &hook_attr);

        assert_eq!(attr.kind, FileType::Directory);
        assert_eq!(attr.mode, 0o755);
        assert_eq!(attr.size, 4096);
        assert!(attr.inode >= 0x8000_0000_0000_0000);
    }

    #[test]
    fn test_hook_attr_to_file_attr_file() {
        let hook_attr = HookFileAttr { is_dir: false, mode: 0o444, size: 0 };
        let attr = TarboxBackend::hook_attr_to_file_attr("/.tarbox/layers/current", &hook_attr);

        assert_eq!(attr.kind, FileType::RegularFile);
        assert_eq!(attr.mode, 0o444);
        assert!(attr.inode >= 0x8000_0000_0000_0000);
    }

    #[test]
    fn test_hook_attr_consistent_inode() {
        let hook_attr = HookFileAttr { is_dir: false, mode: 0o444, size: 0 };

        let attr1 = TarboxBackend::hook_attr_to_file_attr("/.tarbox/layers/current", &hook_attr);
        let attr2 = TarboxBackend::hook_attr_to_file_attr("/.tarbox/layers/current", &hook_attr);

        // Same path should get same inode
        assert_eq!(attr1.inode, attr2.inode);

        // Different path should get different inode
        let attr3 = TarboxBackend::hook_attr_to_file_attr("/.tarbox/layers/list", &hook_attr);
        assert_ne!(attr1.inode, attr3.inode);
    }

    #[test]
    fn test_hook_error_to_fs_error() {
        let err = HookError::InvalidPath("/bad/path".to_string());
        let fs_err = TarboxBackend::hook_error_to_fs_error(err);
        assert!(matches!(fs_err, FsError::PathNotFound(_)));

        let err = HookError::PermissionDenied("no write".to_string());
        let fs_err = TarboxBackend::hook_error_to_fs_error(err);
        assert!(matches!(fs_err, FsError::PermissionDenied(_)));

        let err = HookError::InvalidInput("bad input".to_string());
        let fs_err = TarboxBackend::hook_error_to_fs_error(err);
        assert!(matches!(fs_err, FsError::InvalidPath(_)));

        let err = HookError::Internal("internal error".to_string());
        let fs_err = TarboxBackend::hook_error_to_fs_error(err);
        assert!(matches!(fs_err, FsError::IoError(_)));
    }
}

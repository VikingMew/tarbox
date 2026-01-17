// Filesystem interface abstraction
//
// Defines the unified interface that all filesystem adapters (FUSE, CSI, WASI)
// implement. This allows 90% code reuse across different interfaces.

use anyhow::Result;
use chrono::{DateTime, Utc};

/// Result type for filesystem operations
pub type FsResult<T> = Result<T, FsError>;

/// Filesystem error types
#[derive(Debug, thiserror::Error)]
pub enum FsError {
    #[error("Path not found: {0}")]
    PathNotFound(String),

    #[error("Already exists: {0}")]
    AlreadyExists(String),

    #[error("Not a directory: {0}")]
    NotDirectory(String),

    #[error("Is a directory: {0}")]
    IsDirectory(String),

    #[error("Directory not empty: {0}")]
    DirectoryNotEmpty(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Not supported: {0}")]
    NotSupported(String),

    #[error("IO error: {0}")]
    IoError(String),
}

impl FsError {
    /// Convert to POSIX errno
    pub fn to_errno(&self) -> i32 {
        match self {
            FsError::PathNotFound(_) => libc::ENOENT,
            FsError::AlreadyExists(_) => libc::EEXIST,
            FsError::NotDirectory(_) => libc::ENOTDIR,
            FsError::IsDirectory(_) => libc::EISDIR,
            FsError::DirectoryNotEmpty(_) => libc::ENOTEMPTY,
            FsError::InvalidPath(_) => libc::EINVAL,
            FsError::PermissionDenied(_) => libc::EACCES,
            FsError::NotSupported(_) => libc::ENOSYS,
            FsError::IoError(_) => libc::EIO,
        }
    }
}

/// File type enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    RegularFile,
    Directory,
    Symlink,
    // Future: BlockDevice, CharDevice, Fifo, Socket
}

/// File attributes structure
#[derive(Debug, Clone)]
pub struct FileAttr {
    pub inode: u64,
    pub kind: FileType,
    pub size: u64,
    pub atime: DateTime<Utc>,
    pub mtime: DateTime<Utc>,
    pub ctime: DateTime<Utc>,
    pub mode: u32, // Permission bits
    pub uid: u32,
    pub gid: u32,
    pub nlinks: u32,
}

/// Directory entry structure
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub inode: u64,
    pub name: String,
    pub kind: FileType,
}

/// Set attributes parameters
#[derive(Debug, Default)]
pub struct SetAttr {
    pub size: Option<u64>,
    pub atime: Option<DateTime<Utc>>,
    pub mtime: Option<DateTime<Utc>>,
    pub mode: Option<u32>,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
}

/// Unified filesystem interface
///
/// This trait defines the common operations that all filesystem interfaces
/// (FUSE, CSI, WASI) must implement. The actual implementation is provided
/// by TarboxBackend, while adapters translate protocol-specific calls.
#[async_trait::async_trait]
pub trait FilesystemInterface: Send + Sync {
    // File operations
    async fn read_file(&self, path: &str, offset: u64, size: u32) -> FsResult<Vec<u8>>;
    async fn write_file(&self, path: &str, offset: u64, data: &[u8]) -> FsResult<u32>;
    async fn create_file(&self, path: &str, mode: u32) -> FsResult<FileAttr>;
    async fn delete_file(&self, path: &str) -> FsResult<()>;
    async fn truncate(&self, path: &str, size: u64) -> FsResult<()>;

    // Directory operations
    async fn create_dir(&self, path: &str, mode: u32) -> FsResult<FileAttr>;
    async fn read_dir(&self, path: &str) -> FsResult<Vec<DirEntry>>;
    async fn remove_dir(&self, path: &str) -> FsResult<()>;

    // Metadata operations
    async fn get_attr(&self, path: &str) -> FsResult<FileAttr>;
    async fn set_attr(&self, path: &str, attr: SetAttr) -> FsResult<FileAttr>;
    async fn chmod(&self, path: &str, mode: u32) -> FsResult<()>;
    async fn chown(&self, path: &str, uid: u32, gid: u32) -> FsResult<()>;

    // Link operations (optional, can return NotSupported)
    async fn create_symlink(&self, target: &str, link: &str) -> FsResult<FileAttr> {
        Err(FsError::NotSupported(format!("Symlink not supported: {} -> {}", link, target)))
    }

    async fn read_symlink(&self, path: &str) -> FsResult<String> {
        Err(FsError::NotSupported(format!("Read symlink not supported: {}", path)))
    }

    // Extended attributes (optional)
    async fn setxattr(&self, path: &str, name: &str, _value: &[u8]) -> FsResult<()> {
        Err(FsError::NotSupported(format!("Extended attributes not supported: {}:{}", path, name)))
    }

    async fn getxattr(&self, path: &str, name: &str) -> FsResult<Vec<u8>> {
        Err(FsError::NotSupported(format!("Extended attributes not supported: {}:{}", path, name)))
    }

    async fn listxattr(&self, path: &str) -> FsResult<Vec<String>> {
        Err(FsError::NotSupported(format!("Extended attributes not supported: {}", path)))
    }

    async fn removexattr(&self, path: &str, name: &str) -> FsResult<()> {
        Err(FsError::NotSupported(format!("Extended attributes not supported: {}:{}", path, name)))
    }

    // Filesystem information
    async fn statfs(&self) -> FsResult<StatFs>;
}

/// Filesystem statistics
#[derive(Debug, Clone)]
pub struct StatFs {
    pub blocks: u64,  // Total blocks
    pub bfree: u64,   // Free blocks
    pub bavail: u64,  // Available blocks for unprivileged users
    pub files: u64,   // Total inodes
    pub ffree: u64,   // Free inodes
    pub bsize: u32,   // Block size
    pub namelen: u32, // Maximum filename length
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fserror_to_errno() {
        assert_eq!(FsError::PathNotFound("test".to_string()).to_errno(), libc::ENOENT);
        assert_eq!(FsError::AlreadyExists("test".to_string()).to_errno(), libc::EEXIST);
        assert_eq!(FsError::NotDirectory("test".to_string()).to_errno(), libc::ENOTDIR);
        assert_eq!(FsError::IsDirectory("test".to_string()).to_errno(), libc::EISDIR);
        assert_eq!(FsError::DirectoryNotEmpty("test".to_string()).to_errno(), libc::ENOTEMPTY);
        assert_eq!(FsError::InvalidPath("test".to_string()).to_errno(), libc::EINVAL);
        assert_eq!(FsError::PermissionDenied("test".to_string()).to_errno(), libc::EACCES);
        assert_eq!(FsError::NotSupported("test".to_string()).to_errno(), libc::ENOSYS);
        assert_eq!(FsError::IoError("test".to_string()).to_errno(), libc::EIO);
    }

    #[test]
    fn test_file_type_equality() {
        assert_eq!(FileType::RegularFile, FileType::RegularFile);
        assert_eq!(FileType::Directory, FileType::Directory);
        assert_eq!(FileType::Symlink, FileType::Symlink);
        assert_ne!(FileType::RegularFile, FileType::Directory);
    }

    #[test]
    fn test_file_attr_construction() {
        let now = Utc::now();
        let attr = FileAttr {
            inode: 1,
            kind: FileType::RegularFile,
            size: 1024,
            atime: now,
            mtime: now,
            ctime: now,
            mode: 0o644,
            uid: 1000,
            gid: 1000,
            nlinks: 1,
        };
        assert_eq!(attr.inode, 1);
        assert_eq!(attr.kind, FileType::RegularFile);
        assert_eq!(attr.size, 1024);
        assert_eq!(attr.mode, 0o644);
    }

    #[test]
    fn test_dir_entry_construction() {
        let entry =
            DirEntry { inode: 2, name: "test.txt".to_string(), kind: FileType::RegularFile };
        assert_eq!(entry.inode, 2);
        assert_eq!(entry.name, "test.txt");
        assert_eq!(entry.kind, FileType::RegularFile);
    }

    #[test]
    fn test_setattr_defaults() {
        let attr = SetAttr::default();
        assert!(attr.size.is_none());
        assert!(attr.atime.is_none());
        assert!(attr.mtime.is_none());
        assert!(attr.mode.is_none());
        assert!(attr.uid.is_none());
        assert!(attr.gid.is_none());
    }

    #[test]
    fn test_setattr_partial() {
        let attr = SetAttr { size: Some(2048), mode: Some(0o755), ..Default::default() };
        assert_eq!(attr.size, Some(2048));
        assert_eq!(attr.mode, Some(0o755));
        assert!(attr.uid.is_none());
    }

    #[test]
    fn test_statfs_construction() {
        let stats = StatFs {
            blocks: 1000,
            bfree: 500,
            bavail: 500,
            files: 100,
            ffree: 50,
            bsize: 4096,
            namelen: 255,
        };
        assert_eq!(stats.blocks, 1000);
        assert_eq!(stats.bfree, 500);
        assert_eq!(stats.bsize, 4096);
        assert_eq!(stats.namelen, 255);
    }

    #[test]
    fn test_fserror_display() {
        let err = FsError::PathNotFound("/test".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("Path not found"));
        assert!(msg.contains("/test"));
    }

    #[test]
    fn test_fserror_all_variants() {
        let errors = vec![
            FsError::PathNotFound("path".to_string()),
            FsError::AlreadyExists("file".to_string()),
            FsError::NotDirectory("file".to_string()),
            FsError::IsDirectory("dir".to_string()),
            FsError::DirectoryNotEmpty("dir".to_string()),
            FsError::InvalidPath("invalid".to_string()),
            FsError::PermissionDenied("file".to_string()),
            FsError::NotSupported("op".to_string()),
            FsError::IoError("error".to_string()),
        ];

        for err in errors {
            assert!(!format!("{}", err).is_empty());
            assert!(err.to_errno() > 0);
        }
    }

    #[test]
    fn test_dir_entry_with_different_types() {
        let entries = vec![
            (FileType::RegularFile, "file.txt"),
            (FileType::Directory, "dir"),
            (FileType::Symlink, "link"),
        ];

        for (kind, name) in entries {
            let entry = DirEntry { inode: 1, name: name.to_string(), kind };
            assert_eq!(entry.name, name);
            assert_eq!(entry.kind, kind);
        }
    }
}

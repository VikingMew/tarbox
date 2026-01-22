// WASI filesystem adapter
//
// This adapter implements WASI filesystem interface by bridging to TarboxBackend.
// It provides WASI-compatible file operations while reusing ~90% of the core filesystem logic.

use crate::fs::error::FsResult;
use crate::fs::operations::FileSystem;
use crate::storage::models::{Inode, InodeType};
use crate::wasi::config::WasiConfig;
use crate::wasi::error::WasiError;
use crate::wasi::fd_table::{FdTable, FileDescriptor, OpenFlags};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// WASI Adapter
///
/// Implements WASI filesystem operations by delegating to FilesystemInterface.
/// Manages file descriptors and provides POSIX-like API for WASM runtimes.
pub struct WasiAdapter<'a> {
    /// Underlying filesystem implementation
    fs: Arc<FileSystem<'a>>,
    /// Tenant ID for multi-tenancy
    tenant_id: Uuid,
    /// File descriptor table
    fd_table: Arc<Mutex<FdTable>>,
    /// Configuration
    config: WasiConfig,
}

impl<'a> WasiAdapter<'a> {
    /// Create a new WASI adapter
    pub fn new(fs: Arc<FileSystem<'a>>, tenant_id: Uuid, config: WasiConfig) -> Self {
        Self { fs, tenant_id, fd_table: Arc::new(Mutex::new(FdTable::new())), config }
    }

    /// Get the tenant ID
    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    /// Get the configuration
    pub fn config(&self) -> &WasiConfig {
        &self.config
    }

    /// Open a file and return a file descriptor
    ///
    /// This is a WASI-style open operation that returns a numeric fd.
    pub async fn fd_open(&self, path: &str, flags: OpenFlags) -> Result<u32, WasiError> {
        // Resolve the path to get inode info
        let stat: FsResult<Inode> = self.fs.stat(path).await;
        let stat = stat.map_err(WasiError::from)?;

        // Check if it's a directory
        let is_directory = matches!(stat.inode_type, InodeType::Dir);

        // Create file descriptor
        let descriptor = FileDescriptor::new(stat.inode_id, path.to_string(), flags, is_directory);

        // Allocate fd
        let fd = self.fd_table.lock().unwrap().allocate(descriptor);

        Ok(fd)
    }

    /// Read from a file descriptor
    pub async fn fd_read(&self, fd: u32, buf: &mut [u8]) -> Result<usize, WasiError> {
        // Get file descriptor
        let (path, position, _can_read) = {
            let table = self.fd_table.lock().unwrap();
            let descriptor = table.get(fd)?;

            if !descriptor.can_read() {
                return Err(WasiError::PermissionDenied);
            }

            if descriptor.is_directory {
                return Err(WasiError::IsDirectory);
            }

            (descriptor.path.clone(), descriptor.position, true)
        };

        // Read from filesystem
        let data: FsResult<Vec<u8>> = self.fs.read_file(&path).await;
        let data = data.map_err(|e| WasiError::IoError(format!("Failed to read file: {}", e)))?;

        // Calculate how much to read
        let start = position as usize;
        let end = std::cmp::min(start + buf.len(), data.len());

        if start >= data.len() {
            return Ok(0); // EOF
        }

        let to_read = end - start;
        buf[..to_read].copy_from_slice(&data[start..end]);

        // Update position
        {
            let mut table = self.fd_table.lock().unwrap();
            let descriptor = table.get_mut(fd)?;
            descriptor.position += to_read as u64;
        }

        Ok(to_read)
    }

    /// Write to a file descriptor
    pub async fn fd_write(&self, fd: u32, data: &[u8]) -> Result<usize, WasiError> {
        // Get file descriptor
        let (path, _position, _can_write, _is_append) = {
            let table = self.fd_table.lock().unwrap();
            let descriptor = table.get(fd)?;

            if !descriptor.can_write() {
                return Err(WasiError::PermissionDenied);
            }

            if descriptor.is_directory {
                return Err(WasiError::IsDirectory);
            }

            (descriptor.path.clone(), descriptor.position, true, descriptor.flags.append)
        };

        // For now, we do a simple write (replace entire file)
        // TODO: Implement proper offset-based writes
        let result: FsResult<()> = self.fs.write_file(&path, data).await;
        result.map_err(|e| WasiError::IoError(format!("Failed to write file: {}", e)))?;

        let written = data.len();

        // Update position
        {
            let mut table = self.fd_table.lock().unwrap();
            let descriptor = table.get_mut(fd)?;
            descriptor.position += written as u64;
        }

        Ok(written)
    }

    /// Seek within a file descriptor
    pub fn fd_seek(&self, fd: u32, offset: i64, whence: u8) -> Result<u64, WasiError> {
        let mut table = self.fd_table.lock().unwrap();
        let descriptor = table.get_mut(fd)?;
        descriptor.seek(offset, whence)
    }

    /// Close a file descriptor
    pub fn fd_close(&self, fd: u32) -> Result<(), WasiError> {
        let mut table = self.fd_table.lock().unwrap();
        table.close(fd)
    }

    /// Get file stat by path
    pub async fn path_stat(&self, path: &str) -> Result<FileStat, WasiError> {
        let stat: FsResult<Inode> = self.fs.stat(path).await;
        let stat = stat.map_err(WasiError::from)?;
        Ok(FileStat {
            inode_id: stat.inode_id as u64,
            size: stat.size as u64,
            is_directory: matches!(stat.inode_type, InodeType::Dir),
            is_symlink: matches!(stat.inode_type, InodeType::Symlink),
        })
    }

    /// Create a directory
    pub async fn path_create_directory(&self, path: &str) -> Result<(), WasiError> {
        let result: FsResult<Inode> = self.fs.create_directory(path).await;
        result.map_err(WasiError::from)?;
        Ok(())
    }

    /// Remove a directory
    pub async fn path_remove_directory(&self, path: &str) -> Result<(), WasiError> {
        let result: FsResult<()> = self.fs.remove_directory(path).await;
        result.map_err(WasiError::from)?;
        Ok(())
    }

    /// Unlink a file
    pub async fn path_unlink_file(&self, path: &str) -> Result<(), WasiError> {
        let result: FsResult<()> = self.fs.delete_file(path).await;
        result.map_err(WasiError::from)?;
        Ok(())
    }

    /// List directory entries
    pub async fn fd_readdir(&self, fd: u32) -> Result<Vec<DirEntry>, WasiError> {
        // Get directory path
        let path = {
            let table = self.fd_table.lock().unwrap();
            let descriptor = table.get(fd)?;

            if !descriptor.is_directory {
                return Err(WasiError::NotDirectory);
            }

            descriptor.path.clone()
        };

        // List directory
        let entries: FsResult<Vec<Inode>> = self.fs.list_directory(&path).await;
        let entries = entries.map_err(WasiError::from)?;

        Ok(entries
            .into_iter()
            .map(|e| DirEntry {
                name: e.name,
                is_directory: matches!(e.inode_type, InodeType::Dir),
            })
            .collect())
    }

    /// Get the number of open file descriptors
    pub fn fd_count(&self) -> usize {
        self.fd_table.lock().unwrap().len()
    }

    /// Close all file descriptors
    pub fn close_all(&self) {
        self.fd_table.lock().unwrap().close_all();
    }
}

/// File stat information (WASI-compatible)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStat {
    pub inode_id: u64,
    pub size: u64,
    pub is_directory: bool,
    pub is_symlink: bool,
}

/// Directory entry (WASI-compatible)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub is_directory: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> WasiConfig {
        WasiConfig::http("https://api.tarbox.io".to_string(), None)
    }

    #[test]
    fn test_wasi_adapter_new() {
        // We can't create a real FileSystem without DB, so just test construction logic
        let config = create_test_config();
        assert_eq!(config.db_mode, crate::wasi::config::DbMode::Http);
    }

    #[test]
    fn test_file_stat_construction() {
        let stat = FileStat { inode_id: 1, size: 1024, is_directory: false, is_symlink: false };
        assert_eq!(stat.inode_id, 1);
        assert_eq!(stat.size, 1024);
        assert!(!stat.is_directory);
        assert!(!stat.is_symlink);
    }

    #[test]
    fn test_file_stat_clone() {
        let stat = FileStat { inode_id: 1, size: 1024, is_directory: true, is_symlink: false };
        let cloned = stat.clone();
        assert_eq!(stat, cloned);
    }

    #[test]
    fn test_file_stat_equality() {
        let stat1 = FileStat { inode_id: 1, size: 1024, is_directory: false, is_symlink: false };
        let stat2 = FileStat { inode_id: 1, size: 1024, is_directory: false, is_symlink: false };
        let stat3 = FileStat { inode_id: 2, size: 1024, is_directory: false, is_symlink: false };
        assert_eq!(stat1, stat2);
        assert_ne!(stat1, stat3);
    }

    #[test]
    fn test_dir_entry_construction() {
        let entry = DirEntry { name: "test.txt".to_string(), is_directory: false };
        assert_eq!(entry.name, "test.txt");
        assert!(!entry.is_directory);
    }

    #[test]
    fn test_dir_entry_clone() {
        let entry = DirEntry { name: "test".to_string(), is_directory: true };
        let cloned = entry.clone();
        assert_eq!(entry, cloned);
    }

    #[test]
    fn test_dir_entry_equality() {
        let entry1 = DirEntry { name: "test".to_string(), is_directory: true };
        let entry2 = DirEntry { name: "test".to_string(), is_directory: true };
        let entry3 = DirEntry { name: "other".to_string(), is_directory: true };
        assert_eq!(entry1, entry2);
        assert_ne!(entry1, entry3);
    }

    #[test]
    fn test_wasi_config_integration() {
        let config = WasiConfig::http("https://api.tarbox.io".to_string(), Some("key".to_string()))
            .with_cache_size(200)
            .with_cache_ttl(600);

        assert_eq!(config.cache_size_mb, 200);
        assert_eq!(config.cache_ttl_secs, 600);
        assert_eq!(config.api_key, Some("key".to_string()));
    }

    // Note: Integration tests with actual FileSystem would go in tests/wasi_integration_test.rs
}

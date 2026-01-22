// WASI file descriptor table management

use crate::wasi::error::WasiError;
use std::collections::HashMap;

/// Open flags for files
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenFlags {
    /// Read access
    pub read: bool,
    /// Write access
    pub write: bool,
    /// Append mode
    pub append: bool,
    /// Create if not exists
    pub create: bool,
    /// Truncate on open
    pub truncate: bool,
}

impl Default for OpenFlags {
    fn default() -> Self {
        Self { read: true, write: false, append: false, create: false, truncate: false }
    }
}

impl OpenFlags {
    /// Create read-only flags
    pub fn read_only() -> Self {
        Self { read: true, write: false, ..Default::default() }
    }

    /// Create write-only flags
    pub fn write_only() -> Self {
        Self { read: false, write: true, ..Default::default() }
    }

    /// Create read-write flags
    pub fn read_write() -> Self {
        Self { read: true, write: true, ..Default::default() }
    }

    /// Create flags for creating a new file
    pub fn create() -> Self {
        Self { read: false, write: true, create: true, ..Default::default() }
    }

    /// Create flags with truncate
    pub fn with_truncate(mut self) -> Self {
        self.truncate = true;
        self
    }

    /// Create flags with append
    pub fn with_append(mut self) -> Self {
        self.append = true;
        self
    }
}

/// File descriptor entry
#[derive(Debug, Clone)]
pub struct FileDescriptor {
    /// Inode ID
    pub inode_id: i64,
    /// File path
    pub path: String,
    /// Open flags
    pub flags: OpenFlags,
    /// Current file position
    pub position: u64,
    /// Is this a directory?
    pub is_directory: bool,
}

impl FileDescriptor {
    /// Create a new file descriptor
    pub fn new(inode_id: i64, path: String, flags: OpenFlags, is_directory: bool) -> Self {
        Self { inode_id, path, flags, position: 0, is_directory }
    }

    /// Check if the descriptor allows reading
    pub fn can_read(&self) -> bool {
        self.flags.read
    }

    /// Check if the descriptor allows writing
    pub fn can_write(&self) -> bool {
        self.flags.write
    }

    /// Seek to a new position
    pub fn seek(&mut self, offset: i64, whence: u8) -> Result<u64, WasiError> {
        match whence {
            0 => {
                // SEEK_SET
                if offset < 0 {
                    return Err(WasiError::InvalidArgument);
                }
                self.position = offset as u64;
            }
            1 => {
                // SEEK_CUR
                if offset < 0 {
                    let abs_offset = (-offset) as u64;
                    if abs_offset > self.position {
                        return Err(WasiError::InvalidArgument);
                    }
                    self.position -= abs_offset;
                } else {
                    self.position += offset as u64;
                }
            }
            2 => {
                // SEEK_END - not supported without file size
                return Err(WasiError::NotSupported);
            }
            _ => return Err(WasiError::InvalidArgument),
        }
        Ok(self.position)
    }
}

/// File descriptor table
#[derive(Debug)]
pub struct FdTable {
    /// Map from fd to descriptor
    fds: HashMap<u32, FileDescriptor>,
    /// Next available fd
    next_fd: u32,
}

impl Default for FdTable {
    fn default() -> Self {
        Self::new()
    }
}

impl FdTable {
    /// Create a new file descriptor table
    ///
    /// Reserves fds 0, 1, 2 for stdin, stdout, stderr
    pub fn new() -> Self {
        Self {
            fds: HashMap::new(),
            next_fd: 3, // 0=stdin, 1=stdout, 2=stderr
        }
    }

    /// Allocate a new file descriptor
    pub fn allocate(&mut self, descriptor: FileDescriptor) -> u32 {
        let fd = self.next_fd;
        self.fds.insert(fd, descriptor);
        self.next_fd += 1;
        fd
    }

    /// Get a file descriptor
    pub fn get(&self, fd: u32) -> Result<&FileDescriptor, WasiError> {
        self.fds.get(&fd).ok_or(WasiError::BadFd)
    }

    /// Get a mutable file descriptor
    pub fn get_mut(&mut self, fd: u32) -> Result<&mut FileDescriptor, WasiError> {
        self.fds.get_mut(&fd).ok_or(WasiError::BadFd)
    }

    /// Close a file descriptor
    pub fn close(&mut self, fd: u32) -> Result<(), WasiError> {
        self.fds.remove(&fd).ok_or(WasiError::BadFd)?;
        Ok(())
    }

    /// Get the number of open file descriptors
    pub fn len(&self) -> usize {
        self.fds.len()
    }

    /// Check if the table is empty
    pub fn is_empty(&self) -> bool {
        self.fds.is_empty()
    }

    /// Close all file descriptors
    pub fn close_all(&mut self) {
        self.fds.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_flags_default() {
        let flags = OpenFlags::default();
        assert!(flags.read);
        assert!(!flags.write);
        assert!(!flags.append);
        assert!(!flags.create);
        assert!(!flags.truncate);
    }

    #[test]
    fn test_open_flags_read_only() {
        let flags = OpenFlags::read_only();
        assert!(flags.read);
        assert!(!flags.write);
    }

    #[test]
    fn test_open_flags_write_only() {
        let flags = OpenFlags::write_only();
        assert!(!flags.read);
        assert!(flags.write);
    }

    #[test]
    fn test_open_flags_read_write() {
        let flags = OpenFlags::read_write();
        assert!(flags.read);
        assert!(flags.write);
    }

    #[test]
    fn test_open_flags_create() {
        let flags = OpenFlags::create();
        assert!(!flags.read);
        assert!(flags.write);
        assert!(flags.create);
    }

    #[test]
    fn test_open_flags_with_truncate() {
        let flags = OpenFlags::read_write().with_truncate();
        assert!(flags.truncate);
    }

    #[test]
    fn test_open_flags_with_append() {
        let flags = OpenFlags::write_only().with_append();
        assert!(flags.append);
    }

    #[test]
    fn test_open_flags_clone() {
        let flags = OpenFlags::read_write();
        let cloned = flags;
        assert_eq!(flags, cloned);
    }

    #[test]
    fn test_file_descriptor_new() {
        let fd = FileDescriptor::new(1, "/test.txt".to_string(), OpenFlags::read_only(), false);
        assert_eq!(fd.inode_id, 1);
        assert_eq!(fd.path, "/test.txt");
        assert_eq!(fd.position, 0);
        assert!(!fd.is_directory);
    }

    #[test]
    fn test_file_descriptor_can_read() {
        let fd = FileDescriptor::new(1, "/test.txt".to_string(), OpenFlags::read_only(), false);
        assert!(fd.can_read());
        assert!(!fd.can_write());
    }

    #[test]
    fn test_file_descriptor_can_write() {
        let fd = FileDescriptor::new(1, "/test.txt".to_string(), OpenFlags::write_only(), false);
        assert!(!fd.can_read());
        assert!(fd.can_write());
    }

    #[test]
    fn test_file_descriptor_seek_set() {
        let mut fd = FileDescriptor::new(1, "/test.txt".to_string(), OpenFlags::read_only(), false);
        let pos = fd.seek(100, 0).unwrap();
        assert_eq!(pos, 100);
        assert_eq!(fd.position, 100);
    }

    #[test]
    fn test_file_descriptor_seek_set_negative() {
        let mut fd = FileDescriptor::new(1, "/test.txt".to_string(), OpenFlags::read_only(), false);
        let result = fd.seek(-10, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_file_descriptor_seek_cur_forward() {
        let mut fd = FileDescriptor::new(1, "/test.txt".to_string(), OpenFlags::read_only(), false);
        fd.position = 100;
        let pos = fd.seek(50, 1).unwrap();
        assert_eq!(pos, 150);
    }

    #[test]
    fn test_file_descriptor_seek_cur_backward() {
        let mut fd = FileDescriptor::new(1, "/test.txt".to_string(), OpenFlags::read_only(), false);
        fd.position = 100;
        let pos = fd.seek(-50, 1).unwrap();
        assert_eq!(pos, 50);
    }

    #[test]
    fn test_file_descriptor_seek_cur_underflow() {
        let mut fd = FileDescriptor::new(1, "/test.txt".to_string(), OpenFlags::read_only(), false);
        fd.position = 10;
        let result = fd.seek(-20, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_file_descriptor_seek_end_not_supported() {
        let mut fd = FileDescriptor::new(1, "/test.txt".to_string(), OpenFlags::read_only(), false);
        let result = fd.seek(0, 2);
        assert!(matches!(result, Err(WasiError::NotSupported)));
    }

    #[test]
    fn test_file_descriptor_seek_invalid_whence() {
        let mut fd = FileDescriptor::new(1, "/test.txt".to_string(), OpenFlags::read_only(), false);
        let result = fd.seek(0, 99);
        assert!(matches!(result, Err(WasiError::InvalidArgument)));
    }

    #[test]
    fn test_file_descriptor_clone() {
        let fd = FileDescriptor::new(1, "/test.txt".to_string(), OpenFlags::read_only(), false);
        let cloned = fd.clone();
        assert_eq!(fd.inode_id, cloned.inode_id);
        assert_eq!(fd.path, cloned.path);
    }

    #[test]
    fn test_fd_table_new() {
        let table = FdTable::new();
        assert_eq!(table.next_fd, 3); // Reserves 0,1,2
        assert!(table.is_empty());
    }

    #[test]
    fn test_fd_table_allocate() {
        let mut table = FdTable::new();
        let fd1 = table.allocate(FileDescriptor::new(
            1,
            "/test1.txt".to_string(),
            OpenFlags::read_only(),
            false,
        ));
        let fd2 = table.allocate(FileDescriptor::new(
            2,
            "/test2.txt".to_string(),
            OpenFlags::read_only(),
            false,
        ));

        assert_eq!(fd1, 3);
        assert_eq!(fd2, 4);
        assert_eq!(table.len(), 2);
    }

    #[test]
    fn test_fd_table_get() {
        let mut table = FdTable::new();
        let fd = table.allocate(FileDescriptor::new(
            1,
            "/test.txt".to_string(),
            OpenFlags::read_only(),
            false,
        ));

        let descriptor = table.get(fd).unwrap();
        assert_eq!(descriptor.inode_id, 1);
        assert_eq!(descriptor.path, "/test.txt");
    }

    #[test]
    fn test_fd_table_get_invalid() {
        let table = FdTable::new();
        let result = table.get(999);
        assert!(matches!(result, Err(WasiError::BadFd)));
    }

    #[test]
    fn test_fd_table_get_mut() {
        let mut table = FdTable::new();
        let fd = table.allocate(FileDescriptor::new(
            1,
            "/test.txt".to_string(),
            OpenFlags::read_only(),
            false,
        ));

        let descriptor = table.get_mut(fd).unwrap();
        descriptor.position = 100;

        let descriptor = table.get(fd).unwrap();
        assert_eq!(descriptor.position, 100);
    }

    #[test]
    fn test_fd_table_close() {
        let mut table = FdTable::new();
        let fd = table.allocate(FileDescriptor::new(
            1,
            "/test.txt".to_string(),
            OpenFlags::read_only(),
            false,
        ));

        assert_eq!(table.len(), 1);
        table.close(fd).unwrap();
        assert_eq!(table.len(), 0);
    }

    #[test]
    fn test_fd_table_close_invalid() {
        let mut table = FdTable::new();
        let result = table.close(999);
        assert!(matches!(result, Err(WasiError::BadFd)));
    }

    #[test]
    fn test_fd_table_close_all() {
        let mut table = FdTable::new();
        table.allocate(FileDescriptor::new(
            1,
            "/test1.txt".to_string(),
            OpenFlags::read_only(),
            false,
        ));
        table.allocate(FileDescriptor::new(
            2,
            "/test2.txt".to_string(),
            OpenFlags::read_only(),
            false,
        ));

        assert_eq!(table.len(), 2);
        table.close_all();
        assert_eq!(table.len(), 0);
    }

    #[test]
    fn test_fd_table_default() {
        let table = FdTable::default();
        assert_eq!(table.next_fd, 3);
    }

    #[test]
    fn test_fd_table_is_empty() {
        let mut table = FdTable::new();
        assert!(table.is_empty());

        table.allocate(FileDescriptor::new(
            1,
            "/test.txt".to_string(),
            OpenFlags::read_only(),
            false,
        ));
        assert!(!table.is_empty());
    }

    #[test]
    fn test_fd_table_multiple_operations() {
        let mut table = FdTable::new();

        // Allocate multiple fds
        let fd1 = table.allocate(FileDescriptor::new(
            1,
            "/test1.txt".to_string(),
            OpenFlags::read_only(),
            false,
        ));
        let fd2 = table.allocate(FileDescriptor::new(
            2,
            "/test2.txt".to_string(),
            OpenFlags::write_only(),
            false,
        ));
        let fd3 = table.allocate(FileDescriptor::new(
            3,
            "/test3.txt".to_string(),
            OpenFlags::read_write(),
            false,
        ));

        assert_eq!(table.len(), 3);

        // Close middle fd
        table.close(fd2).unwrap();
        assert_eq!(table.len(), 2);

        // Can still access others
        assert!(table.get(fd1).is_ok());
        assert!(table.get(fd3).is_ok());
        assert!(table.get(fd2).is_err());

        // Allocate a new one
        let fd4 = table.allocate(FileDescriptor::new(
            4,
            "/test4.txt".to_string(),
            OpenFlags::read_only(),
            false,
        ));
        assert_eq!(table.len(), 3);
        assert!(table.get(fd4).is_ok());
    }
}

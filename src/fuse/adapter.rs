// FUSE adapter - bridges sync FUSE callbacks to async FilesystemInterface
//
// This adapter implements the fuser::Filesystem trait and delegates all operations
// to the async FilesystemInterface implementation. It handles:
// - Async to sync conversion using a dedicated tokio runtime
// - Inode to path mapping
// - FUSE types to FilesystemInterface types conversion
// - Error code translation
//
// IMPORTANT: The adapter uses its own dedicated runtime to avoid deadlocks.
// FUSE callbacks are synchronous, but our backend is async. If we used the
// caller's runtime (via Handle::current()), calling block_on() inside a
// runtime context would cause a deadlock. By creating a dedicated runtime,
// we ensure FUSE operations can safely block without affecting the caller.

use super::interface::{FileAttr, FilesystemInterface, FsError, SetAttr};
use fuser::{
    FileType as FuseFileType, Filesystem, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory,
    ReplyEmpty, ReplyEntry, ReplyOpen, ReplyStatfs, ReplyWrite, Request, TimeOrNow,
};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::runtime::Runtime;

/// FUSE adapter that bridges sync FUSE callbacks to async FilesystemInterface
///
/// This adapter owns a dedicated tokio runtime to safely bridge sync FUSE
/// callbacks to async backend operations without risking deadlocks.
pub struct FuseAdapter {
    /// The underlying filesystem implementation
    backend: Arc<dyn FilesystemInterface>,

    /// Dedicated tokio runtime for async operations
    /// Using a dedicated runtime (not Handle) avoids deadlocks when
    /// the caller is already inside a tokio runtime context.
    runtime: Arc<Runtime>,

    /// Inode to path mapping
    /// FUSE uses inodes, but our backend uses paths
    inode_map: Arc<RwLock<InodeMap>>,
}

/// Manages inode <-> path bidirectional mapping
struct InodeMap {
    /// inode -> path
    inode_to_path: HashMap<u64, String>,

    /// path -> inode
    path_to_inode: HashMap<String, u64>,

    /// Next inode to allocate
    next_inode: u64,
}

impl InodeMap {
    fn new() -> Self {
        let mut map = Self {
            inode_to_path: HashMap::new(),
            path_to_inode: HashMap::new(),
            next_inode: 2, // 1 is reserved for root
        };

        // Initialize root inode
        map.insert(1, "/".to_string());

        map
    }

    /// Insert a path and get its inode (or create new inode)
    fn insert(&mut self, inode: u64, path: String) {
        self.inode_to_path.insert(inode, path.clone());
        self.path_to_inode.insert(path, inode);
    }

    /// Get or create inode for path
    fn get_or_create(&mut self, path: &str) -> u64 {
        if let Some(&inode) = self.path_to_inode.get(path) {
            return inode;
        }

        let inode = self.next_inode;
        self.next_inode += 1;
        self.insert(inode, path.to_string());
        inode
    }

    /// Get path by inode
    fn get_path(&self, inode: u64) -> Option<&str> {
        self.inode_to_path.get(&inode).map(|s| s.as_str())
    }

    /// Remove inode mapping
    fn remove(&mut self, inode: u64) {
        if let Some(path) = self.inode_to_path.remove(&inode) {
            self.path_to_inode.remove(&path);
        }
    }
}

impl FuseAdapter {
    /// Create a new FUSE adapter with a dedicated runtime
    ///
    /// The adapter creates and owns a dedicated tokio runtime to safely
    /// bridge sync FUSE callbacks to async backend operations.
    pub fn new(backend: Arc<dyn FilesystemInterface>) -> Self {
        // Create a dedicated runtime for FUSE operations
        // This avoids deadlocks when the caller is already in a tokio runtime
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .thread_name("tarbox-fuse")
            .enable_all()
            .build()
            .expect("Failed to create FUSE runtime");

        Self {
            backend,
            runtime: Arc::new(runtime),
            inode_map: Arc::new(RwLock::new(InodeMap::new())),
        }
    }

    /// Create a new FUSE adapter with a provided runtime
    ///
    /// Use this when you want to provide your own runtime (e.g., for testing).
    /// WARNING: If the provided runtime is the same as the caller's runtime,
    /// this may cause deadlocks. Only use this if you know what you're doing.
    pub fn with_runtime(backend: Arc<dyn FilesystemInterface>, runtime: Arc<Runtime>) -> Self {
        Self { backend, runtime, inode_map: Arc::new(RwLock::new(InodeMap::new())) }
    }

    /// Get path from inode
    fn get_path(&self, inode: u64) -> Result<String, libc::c_int> {
        let map = self.inode_map.read().unwrap();
        map.get_path(inode).map(|s| s.to_string()).ok_or(libc::ENOENT)
    }

    /// Execute async operation in tokio runtime
    fn block_on<F, T>(&self, future: F) -> T
    where
        F: std::future::Future<Output = T>,
    {
        self.runtime.block_on(future)
    }

    /// Convert FsError to errno
    fn error_to_errno(error: FsError) -> libc::c_int {
        error.to_errno()
    }

    /// Convert our FileAttr to fuser FileAttr
    fn to_fuse_attr(attr: &FileAttr, _ttl: Duration) -> fuser::FileAttr {
        fuser::FileAttr {
            ino: attr.inode,
            size: attr.size,
            blocks: attr.size.div_ceil(512),
            atime: datetime_to_systemtime(attr.atime),
            mtime: datetime_to_systemtime(attr.mtime),
            ctime: datetime_to_systemtime(attr.ctime),
            crtime: UNIX_EPOCH,
            kind: match attr.kind {
                super::interface::FileType::RegularFile => FuseFileType::RegularFile,
                super::interface::FileType::Directory => FuseFileType::Directory,
                super::interface::FileType::Symlink => FuseFileType::Symlink,
            },
            perm: attr.mode as u16,
            nlink: attr.nlinks,
            uid: attr.uid,
            gid: attr.gid,
            rdev: 0,
            blksize: 4096,
            flags: 0,
        }
    }
}

/// Convert chrono DateTime to SystemTime
fn datetime_to_systemtime(dt: chrono::DateTime<chrono::Utc>) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(dt.timestamp() as u64)
}

/// Convert SystemTime to chrono DateTime
fn systemtime_to_datetime(st: SystemTime) -> chrono::DateTime<chrono::Utc> {
    let duration = st.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
    chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0).unwrap_or_else(chrono::Utc::now)
}

/// Default TTL for file attributes (1 second)
const ATTR_TTL: Duration = Duration::from_secs(1);

/// Default TTL for directory entries (1 second)
const ENTRY_TTL: Duration = Duration::from_secs(1);

impl Filesystem for FuseAdapter {
    /// Initialize filesystem
    fn init(
        &mut self,
        _req: &Request,
        _config: &mut fuser::KernelConfig,
    ) -> Result<(), libc::c_int> {
        tracing::info!("FUSE filesystem initialized");
        Ok(())
    }

    /// Cleanup filesystem
    fn destroy(&mut self) {
        tracing::info!("FUSE filesystem destroyed");
    }

    /// Look up a directory entry by name
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let name = match name.to_str() {
            Some(n) => n,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        let parent_path = match self.get_path(parent) {
            Ok(p) => p,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        // Construct child path
        let path = if parent_path == "/" {
            format!("/{}", name)
        } else {
            format!("{}/{}", parent_path, name)
        };

        // Get file attributes
        let result = self.block_on(self.backend.get_attr(&path));

        match result {
            Ok(attr) => {
                // Update inode mapping
                let inode = {
                    let mut map = self.inode_map.write().unwrap();
                    map.get_or_create(&path)
                };

                // Update attr with mapped inode
                let mut attr = attr;
                attr.inode = inode;

                let fuse_attr = Self::to_fuse_attr(&attr, ENTRY_TTL);
                reply.entry(&ENTRY_TTL, &fuse_attr, 0);
            }
            Err(e) => {
                reply.error(Self::error_to_errno(e));
            }
        }
    }

    /// Get file attributes
    fn getattr(&mut self, _req: &Request, ino: u64, _fh: Option<u64>, reply: ReplyAttr) {
        let path = match self.get_path(ino) {
            Ok(p) => p,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        let result = self.block_on(self.backend.get_attr(&path));

        match result {
            Ok(mut attr) => {
                attr.inode = ino; // Use FUSE inode
                let fuse_attr = Self::to_fuse_attr(&attr, ATTR_TTL);
                reply.attr(&ATTR_TTL, &fuse_attr);
            }
            Err(e) => {
                reply.error(Self::error_to_errno(e));
            }
        }
    }

    /// Set file attributes
    fn setattr(
        &mut self,
        _req: &Request,
        ino: u64,
        mode: Option<u32>,
        uid: Option<u32>,
        gid: Option<u32>,
        size: Option<u64>,
        atime: Option<TimeOrNow>,
        mtime: Option<TimeOrNow>,
        _ctime: Option<SystemTime>,
        _fh: Option<u64>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        _flags: Option<u32>,
        reply: ReplyAttr,
    ) {
        let path = match self.get_path(ino) {
            Ok(p) => p,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        // Convert TimeOrNow to DateTime
        let atime_dt = atime.map(|t| match t {
            TimeOrNow::SpecificTime(st) => systemtime_to_datetime(st),
            TimeOrNow::Now => chrono::Utc::now(),
        });

        let mtime_dt = mtime.map(|t| match t {
            TimeOrNow::SpecificTime(st) => systemtime_to_datetime(st),
            TimeOrNow::Now => chrono::Utc::now(),
        });

        let set_attr = SetAttr { mode, uid, gid, size, atime: atime_dt, mtime: mtime_dt };

        let result = self.block_on(self.backend.set_attr(&path, set_attr));

        match result {
            Ok(mut attr) => {
                attr.inode = ino;
                let fuse_attr = Self::to_fuse_attr(&attr, ATTR_TTL);
                reply.attr(&ATTR_TTL, &fuse_attr);
            }
            Err(e) => {
                reply.error(Self::error_to_errno(e));
            }
        }
    }

    /// Read data from file
    fn read(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyData,
    ) {
        let path = match self.get_path(ino) {
            Ok(p) => p,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        let result = self.block_on(self.backend.read_file(&path, offset as u64, size));

        match result {
            Ok(data) => {
                reply.data(&data);
            }
            Err(e) => {
                reply.error(Self::error_to_errno(e));
            }
        }
    }

    /// Write data to file
    fn write(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        data: &[u8],
        _write_flags: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyWrite,
    ) {
        let path = match self.get_path(ino) {
            Ok(p) => p,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        let result = self.block_on(self.backend.write_file(&path, offset as u64, data));

        match result {
            Ok(written) => {
                reply.written(written);
            }
            Err(e) => {
                reply.error(Self::error_to_errno(e));
            }
        }
    }

    /// Create a directory
    fn mkdir(
        &mut self,
        _req: &Request,
        parent: u64,
        name: &OsStr,
        mode: u32,
        _umask: u32,
        reply: ReplyEntry,
    ) {
        let name = match name.to_str() {
            Some(n) => n,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        let parent_path = match self.get_path(parent) {
            Ok(p) => p,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        let path = if parent_path == "/" {
            format!("/{}", name)
        } else {
            format!("{}/{}", parent_path, name)
        };

        let result = self.block_on(self.backend.create_dir(&path, mode));

        match result {
            Ok(attr) => {
                let inode = {
                    let mut map = self.inode_map.write().unwrap();
                    map.get_or_create(&path)
                };

                let mut attr = attr;
                attr.inode = inode;

                let fuse_attr = Self::to_fuse_attr(&attr, ENTRY_TTL);
                reply.entry(&ENTRY_TTL, &fuse_attr, 0);
            }
            Err(e) => {
                reply.error(Self::error_to_errno(e));
            }
        }
    }

    /// Remove a directory
    fn rmdir(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        let name = match name.to_str() {
            Some(n) => n,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        let parent_path = match self.get_path(parent) {
            Ok(p) => p,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        let path = if parent_path == "/" {
            format!("/{}", name)
        } else {
            format!("{}/{}", parent_path, name)
        };

        let result = self.block_on(self.backend.remove_dir(&path));

        match result {
            Ok(_) => {
                // Remove from inode map
                if let Some(&inode) = self.inode_map.read().unwrap().path_to_inode.get(&path) {
                    self.inode_map.write().unwrap().remove(inode);
                }
                reply.ok();
            }
            Err(e) => {
                reply.error(Self::error_to_errno(e));
            }
        }
    }

    /// Read directory entries
    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        let path = match self.get_path(ino) {
            Ok(p) => p,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        let result = self.block_on(self.backend.read_dir(&path));

        match result {
            Ok(entries) => {
                // Add . and ..
                let mut all_entries = vec![
                    (ino, FuseFileType::Directory, "."),
                    (ino, FuseFileType::Directory, ".."), // TODO: get parent inode
                ];

                // Add actual entries
                for entry in &entries {
                    let inode = {
                        let entry_path = if path == "/" {
                            format!("/{}", entry.name)
                        } else {
                            format!("{}/{}", path, entry.name)
                        };
                        let mut map = self.inode_map.write().unwrap();
                        map.get_or_create(&entry_path)
                    };

                    let kind = match entry.kind {
                        super::interface::FileType::RegularFile => FuseFileType::RegularFile,
                        super::interface::FileType::Directory => FuseFileType::Directory,
                        super::interface::FileType::Symlink => FuseFileType::Symlink,
                    };

                    all_entries.push((inode, kind, entry.name.as_str()));
                }

                // Reply with entries starting from offset
                for (i, (inode, kind, name)) in all_entries.iter().enumerate().skip(offset as usize)
                {
                    let buffer_full = reply.add(*inode, (i + 1) as i64, *kind, name);
                    if buffer_full {
                        break;
                    }
                }

                reply.ok();
            }
            Err(e) => {
                reply.error(Self::error_to_errno(e));
            }
        }
    }

    /// Create and open a file
    fn create(
        &mut self,
        _req: &Request,
        parent: u64,
        name: &OsStr,
        mode: u32,
        _umask: u32,
        _flags: i32,
        reply: ReplyCreate,
    ) {
        let name = match name.to_str() {
            Some(n) => n,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        let parent_path = match self.get_path(parent) {
            Ok(p) => p,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        let path = if parent_path == "/" {
            format!("/{}", name)
        } else {
            format!("{}/{}", parent_path, name)
        };

        let result = self.block_on(self.backend.create_file(&path, mode));

        match result {
            Ok(attr) => {
                let inode = {
                    let mut map = self.inode_map.write().unwrap();
                    map.get_or_create(&path)
                };

                let mut attr = attr;
                attr.inode = inode;

                let fuse_attr = Self::to_fuse_attr(&attr, ENTRY_TTL);
                reply.created(&ENTRY_TTL, &fuse_attr, 0, 0, 0);
            }
            Err(e) => {
                reply.error(Self::error_to_errno(e));
            }
        }
    }

    /// Remove a file
    fn unlink(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        let name = match name.to_str() {
            Some(n) => n,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        let parent_path = match self.get_path(parent) {
            Ok(p) => p,
            Err(e) => {
                reply.error(e);
                return;
            }
        };

        let path = if parent_path == "/" {
            format!("/{}", name)
        } else {
            format!("{}/{}", parent_path, name)
        };

        let result = self.block_on(self.backend.delete_file(&path));

        match result {
            Ok(_) => {
                // Remove from inode map
                if let Some(&inode) = self.inode_map.read().unwrap().path_to_inode.get(&path) {
                    self.inode_map.write().unwrap().remove(inode);
                }
                reply.ok();
            }
            Err(e) => {
                reply.error(Self::error_to_errno(e));
            }
        }
    }

    /// Open a file
    fn open(&mut self, _req: &Request, _ino: u64, _flags: i32, reply: ReplyOpen) {
        // For now, we don't maintain file handles
        // Just return a dummy file handle
        reply.opened(0, 0);
    }

    /// Release (close) a file
    fn release(
        &mut self,
        _req: &Request,
        _ino: u64,
        _fh: u64,
        _flags: i32,
        _lock_owner: Option<u64>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        // Nothing to do for now
        reply.ok();
    }

    /// Get filesystem statistics
    fn statfs(&mut self, _req: &Request, _ino: u64, reply: ReplyStatfs) {
        let result = self.block_on(self.backend.statfs());

        match result {
            Ok(stats) => {
                reply.statfs(
                    stats.blocks,
                    stats.bfree,
                    stats.bavail,
                    stats.files,
                    stats.ffree,
                    stats.bsize,
                    stats.namelen,
                    0, // frsize
                );
            }
            Err(e) => {
                reply.error(Self::error_to_errno(e));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inode_map_new() {
        let map = InodeMap::new();
        assert_eq!(map.get_path(1), Some("/"));
        assert_eq!(map.next_inode, 2);
    }

    #[test]
    fn test_inode_map_insert() {
        let mut map = InodeMap::new();
        map.insert(2, "/test".to_string());
        assert_eq!(map.get_path(2), Some("/test"));
    }

    #[test]
    fn test_inode_map_get_or_create() {
        let mut map = InodeMap::new();
        let ino1 = map.get_or_create("/test");
        let ino2 = map.get_or_create("/test");
        assert_eq!(ino1, ino2);
        assert_eq!(map.get_path(ino1), Some("/test"));
    }

    #[test]
    fn test_inode_map_remove() {
        let mut map = InodeMap::new();
        let ino = map.get_or_create("/test");
        map.remove(ino);
        assert_eq!(map.get_path(ino), None);
    }

    #[test]
    fn test_datetime_conversion() {
        let dt = chrono::Utc::now();
        let st = datetime_to_systemtime(dt);
        let dt2 = systemtime_to_datetime(st);

        // Should be within 1 second
        let diff = (dt.timestamp() - dt2.timestamp()).abs();
        assert!(diff <= 1);
    }
}

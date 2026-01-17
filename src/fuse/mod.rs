// FUSE interface implementation
//
// This module provides FUSE (Filesystem in Userspace) support,
// allowing Tarbox to be mounted as a standard POSIX filesystem.

pub mod backend;
pub mod interface;

pub use backend::TarboxBackend;
pub use interface::{
    DirEntry, FileAttr, FileType, FilesystemInterface, FsError, FsResult, SetAttr, StatFs,
};

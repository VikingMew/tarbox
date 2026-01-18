// FUSE interface implementation
//
// This module provides FUSE (Filesystem in Userspace) support,
// allowing Tarbox to be mounted as a standard POSIX filesystem.

pub mod adapter;
pub mod backend;
pub mod interface;
pub mod mount;

pub use adapter::FuseAdapter;
pub use backend::TarboxBackend;
pub use interface::{
    DirEntry, FileAttr, FileType, FilesystemInterface, FsError, FsResult, SetAttr, StatFs,
};
pub use mount::{MountOptions, mount, unmount};

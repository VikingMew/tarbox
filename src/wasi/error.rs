// WASI error handling and errno mapping

use crate::fs::error::FsError;
use std::fmt;

/// WASI-specific errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WasiError {
    /// File not found
    NotFound,
    /// Permission denied
    PermissionDenied,
    /// File already exists
    AlreadyExists,
    /// Invalid argument
    InvalidArgument,
    /// Is a directory
    IsDirectory,
    /// Not a directory
    NotDirectory,
    /// Directory not empty
    DirectoryNotEmpty,
    /// No space left on device
    NoSpaceLeft,
    /// Invalid file descriptor
    InvalidFd,
    /// Bad file number
    BadFd,
    /// File descriptor not open
    FdNotOpen,
    /// Operation not supported
    NotSupported,
    /// IO error
    IoError(String),
}

impl fmt::Display for WasiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WasiError::NotFound => write!(f, "File not found"),
            WasiError::PermissionDenied => write!(f, "Permission denied"),
            WasiError::AlreadyExists => write!(f, "File already exists"),
            WasiError::InvalidArgument => write!(f, "Invalid argument"),
            WasiError::IsDirectory => write!(f, "Is a directory"),
            WasiError::NotDirectory => write!(f, "Not a directory"),
            WasiError::DirectoryNotEmpty => write!(f, "Directory not empty"),
            WasiError::NoSpaceLeft => write!(f, "No space left on device"),
            WasiError::InvalidFd => write!(f, "Invalid file descriptor"),
            WasiError::BadFd => write!(f, "Bad file descriptor"),
            WasiError::FdNotOpen => write!(f, "File descriptor not open"),
            WasiError::NotSupported => write!(f, "Operation not supported"),
            WasiError::IoError(msg) => write!(f, "IO error: {}", msg),
        }
    }
}

impl std::error::Error for WasiError {}

impl From<FsError> for WasiError {
    fn from(err: FsError) -> Self {
        match err {
            FsError::PathNotFound(_) => WasiError::NotFound,
            FsError::AlreadyExists(_) => WasiError::AlreadyExists,
            FsError::NotDirectory(_) => WasiError::NotDirectory,
            FsError::IsDirectory(_) => WasiError::IsDirectory,
            FsError::DirectoryNotEmpty(_) => WasiError::DirectoryNotEmpty,
            FsError::InvalidPath(_) => WasiError::InvalidArgument,
            FsError::PathTooLong(_) => WasiError::InvalidArgument,
            FsError::FilenameTooLong(_) => WasiError::InvalidArgument,
            FsError::Storage(_) => WasiError::IoError("Storage error".to_string()),
        }
    }
}

/// Map WasiError to WASI errno values
///
/// WASI errno values are defined in the WASI specification.
/// See: https://github.com/WebAssembly/WASI/blob/main/phases/snapshot/docs.md
pub fn to_wasi_errno(err: &WasiError) -> u16 {
    match err {
        WasiError::NotFound => 44,          // ENOENT
        WasiError::PermissionDenied => 63,  // EACCES
        WasiError::AlreadyExists => 20,     // EEXIST
        WasiError::InvalidArgument => 28,   // EINVAL
        WasiError::IsDirectory => 31,       // EISDIR
        WasiError::NotDirectory => 54,      // ENOTDIR
        WasiError::DirectoryNotEmpty => 66, // ENOTEMPTY
        WasiError::NoSpaceLeft => 51,       // ENOSPC
        WasiError::InvalidFd => 8,          // EBADF
        WasiError::BadFd => 8,              // EBADF
        WasiError::FdNotOpen => 8,          // EBADF
        WasiError::NotSupported => 58,      // ENOTSUP
        WasiError::IoError(_) => 29,        // EIO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasi_error_display() {
        assert_eq!(WasiError::NotFound.to_string(), "File not found");
        assert_eq!(WasiError::PermissionDenied.to_string(), "Permission denied");
        assert_eq!(WasiError::AlreadyExists.to_string(), "File already exists");
        assert_eq!(WasiError::InvalidArgument.to_string(), "Invalid argument");
        assert_eq!(WasiError::IsDirectory.to_string(), "Is a directory");
        assert_eq!(WasiError::NotDirectory.to_string(), "Not a directory");
        assert_eq!(WasiError::DirectoryNotEmpty.to_string(), "Directory not empty");
        assert_eq!(WasiError::NoSpaceLeft.to_string(), "No space left on device");
        assert_eq!(WasiError::InvalidFd.to_string(), "Invalid file descriptor");
        assert_eq!(WasiError::BadFd.to_string(), "Bad file descriptor");
        assert_eq!(WasiError::FdNotOpen.to_string(), "File descriptor not open");
        assert_eq!(WasiError::NotSupported.to_string(), "Operation not supported");
        assert_eq!(WasiError::IoError("test".to_string()).to_string(), "IO error: test");
    }

    #[test]
    fn test_wasi_error_equality() {
        assert_eq!(WasiError::NotFound, WasiError::NotFound);
        assert_ne!(WasiError::NotFound, WasiError::PermissionDenied);
        assert_eq!(WasiError::IoError("test".to_string()), WasiError::IoError("test".to_string()));
    }

    #[test]
    fn test_wasi_error_clone() {
        let err = WasiError::NotFound;
        let cloned = err.clone();
        assert_eq!(err, cloned);
    }

    #[test]
    fn test_from_fs_error() {
        assert_eq!(
            WasiError::from(FsError::PathNotFound("/test".to_string())),
            WasiError::NotFound
        );
        assert_eq!(
            WasiError::from(FsError::AlreadyExists("/test".to_string())),
            WasiError::AlreadyExists
        );
        assert_eq!(
            WasiError::from(FsError::NotDirectory("/test".to_string())),
            WasiError::NotDirectory
        );
        assert_eq!(
            WasiError::from(FsError::IsDirectory("/test".to_string())),
            WasiError::IsDirectory
        );
        assert_eq!(
            WasiError::from(FsError::DirectoryNotEmpty("/test".to_string())),
            WasiError::DirectoryNotEmpty
        );
        assert_eq!(
            WasiError::from(FsError::InvalidPath("/test".to_string())),
            WasiError::InvalidArgument
        );
        assert_eq!(WasiError::from(FsError::PathTooLong(5000)), WasiError::InvalidArgument);
        assert_eq!(WasiError::from(FsError::FilenameTooLong(300)), WasiError::InvalidArgument);
    }

    #[test]
    fn test_to_wasi_errno() {
        assert_eq!(to_wasi_errno(&WasiError::NotFound), 44);
        assert_eq!(to_wasi_errno(&WasiError::PermissionDenied), 63);
        assert_eq!(to_wasi_errno(&WasiError::AlreadyExists), 20);
        assert_eq!(to_wasi_errno(&WasiError::InvalidArgument), 28);
        assert_eq!(to_wasi_errno(&WasiError::IsDirectory), 31);
        assert_eq!(to_wasi_errno(&WasiError::NotDirectory), 54);
        assert_eq!(to_wasi_errno(&WasiError::DirectoryNotEmpty), 66);
        assert_eq!(to_wasi_errno(&WasiError::NoSpaceLeft), 51);
        assert_eq!(to_wasi_errno(&WasiError::InvalidFd), 8);
        assert_eq!(to_wasi_errno(&WasiError::BadFd), 8);
        assert_eq!(to_wasi_errno(&WasiError::FdNotOpen), 8);
        assert_eq!(to_wasi_errno(&WasiError::NotSupported), 58);
        assert_eq!(to_wasi_errno(&WasiError::IoError("test".to_string())), 29);
    }

    #[test]
    fn test_wasi_error_is_error_trait() {
        let err: Box<dyn std::error::Error> = Box::new(WasiError::NotFound);
        assert_eq!(err.to_string(), "File not found");
    }

    #[test]
    fn test_all_error_variants_have_errno() {
        // Ensure all error variants can be mapped to errno
        let errors = vec![
            WasiError::NotFound,
            WasiError::PermissionDenied,
            WasiError::AlreadyExists,
            WasiError::InvalidArgument,
            WasiError::IsDirectory,
            WasiError::NotDirectory,
            WasiError::DirectoryNotEmpty,
            WasiError::NoSpaceLeft,
            WasiError::InvalidFd,
            WasiError::BadFd,
            WasiError::FdNotOpen,
            WasiError::NotSupported,
            WasiError::IoError("test".to_string()),
        ];

        for err in errors {
            let errno = to_wasi_errno(&err);
            assert!(errno > 0, "Error {:?} should have a valid errno", err);
        }
    }
}

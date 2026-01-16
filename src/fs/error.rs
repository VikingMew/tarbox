use thiserror::Error;

pub type FsResult<T> = Result<T, FsError>;

#[derive(Error, Debug)]
pub enum FsError {
    #[error("Path not found: {0}")]
    PathNotFound(String),

    #[error("File or directory already exists: {0}")]
    AlreadyExists(String),

    #[error("Not a directory: {0}")]
    NotDirectory(String),

    #[error("Is a directory: {0}")]
    IsDirectory(String),

    #[error("Directory not empty: {0}")]
    DirectoryNotEmpty(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Path too long: {0} bytes (max 4096)")]
    PathTooLong(usize),

    #[error("Filename too long: {0} bytes (max 255)")]
    FilenameTooLong(usize),

    #[error("Storage error: {0}")]
    Storage(#[from] anyhow::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_not_found_error() {
        let err = FsError::PathNotFound("/test/path".to_string());
        assert_eq!(err.to_string(), "Path not found: /test/path");
    }

    #[test]
    fn test_already_exists_error() {
        let err = FsError::AlreadyExists("/test/file".to_string());
        assert_eq!(err.to_string(), "File or directory already exists: /test/file");
    }

    #[test]
    fn test_not_directory_error() {
        let err = FsError::NotDirectory("/file.txt".to_string());
        assert_eq!(err.to_string(), "Not a directory: /file.txt");
    }

    #[test]
    fn test_is_directory_error() {
        let err = FsError::IsDirectory("/dir".to_string());
        assert_eq!(err.to_string(), "Is a directory: /dir");
    }

    #[test]
    fn test_directory_not_empty_error() {
        let err = FsError::DirectoryNotEmpty("/dir".to_string());
        assert_eq!(err.to_string(), "Directory not empty: /dir");
    }

    #[test]
    fn test_invalid_path_error() {
        let err = FsError::InvalidPath("empty".to_string());
        assert_eq!(err.to_string(), "Invalid path: empty");
    }

    #[test]
    fn test_path_too_long_error() {
        let err = FsError::PathTooLong(5000);
        assert_eq!(err.to_string(), "Path too long: 5000 bytes (max 4096)");
    }

    #[test]
    fn test_filename_too_long_error() {
        let err = FsError::FilenameTooLong(300);
        assert_eq!(err.to_string(), "Filename too long: 300 bytes (max 255)");
    }

    #[test]
    fn test_fs_result_ok() {
        fn get_value() -> FsResult<i32> {
            Ok(42)
        }
        let result = get_value();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_fs_result_err() {
        let result: FsResult<i32> = Err(FsError::PathNotFound("/test".to_string()));
        assert!(result.is_err());
    }
}

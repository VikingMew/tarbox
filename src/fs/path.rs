use crate::fs::error::{FsError, FsResult};

const MAX_PATH_LENGTH: usize = 4096;
const MAX_FILENAME_LENGTH: usize = 255;

pub fn normalize_path(path: &str) -> FsResult<String> {
    if path.is_empty() {
        return Err(FsError::InvalidPath("Empty path".to_string()));
    }

    if path.contains('\0') {
        return Err(FsError::InvalidPath("Path contains NULL character".to_string()));
    }

    if path.as_bytes().len() > MAX_PATH_LENGTH {
        return Err(FsError::PathTooLong(path.as_bytes().len()));
    }

    if !path.starts_with('/') {
        return Err(FsError::InvalidPath("Path must start with /".to_string()));
    }

    if path == "/" {
        return Ok("/".to_string());
    }

    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    for part in &parts {
        if part.as_bytes().len() > MAX_FILENAME_LENGTH {
            return Err(FsError::FilenameTooLong(part.as_bytes().len()));
        }
    }

    Ok(format!("/{}", parts.join("/")))
}

pub fn split_path(path: &str) -> FsResult<(String, String)> {
    let normalized = normalize_path(path)?;

    if normalized == "/" {
        return Err(FsError::InvalidPath("Cannot split root path".to_string()));
    }

    let parts: Vec<&str> = normalized.split('/').filter(|s| !s.is_empty()).collect();

    if parts.is_empty() {
        return Err(FsError::InvalidPath("Empty path after split".to_string()));
    }

    if parts.len() == 1 {
        return Ok(("/".to_string(), parts[0].to_string()));
    }

    let parent_parts = &parts[..parts.len() - 1];
    let filename = parts[parts.len() - 1];

    Ok((format!("/{}", parent_parts.join("/")), filename.to_string()))
}

pub fn path_components(path: &str) -> FsResult<Vec<String>> {
    let normalized = normalize_path(path)?;

    if normalized == "/" {
        return Ok(vec![]);
    }

    Ok(normalized.split('/').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path_root() {
        assert_eq!(normalize_path("/").unwrap(), "/");
    }

    #[test]
    fn test_normalize_path_single() {
        assert_eq!(normalize_path("/data").unwrap(), "/data");
    }

    #[test]
    fn test_normalize_path_multiple() {
        assert_eq!(normalize_path("/data/files").unwrap(), "/data/files");
    }

    #[test]
    fn test_normalize_path_trailing_slash() {
        assert_eq!(normalize_path("/data/").unwrap(), "/data");
    }

    #[test]
    fn test_normalize_path_multiple_slashes() {
        assert_eq!(normalize_path("//data//files//").unwrap(), "/data/files");
    }

    #[test]
    fn test_normalize_path_empty() {
        assert!(normalize_path("").is_err());
    }

    #[test]
    fn test_normalize_path_no_leading_slash() {
        assert!(normalize_path("data").is_err());
    }

    #[test]
    fn test_split_path_single() {
        let (parent, name) = split_path("/data").unwrap();
        assert_eq!(parent, "/");
        assert_eq!(name, "data");
    }

    #[test]
    fn test_split_path_multiple() {
        let (parent, name) = split_path("/data/files/test.txt").unwrap();
        assert_eq!(parent, "/data/files");
        assert_eq!(name, "test.txt");
    }

    #[test]
    fn test_split_path_root() {
        assert!(split_path("/").is_err());
    }

    #[test]
    fn test_path_components_root() {
        let components = path_components("/").unwrap();
        assert_eq!(components.len(), 0);
    }

    #[test]
    fn test_path_components_single() {
        let components = path_components("/data").unwrap();
        assert_eq!(components, vec!["data"]);
    }

    #[test]
    fn test_path_components_multiple() {
        let components = path_components("/data/files/test.txt").unwrap();
        assert_eq!(components, vec!["data", "files", "test.txt"]);
    }
}

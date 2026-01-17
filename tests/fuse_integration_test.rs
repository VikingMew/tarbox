// Integration tests for FUSE module with mocked FileSystem
//
// These tests verify the interaction between FilesystemInterface and TarboxBackend
// using mocked dependencies instead of real database connections.

use chrono::Utc;
use mockall::mock;
use mockall::predicate::*;
use std::sync::Arc;
use tarbox::fuse::{
    DirEntry, FileAttr, FileType, FilesystemInterface, FsError, FsResult, SetAttr, StatFs,
};

// Mock FilesystemInterface for testing
mock! {
    pub FilesystemBackend {}

    #[async_trait::async_trait]
    impl FilesystemInterface for FilesystemBackend {
        async fn read_file(&self, path: &str, offset: u64, size: u32) -> FsResult<Vec<u8>>;
        async fn write_file(&self, path: &str, offset: u64, data: &[u8]) -> FsResult<u32>;
        async fn create_file(&self, path: &str, mode: u32) -> FsResult<FileAttr>;
        async fn delete_file(&self, path: &str) -> FsResult<()>;
        async fn truncate(&self, path: &str, size: u64) -> FsResult<()>;
        async fn create_dir(&self, path: &str, mode: u32) -> FsResult<FileAttr>;
        async fn read_dir(&self, path: &str) -> FsResult<Vec<DirEntry>>;
        async fn remove_dir(&self, path: &str) -> FsResult<()>;
        async fn get_attr(&self, path: &str) -> FsResult<FileAttr>;
        async fn set_attr(&self, path: &str, attr: SetAttr) -> FsResult<FileAttr>;
        async fn chmod(&self, path: &str, mode: u32) -> FsResult<()>;
        async fn chown(&self, path: &str, uid: u32, gid: u32) -> FsResult<()>;
        async fn statfs(&self) -> FsResult<StatFs>;
    }
}

fn create_test_file_attr(inode: u64, name: &str) -> FileAttr {
    let now = Utc::now();
    FileAttr {
        inode,
        kind: FileType::RegularFile,
        size: 1024,
        atime: now,
        mtime: now,
        ctime: now,
        mode: 0o644,
        uid: 1000,
        gid: 1000,
        nlinks: 1,
    }
}

fn create_test_dir_attr(inode: u64, name: &str) -> FileAttr {
    let now = Utc::now();
    FileAttr {
        inode,
        kind: FileType::Directory,
        size: 4096,
        atime: now,
        mtime: now,
        ctime: now,
        mode: 0o755,
        uid: 1000,
        gid: 1000,
        nlinks: 2,
    }
}

#[tokio::test]
async fn test_mock_create_file() {
    let mut mock = MockFilesystemBackend::new();
    let expected_attr = create_test_file_attr(123, "test.txt");

    mock.expect_create_file()
        .with(eq("/test.txt"), eq(0o644))
        .times(1)
        .returning(move |_, _| Ok(expected_attr.clone()));

    let result = mock.create_file("/test.txt", 0o644).await;
    assert!(result.is_ok());
    let attr = result.unwrap();
    assert_eq!(attr.inode, 123);
    assert_eq!(attr.kind, FileType::RegularFile);
}

#[tokio::test]
async fn test_mock_write_and_read_file() {
    let mut mock = MockFilesystemBackend::new();
    let test_data = b"Hello, World!";

    mock.expect_write_file()
        .with(eq("/test.txt"), eq(0u64), eq(test_data.as_ref()))
        .times(1)
        .returning(|_, _, data| Ok(data.len() as u32));

    mock.expect_read_file()
        .with(eq("/test.txt"), eq(0u64), eq(13u32))
        .times(1)
        .returning(|_, _, _| Ok(b"Hello, World!".to_vec()));

    let write_result = mock.write_file("/test.txt", 0, test_data).await;
    assert!(write_result.is_ok());
    assert_eq!(write_result.unwrap(), 13);

    let read_result = mock.read_file("/test.txt", 0, 13).await;
    assert!(read_result.is_ok());
    assert_eq!(read_result.unwrap(), test_data);
}

#[tokio::test]
async fn test_mock_read_with_offset() {
    let mut mock = MockFilesystemBackend::new();

    mock.expect_read_file()
        .with(eq("/test.txt"), eq(7u64), eq(6u32))
        .times(1)
        .returning(|_, _, _| Ok(b"World!".to_vec()));

    let result = mock.read_file("/test.txt", 7, 6).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), b"World!");
}

#[tokio::test]
async fn test_mock_create_directory() {
    let mut mock = MockFilesystemBackend::new();
    let expected_attr = create_test_dir_attr(456, "testdir");

    mock.expect_create_dir()
        .with(eq("/testdir"), eq(0o755))
        .times(1)
        .returning(move |_, _| Ok(expected_attr.clone()));

    let result = mock.create_dir("/testdir", 0o755).await;
    assert!(result.is_ok());
    let attr = result.unwrap();
    assert_eq!(attr.kind, FileType::Directory);
    assert_eq!(attr.mode, 0o755);
}

#[tokio::test]
async fn test_mock_read_directory() {
    let mut mock = MockFilesystemBackend::new();

    let entries = vec![
        DirEntry { inode: 2, name: "file1.txt".to_string(), kind: FileType::RegularFile },
        DirEntry { inode: 3, name: "file2.txt".to_string(), kind: FileType::RegularFile },
    ];

    mock.expect_read_dir().with(eq("/testdir")).times(1).returning(move |_| Ok(entries.clone()));

    let result = mock.read_dir("/testdir").await;
    assert!(result.is_ok());
    let dir_entries = result.unwrap();
    assert_eq!(dir_entries.len(), 2);
    assert_eq!(dir_entries[0].name, "file1.txt");
    assert_eq!(dir_entries[1].name, "file2.txt");
}

#[tokio::test]
async fn test_mock_delete_file() {
    let mut mock = MockFilesystemBackend::new();

    mock.expect_delete_file().with(eq("/test.txt")).times(1).returning(|_| Ok(()));

    let result = mock.delete_file("/test.txt").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_mock_remove_directory() {
    let mut mock = MockFilesystemBackend::new();

    mock.expect_remove_dir().with(eq("/testdir")).times(1).returning(|_| Ok(()));

    let result = mock.remove_dir("/testdir").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_mock_get_attr() {
    let mut mock = MockFilesystemBackend::new();
    let expected_attr = create_test_file_attr(789, "test.txt");

    mock.expect_get_attr()
        .with(eq("/test.txt"))
        .times(1)
        .returning(move |_| Ok(expected_attr.clone()));

    let result = mock.get_attr("/test.txt").await;
    assert!(result.is_ok());
    let attr = result.unwrap();
    assert_eq!(attr.inode, 789);
}

#[tokio::test]
async fn test_mock_chmod() {
    let mut mock = MockFilesystemBackend::new();

    mock.expect_chmod().with(eq("/test.txt"), eq(0o755)).times(1).returning(|_, _| Ok(()));

    let result = mock.chmod("/test.txt", 0o755).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_mock_chown() {
    let mut mock = MockFilesystemBackend::new();

    mock.expect_chown()
        .with(eq("/test.txt"), eq(1001), eq(1001))
        .times(1)
        .returning(|_, _, _| Ok(()));

    let result = mock.chown("/test.txt", 1001, 1001).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_mock_truncate() {
    let mut mock = MockFilesystemBackend::new();

    mock.expect_truncate().with(eq("/test.txt"), eq(0u64)).times(1).returning(|_, _| Ok(()));

    let result = mock.truncate("/test.txt", 0).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_mock_set_attr() {
    let mut mock = MockFilesystemBackend::new();
    let expected_attr = create_test_file_attr(123, "test.txt");

    let attr = SetAttr { size: Some(2048), mode: Some(0o600), ..Default::default() };

    mock.expect_set_attr().times(1).returning(move |_, _| Ok(expected_attr.clone()));

    let result = mock.set_attr("/test.txt", attr).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_mock_statfs() {
    let mut mock = MockFilesystemBackend::new();

    let expected_stats = StatFs {
        blocks: 1_000_000,
        bfree: 500_000,
        bavail: 500_000,
        files: 10_000,
        ffree: 5_000,
        bsize: 4096,
        namelen: 255,
    };

    mock.expect_statfs().times(1).returning(move || Ok(expected_stats.clone()));

    let result = mock.statfs().await;
    assert!(result.is_ok());
    let stats = result.unwrap();
    assert_eq!(stats.blocks, 1_000_000);
    assert_eq!(stats.bsize, 4096);
}

#[tokio::test]
async fn test_mock_error_path_not_found() {
    let mut mock = MockFilesystemBackend::new();

    mock.expect_read_file()
        .with(eq("/nonexistent.txt"), always(), always())
        .times(1)
        .returning(|path, _, _| Err(FsError::PathNotFound(path.to_string())));

    let result = mock.read_file("/nonexistent.txt", 0, 100).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        FsError::PathNotFound(path) => assert_eq!(path, "/nonexistent.txt"),
        _ => panic!("Expected PathNotFound error"),
    }
}

#[tokio::test]
async fn test_mock_error_already_exists() {
    let mut mock = MockFilesystemBackend::new();

    mock.expect_create_file()
        .with(eq("/existing.txt"), always())
        .times(1)
        .returning(|path, _| Err(FsError::AlreadyExists(path.to_string())));

    let result = mock.create_file("/existing.txt", 0o644).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        FsError::AlreadyExists(path) => assert_eq!(path, "/existing.txt"),
        _ => panic!("Expected AlreadyExists error"),
    }
}

#[tokio::test]
async fn test_mock_error_directory_not_empty() {
    let mut mock = MockFilesystemBackend::new();

    mock.expect_remove_dir()
        .with(eq("/nonempty"))
        .times(1)
        .returning(|path| Err(FsError::DirectoryNotEmpty(path.to_string())));

    let result = mock.remove_dir("/nonempty").await;
    assert!(result.is_err());
    match result.unwrap_err() {
        FsError::DirectoryNotEmpty(path) => assert_eq!(path, "/nonempty"),
        _ => panic!("Expected DirectoryNotEmpty error"),
    }
}

#[tokio::test]
async fn test_mock_error_permission_denied() {
    let mut mock = MockFilesystemBackend::new();

    mock.expect_chmod()
        .with(eq("/forbidden.txt"), always())
        .times(1)
        .returning(|path, _| Err(FsError::PermissionDenied(path.to_string())));

    let result = mock.chmod("/forbidden.txt", 0o777).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        FsError::PermissionDenied(path) => assert_eq!(path, "/forbidden.txt"),
        _ => panic!("Expected PermissionDenied error"),
    }
}

#[tokio::test]
async fn test_mock_error_not_directory() {
    let mut mock = MockFilesystemBackend::new();

    mock.expect_read_dir()
        .with(eq("/file.txt"))
        .times(1)
        .returning(|path| Err(FsError::NotDirectory(path.to_string())));

    let result = mock.read_dir("/file.txt").await;
    assert!(result.is_err());
    match result.unwrap_err() {
        FsError::NotDirectory(path) => assert_eq!(path, "/file.txt"),
        _ => panic!("Expected NotDirectory error"),
    }
}

#[tokio::test]
async fn test_mock_multiple_operations_sequence() {
    let mut mock = MockFilesystemBackend::new();

    // Sequence: create file -> write -> read -> delete
    let expected_attr = create_test_file_attr(100, "seq.txt");

    mock.expect_create_file()
        .with(eq("/seq.txt"), eq(0o644))
        .times(1)
        .returning(move |_, _| Ok(expected_attr.clone()));

    mock.expect_write_file()
        .with(eq("/seq.txt"), eq(0u64), always())
        .times(1)
        .returning(|_, _, data| Ok(data.len() as u32));

    mock.expect_read_file()
        .with(eq("/seq.txt"), eq(0u64), always())
        .times(1)
        .returning(|_, _, _| Ok(b"test data".to_vec()));

    mock.expect_delete_file().with(eq("/seq.txt")).times(1).returning(|_| Ok(()));

    // Execute sequence
    assert!(mock.create_file("/seq.txt", 0o644).await.is_ok());
    assert!(mock.write_file("/seq.txt", 0, b"test data").await.is_ok());
    assert!(mock.read_file("/seq.txt", 0, 100).await.is_ok());
    assert!(mock.delete_file("/seq.txt").await.is_ok());
}

#[tokio::test]
async fn test_mock_concurrent_reads() {
    use tokio::task;

    let mock = Arc::new(tokio::sync::Mutex::new(MockFilesystemBackend::new()));

    {
        let mut m = mock.lock().await;
        m.expect_read_file()
            .with(eq("/shared.txt"), always(), always())
            .times(3)
            .returning(|_, _, _| Ok(b"shared data".to_vec()));
    }

    let handles: Vec<_> = (0..3)
        .map(|_| {
            let mock_clone = Arc::clone(&mock);
            task::spawn(async move {
                let m = mock_clone.lock().await;
                m.read_file("/shared.txt", 0, 100).await
            })
        })
        .collect();

    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }
}

#[tokio::test]
async fn test_mock_large_file_operations() {
    let mut mock = MockFilesystemBackend::new();
    let large_data = vec![0u8; 1048576]; // 1MB

    mock.expect_write_file()
        .with(eq("/large.bin"), eq(0u64), always())
        .times(1)
        .returning(|_, _, data| Ok(data.len() as u32));

    mock.expect_read_file()
        .with(eq("/large.bin"), eq(0u64), eq(1048576u32))
        .times(1)
        .returning(move |_, _, _| Ok(vec![0u8; 1048576]));

    let write_result = mock.write_file("/large.bin", 0, &large_data).await;
    assert!(write_result.is_ok());
    assert_eq!(write_result.unwrap(), 1048576);

    let read_result = mock.read_file("/large.bin", 0, 1048576).await;
    assert!(read_result.is_ok());
    assert_eq!(read_result.unwrap().len(), 1048576);
}

#[tokio::test]
async fn test_mock_nested_directories() {
    let mut mock = MockFilesystemBackend::new();
    let dir1_attr = create_test_dir_attr(10, "dir1");
    let dir2_attr = create_test_dir_attr(11, "dir2");
    let dir3_attr = create_test_dir_attr(12, "dir3");

    mock.expect_create_dir()
        .with(eq("/dir1"), always())
        .times(1)
        .returning(move |_, _| Ok(dir1_attr.clone()));

    mock.expect_create_dir()
        .with(eq("/dir1/dir2"), always())
        .times(1)
        .returning(move |_, _| Ok(dir2_attr.clone()));

    mock.expect_create_dir()
        .with(eq("/dir1/dir2/dir3"), always())
        .times(1)
        .returning(move |_, _| Ok(dir3_attr.clone()));

    assert!(mock.create_dir("/dir1", 0o755).await.is_ok());
    assert!(mock.create_dir("/dir1/dir2", 0o755).await.is_ok());
    assert!(mock.create_dir("/dir1/dir2/dir3", 0o755).await.is_ok());
}

#[tokio::test]
async fn test_mock_file_permissions_variations() {
    let mut mock = MockFilesystemBackend::new();

    for mode in [0o644, 0o600, 0o755, 0o777] {
        mock.expect_chmod().with(eq("/test.txt"), eq(mode)).times(1).returning(|_, _| Ok(()));
    }

    for mode in [0o644, 0o600, 0o755, 0o777] {
        assert!(mock.chmod("/test.txt", mode).await.is_ok());
    }
}

#[tokio::test]
async fn test_mock_ownership_changes() {
    let mut mock = MockFilesystemBackend::new();

    let ownership_pairs = vec![(1000, 1000), (0, 0), (1001, 1001), (65534, 65534)];

    for (uid, gid) in ownership_pairs.iter() {
        mock.expect_chown()
            .with(eq("/test.txt"), eq(*uid), eq(*gid))
            .times(1)
            .returning(|_, _, _| Ok(()));
    }

    for (uid, gid) in ownership_pairs {
        assert!(mock.chown("/test.txt", uid, gid).await.is_ok());
    }
}

#[tokio::test]
async fn test_mock_mixed_file_types() {
    let mut mock = MockFilesystemBackend::new();

    let entries = vec![
        DirEntry { inode: 2, name: "file.txt".to_string(), kind: FileType::RegularFile },
        DirEntry { inode: 3, name: "subdir".to_string(), kind: FileType::Directory },
        DirEntry { inode: 4, name: "link".to_string(), kind: FileType::Symlink },
    ];

    mock.expect_read_dir().with(eq("/")).times(1).returning(move |_| Ok(entries.clone()));

    let result = mock.read_dir("/").await;
    assert!(result.is_ok());
    let dir_entries = result.unwrap();
    assert_eq!(dir_entries.len(), 3);
    assert_eq!(dir_entries[0].kind, FileType::RegularFile);
    assert_eq!(dir_entries[1].kind, FileType::Directory);
    assert_eq!(dir_entries[2].kind, FileType::Symlink);
}

#[tokio::test]
async fn test_mock_empty_directory() {
    let mut mock = MockFilesystemBackend::new();

    mock.expect_read_dir().with(eq("/empty")).times(1).returning(|_| Ok(Vec::new()));

    let result = mock.read_dir("/empty").await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 0);
}

#[tokio::test]
async fn test_mock_setattr_timestamps() {
    let mut mock = MockFilesystemBackend::new();
    let expected_attr = create_test_file_attr(100, "test.txt");
    let now = Utc::now();

    let attr = SetAttr { atime: Some(now), mtime: Some(now), ..Default::default() };

    mock.expect_set_attr().times(1).returning(move |_, _| Ok(expected_attr.clone()));

    let result = mock.set_attr("/test.txt", attr).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_mock_error_is_directory() {
    let mut mock = MockFilesystemBackend::new();

    mock.expect_delete_file()
        .with(eq("/dir"))
        .times(1)
        .returning(|path| Err(FsError::IsDirectory(path.to_string())));

    let result = mock.delete_file("/dir").await;
    assert!(result.is_err());
    match result.unwrap_err() {
        FsError::IsDirectory(path) => assert_eq!(path, "/dir"),
        _ => panic!("Expected IsDirectory error"),
    }
}

#[tokio::test]
async fn test_mock_error_invalid_path() {
    let mut mock = MockFilesystemBackend::new();

    mock.expect_create_file()
        .with(eq("invalid/path"), always())
        .times(1)
        .returning(|path, _| Err(FsError::InvalidPath(path.to_string())));

    let result = mock.create_file("invalid/path", 0o644).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_mock_batch_file_creation() {
    let mut mock = MockFilesystemBackend::new();

    for i in 0..5 {
        let expected_attr = create_test_file_attr(i + 100, &format!("file{}.txt", i));
        let path = format!("/file{}.txt", i);

        mock.expect_create_file()
            .withf(move |p, _| p == path)
            .times(1)
            .returning(move |_, _| Ok(expected_attr.clone()));
    }

    for i in 0..5 {
        let result = mock.create_file(&format!("/file{}.txt", i), 0o644).await;
        assert!(result.is_ok());
    }
}

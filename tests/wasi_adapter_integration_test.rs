// Integration tests for WASI adapter

use tarbox::wasi::{FdTable, FileDescriptor, OpenFlags, WasiConfig, WasiError};
use uuid::Uuid;

// Note: These tests verify the WASI module components work correctly.
// Full E2E tests with a real filesystem would require a database connection.

#[tokio::test]
async fn test_wasi_adapter_construction() {
    let config = WasiConfig::default();
    let tenant_id = Uuid::new_v4();

    // Note: We can't easily construct a real WasiAdapter here without a database
    // This test just verifies the config can be created
    assert_eq!(config.cache_size_mb, 100);
    assert_eq!(config.cache_ttl_secs, 300);
    assert_eq!(config.tenant_id, None);
    assert!(!tenant_id.to_string().is_empty());
}

#[tokio::test]
async fn test_wasi_adapter_with_http_config() {
    let config = WasiConfig::http("https://api.tarbox.io".to_string(), Some("key".to_string()));
    assert_eq!(config.api_url, Some("https://api.tarbox.io".to_string()));
    assert_eq!(config.api_key, Some("key".to_string()));
}

#[tokio::test]
async fn test_open_flags_for_wasi_operations() {
    let read_flags = OpenFlags::read_only();
    assert!(read_flags.read);
    assert!(!read_flags.write);

    let write_flags = OpenFlags::write_only();
    assert!(!write_flags.read);
    assert!(write_flags.write);

    let rw_flags = OpenFlags::read_write();
    assert!(rw_flags.read);
    assert!(rw_flags.write);
}

#[tokio::test]
async fn test_wasi_error_conversions() {
    use tarbox::fs::error::FsError;

    let fs_err = FsError::PathNotFound("/test".to_string());
    let wasi_err: WasiError = fs_err.into();
    assert!(matches!(wasi_err, WasiError::NotFound));

    let fs_err = FsError::AlreadyExists("/test".to_string());
    let wasi_err: WasiError = fs_err.into();
    assert!(matches!(wasi_err, WasiError::AlreadyExists));
}

#[test]
fn test_wasi_module_structure() {
    // Verify all WASI module components are accessible
    use tarbox::wasi::DbMode;

    let _config = WasiConfig::default();
    let _fd_table = FdTable::new();
    let _flags = OpenFlags::default();
    let _db_mode = DbMode::Http;

    // Verify error types exist
    let _err: WasiError = WasiError::NotFound;

    // Verify FileDescriptor can be created
    let descriptor = FileDescriptor::new(1, "/test".to_string(), OpenFlags::default(), false);
    assert_eq!(descriptor.path, "/test");
    assert_eq!(descriptor.inode_id, 1);
}

#[test]
fn test_wasi_config_builder_combinations() {
    let tenant_id = Uuid::new_v4();

    let config = WasiConfig::http("https://api.tarbox.io".to_string(), Some("key".to_string()))
        .with_tenant_id(tenant_id)
        .with_cache_size(256)
        .with_cache_ttl(600);

    assert_eq!(config.cache_size_mb, 256);
    assert_eq!(config.cache_ttl_secs, 600);
    assert_eq!(config.tenant_id, Some(tenant_id));
}

#[test]
fn test_fd_table_lifecycle() {
    let mut fd_table = FdTable::new();
    let flags = OpenFlags::read_only();
    let descriptor = FileDescriptor::new(1, "/test.txt".to_string(), flags, false);

    // Allocate FD
    let fd = fd_table.allocate(descriptor.clone());
    assert!(fd >= 3); // FDs 0-2 are reserved

    // Get FD
    assert!(fd_table.get(fd).is_ok());

    // Close FD
    assert!(fd_table.close(fd).is_ok());

    // Verify FD is closed
    assert!(fd_table.get(fd).is_err());
}

#[test]
fn test_file_descriptor_seeking() {
    let flags = OpenFlags::read_only();
    let mut descriptor = FileDescriptor::new(1, "/test.txt".to_string(), flags, false);

    // Seek to absolute position (SEEK_SET)
    assert!(descriptor.seek(10, 0).is_ok());
    assert_eq!(descriptor.position, 10);

    // Seek relative forward (SEEK_CUR)
    assert!(descriptor.seek(5, 1).is_ok());
    assert_eq!(descriptor.position, 15);

    // Seek relative backward (SEEK_CUR)
    assert!(descriptor.seek(-5, 1).is_ok());
    assert_eq!(descriptor.position, 10);

    // Seek end not supported (SEEK_END)
    assert!(descriptor.seek(0, 2).is_err());
}

#[test]
fn test_wasi_error_display() {
    let err = WasiError::NotFound;
    let display = format!("{}", err);
    assert!(display.contains("not found") || display.contains("Not found"));

    let err = WasiError::PermissionDenied;
    let display = format!("{}", err);
    assert!(display.contains("Permission denied") || display.contains("permission denied"));
}

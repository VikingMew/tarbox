use anyhow::Result;
use tarbox::config::DatabaseConfig;
use tarbox::fs::error::FsError;
use tarbox::fs::operations::FileSystem;
use tarbox::storage::{CreateTenantInput, DatabasePool, TenantOperations, TenantRepository};

async fn setup_test_db() -> Result<DatabasePool> {
    let config = DatabaseConfig {
        url: std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/tarbox_test".into()),
        max_connections: 5,
        min_connections: 1,
    };

    let pool = DatabasePool::new(&config).await?;
    pool.run_migrations().await?;
    Ok(pool)
}

async fn cleanup_tenant(pool: &DatabasePool, tenant_name: &str) -> Result<()> {
    let tenant_ops = TenantOperations::new(pool.pool());
    if let Some(tenant) = tenant_ops.get_by_name(tenant_name).await? {
        tenant_ops.delete(tenant.tenant_id).await?;
    }
    Ok(())
}

#[tokio::test]
async fn test_resolve_root_path() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_resolve_root_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id);

    let root = fs.resolve_path("/").await?;
    assert_eq!(root.name, "/");
    assert_eq!(root.inode_id, tenant.root_inode_id);

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_create_and_resolve_directory() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_create_dir_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id);

    let dir = fs.create_directory("/test_dir").await?;
    assert_eq!(dir.name, "test_dir");

    let resolved = fs.resolve_path("/test_dir").await?;
    assert_eq!(resolved.inode_id, dir.inode_id);
    assert_eq!(resolved.name, "test_dir");

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_create_nested_directories() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_nested_dir_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id);

    fs.create_directory("/parent").await?;
    let child = fs.create_directory("/parent/child").await?;

    assert_eq!(child.name, "child");

    let resolved = fs.resolve_path("/parent/child").await?;
    assert_eq!(resolved.inode_id, child.inode_id);

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_create_directory_already_exists() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_dir_exists_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id);

    fs.create_directory("/duplicate").await?;
    let result = fs.create_directory("/duplicate").await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), FsError::AlreadyExists(_)));

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_list_directory() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_list_dir_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id);

    fs.create_directory("/dir1").await?;
    fs.create_directory("/dir2").await?;
    fs.create_file("/file1.txt").await?;

    let entries = fs.list_directory("/").await?;
    assert_eq!(entries.len(), 3);

    let names: Vec<String> = entries.iter().map(|e| e.name.clone()).collect();
    assert!(names.contains(&"dir1".to_string()));
    assert!(names.contains(&"dir2".to_string()));
    assert!(names.contains(&"file1.txt".to_string()));

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_remove_empty_directory() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_remove_dir_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id);

    fs.create_directory("/empty_dir").await?;
    fs.remove_directory("/empty_dir").await?;

    let result = fs.resolve_path("/empty_dir").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), FsError::PathNotFound(_)));

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_remove_non_empty_directory_fails() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_remove_nonempty_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id);

    fs.create_directory("/parent").await?;
    fs.create_file("/parent/child.txt").await?;

    let result = fs.remove_directory("/parent").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), FsError::DirectoryNotEmpty(_)));

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_create_and_read_file() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_create_file_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id);

    let file = fs.create_file("/test.txt").await?;
    assert_eq!(file.name, "test.txt");

    let resolved = fs.resolve_path("/test.txt").await?;
    assert_eq!(resolved.inode_id, file.inode_id);

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_write_and_read_file() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_write_file_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id);

    fs.create_file("/data.txt").await?;

    let test_data = b"Hello, Tarbox!";
    fs.write_file("/data.txt", test_data).await?;

    let read_data = fs.read_file("/data.txt").await?;
    assert_eq!(read_data, test_data);

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_write_large_file() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_large_file_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id);

    fs.create_file("/large.bin").await?;

    // Create 10KB of data (spans multiple 4KB blocks)
    let test_data: Vec<u8> = (0..10240).map(|i| (i % 256) as u8).collect();
    fs.write_file("/large.bin", &test_data).await?;

    let read_data = fs.read_file("/large.bin").await?;
    assert_eq!(read_data.len(), test_data.len());
    assert_eq!(read_data, test_data);

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_overwrite_file() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_overwrite_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id);

    fs.create_file("/overwrite.txt").await?;

    fs.write_file("/overwrite.txt", b"First content").await?;
    let first_read = fs.read_file("/overwrite.txt").await?;
    assert_eq!(first_read, b"First content");

    fs.write_file("/overwrite.txt", b"Second content - longer").await?;
    let second_read = fs.read_file("/overwrite.txt").await?;
    assert_eq!(second_read, b"Second content - longer");

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_delete_file() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_delete_file_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id);

    fs.create_file("/delete_me.txt").await?;
    fs.write_file("/delete_me.txt", b"Some data").await?;

    fs.delete_file("/delete_me.txt").await?;

    let result = fs.resolve_path("/delete_me.txt").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), FsError::PathNotFound(_)));

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_stat_file() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_stat_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id);

    fs.create_file("/stat_me.txt").await?;
    let test_data = b"Test data for stat";
    fs.write_file("/stat_me.txt", test_data).await?;

    let stat = fs.stat("/stat_me.txt").await?;
    assert_eq!(stat.name, "stat_me.txt");
    assert_eq!(stat.size, test_data.len() as i64);

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_chmod() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_chmod_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id);

    fs.create_file("/chmod_test.txt").await?;

    let original_stat = fs.stat("/chmod_test.txt").await?;
    assert_eq!(original_stat.mode, 0o644);

    fs.chmod("/chmod_test.txt", 0o755).await?;

    let updated_stat = fs.stat("/chmod_test.txt").await?;
    assert_eq!(updated_stat.mode, 0o755);

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_chown() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_chown_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id);

    fs.create_file("/chown_test.txt").await?;

    fs.chown("/chown_test.txt", 1001, 1001).await?;

    let stat = fs.stat("/chown_test.txt").await?;
    assert_eq!(stat.uid, 1001);
    assert_eq!(stat.gid, 1001);

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_path_not_found() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_not_found_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id);

    let result = fs.resolve_path("/nonexistent/path").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), FsError::PathNotFound(_)));

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_create_file_in_nonexistent_directory() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_no_parent_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id);

    let result = fs.create_file("/nonexistent/file.txt").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), FsError::PathNotFound(_)));

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_write_to_directory_fails() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_write_dir_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id);

    fs.create_directory("/testdir").await?;

    let result = fs.write_file("/testdir", b"data").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), FsError::IsDirectory(_)));

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_read_directory_fails() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_read_dir_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id);

    fs.create_directory("/testdir").await?;

    let result = fs.read_file("/testdir").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), FsError::IsDirectory(_)));

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_delete_directory_as_file_fails() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_delete_dir_as_file_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id);

    fs.create_directory("/testdir").await?;

    let result = fs.delete_file("/testdir").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), FsError::IsDirectory(_)));

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

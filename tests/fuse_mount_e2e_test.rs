// FUSE mount E2E tests - requires FUSE permissions and real mount point
// Run with: sudo -E cargo test --test fuse_mount_e2e_test

use anyhow::Result;
use std::fs;
use std::io::{Read, Write};
use std::sync::Arc;
use tarbox::config::DatabaseConfig;
use tarbox::fuse::mount::{MountOptions, mount, unmount};
use tarbox::storage::{CreateTenantInput, DatabasePool, TenantOperations, TenantRepository};
use tempfile::TempDir;

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
#[ignore] // Requires FUSE permissions
async fn test_mount_and_unmount() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_mount_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let mountpoint = TempDir::new()?;
    let mount_path = mountpoint.path().to_path_buf();

    let options = MountOptions {
        allow_other: false,
        allow_root: false,
        read_only: false,
        fsname: Some("tarbox_test".to_string()),
        auto_unmount: true,
    };

    let session = mount(Arc::new(pool.pool().clone()), tenant.tenant_id, &mount_path, options)?;

    // Wait a bit for mount to complete
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Verify mount
    let entries = fs::read_dir(&mount_path)?;
    assert!(entries.count() == 0); // Empty root directory

    // Unmount
    drop(session);
    unmount(&mount_path)?;

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
#[ignore] // Requires FUSE permissions
async fn test_fuse_create_file() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_fuse_create_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let mountpoint = TempDir::new()?;
    let mount_path = mountpoint.path().to_path_buf();

    let options = MountOptions::default();
    let session = mount(Arc::new(pool.pool().clone()), tenant.tenant_id, &mount_path, options)?;

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Create file through FUSE
    let test_file = mount_path.join("test.txt");
    fs::File::create(&test_file)?;

    // Verify file exists
    assert!(test_file.exists());

    drop(session);
    unmount(&mount_path)?;
    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
#[ignore] // Requires FUSE permissions
async fn test_fuse_write_and_read() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_fuse_io_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let mountpoint = TempDir::new()?;
    let mount_path = mountpoint.path().to_path_buf();

    let options = MountOptions::default();
    let session = mount(Arc::new(pool.pool().clone()), tenant.tenant_id, &mount_path, options)?;

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Write through FUSE
    let test_file = mount_path.join("data.txt");
    let test_data = b"Hello, FUSE!";
    let mut file = fs::File::create(&test_file)?;
    file.write_all(test_data)?;
    drop(file);

    // Read through FUSE
    let mut file = fs::File::open(&test_file)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    assert_eq!(buffer, test_data);

    drop(session);
    unmount(&mount_path)?;
    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
#[ignore] // Requires FUSE permissions
async fn test_fuse_mkdir_and_readdir() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_fuse_dir_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let mountpoint = TempDir::new()?;
    let mount_path = mountpoint.path().to_path_buf();

    let options = MountOptions::default();
    let session = mount(Arc::new(pool.pool().clone()), tenant.tenant_id, &mount_path, options)?;

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Create directory through FUSE
    let test_dir = mount_path.join("testdir");
    fs::create_dir(&test_dir)?;

    // Create files in directory
    fs::File::create(test_dir.join("file1.txt"))?;
    fs::File::create(test_dir.join("file2.txt"))?;

    // Read directory through FUSE
    let entries: Vec<String> = fs::read_dir(&test_dir)?
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();

    assert_eq!(entries.len(), 2);
    assert!(entries.contains(&"file1.txt".to_string()));
    assert!(entries.contains(&"file2.txt".to_string()));

    drop(session);
    unmount(&mount_path)?;
    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
#[ignore] // Requires FUSE permissions
async fn test_fuse_delete_file() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_fuse_delete_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let mountpoint = TempDir::new()?;
    let mount_path = mountpoint.path().to_path_buf();

    let options = MountOptions::default();
    let session = mount(Arc::new(pool.pool().clone()), tenant.tenant_id, &mount_path, options)?;

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Create and delete file
    let test_file = mount_path.join("delete_me.txt");
    fs::File::create(&test_file)?;
    assert!(test_file.exists());

    fs::remove_file(&test_file)?;
    assert!(!test_file.exists());

    drop(session);
    unmount(&mount_path)?;
    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
#[ignore] // Requires FUSE permissions
async fn test_fuse_metadata() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_fuse_meta_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let mountpoint = TempDir::new()?;
    let mount_path = mountpoint.path().to_path_buf();

    let options = MountOptions::default();
    let session = mount(Arc::new(pool.pool().clone()), tenant.tenant_id, &mount_path, options)?;

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Create file
    let test_file = mount_path.join("meta.txt");
    let mut file = fs::File::create(&test_file)?;
    file.write_all(b"test")?;
    drop(file);

    // Get metadata
    let metadata = fs::metadata(&test_file)?;
    assert!(metadata.is_file());
    assert_eq!(metadata.len(), 4);

    drop(session);
    unmount(&mount_path)?;
    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
#[ignore] // Requires FUSE permissions
async fn test_fuse_chmod() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_fuse_chmod_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let mountpoint = TempDir::new()?;
    let mount_path = mountpoint.path().to_path_buf();

    let options = MountOptions::default();
    let session = mount(Arc::new(pool.pool().clone()), tenant.tenant_id, &mount_path, options)?;

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Create file
    let test_file = mount_path.join("chmod.txt");
    fs::File::create(&test_file)?;

    // Change permissions through FUSE
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o755);
        fs::set_permissions(&test_file, perms)?;

        let metadata = fs::metadata(&test_file)?;
        assert_eq!(metadata.permissions().mode() & 0o777, 0o755);
    }

    drop(session);
    unmount(&mount_path)?;
    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
#[ignore] // Requires FUSE permissions
async fn test_fuse_nested_directories() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_fuse_nested_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let mountpoint = TempDir::new()?;
    let mount_path = mountpoint.path().to_path_buf();

    let options = MountOptions::default();
    let session = mount(Arc::new(pool.pool().clone()), tenant.tenant_id, &mount_path, options)?;

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Create nested directories
    let nested_path = mount_path.join("a").join("b").join("c");
    fs::create_dir_all(&nested_path)?;

    // Verify structure
    assert!(mount_path.join("a").is_dir());
    assert!(mount_path.join("a/b").is_dir());
    assert!(nested_path.is_dir());

    drop(session);
    unmount(&mount_path)?;
    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
#[ignore] // Requires FUSE permissions
async fn test_fuse_large_file() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_fuse_large_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let mountpoint = TempDir::new()?;
    let mount_path = mountpoint.path().to_path_buf();

    let options = MountOptions::default();
    let session = mount(Arc::new(pool.pool().clone()), tenant.tenant_id, &mount_path, options)?;

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Write large file (100KB)
    let test_file = mount_path.join("large.bin");
    let large_data: Vec<u8> = (0..102400).map(|i| (i % 256) as u8).collect();

    let mut file = fs::File::create(&test_file)?;
    file.write_all(&large_data)?;
    drop(file);

    // Read back
    let mut file = fs::File::open(&test_file)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    assert_eq!(buffer.len(), large_data.len());
    assert_eq!(buffer, large_data);

    drop(session);
    unmount(&mount_path)?;
    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
#[ignore] // Requires FUSE permissions
async fn test_fuse_rename() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_fuse_rename_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let mountpoint = TempDir::new()?;
    let mount_path = mountpoint.path().to_path_buf();

    let options = MountOptions::default();
    let session = mount(Arc::new(pool.pool().clone()), tenant.tenant_id, &mount_path, options)?;

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Create file
    let old_path = mount_path.join("old.txt");
    let new_path = mount_path.join("new.txt");

    let mut file = fs::File::create(&old_path)?;
    file.write_all(b"test data")?;
    drop(file);

    // Rename through FUSE
    fs::rename(&old_path, &new_path)?;

    assert!(!old_path.exists());
    assert!(new_path.exists());

    // Verify content preserved
    let mut file = fs::File::open(&new_path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    assert_eq!(buffer, b"test data");

    drop(session);
    unmount(&mount_path)?;
    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
#[ignore] // Requires FUSE permissions
async fn test_fuse_rmdir() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_fuse_rmdir_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let mountpoint = TempDir::new()?;
    let mount_path = mountpoint.path().to_path_buf();

    let options = MountOptions::default();
    let session = mount(Arc::new(pool.pool().clone()), tenant.tenant_id, &mount_path, options)?;

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Create and remove empty directory
    let test_dir = mount_path.join("emptydir");
    fs::create_dir(&test_dir)?;
    assert!(test_dir.exists());

    fs::remove_dir(&test_dir)?;
    assert!(!test_dir.exists());

    drop(session);
    unmount(&mount_path)?;
    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

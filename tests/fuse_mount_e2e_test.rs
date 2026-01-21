// FUSE mount E2E tests - requires FUSE permissions and real mount point
// Run with: cargo test --test fuse_mount_e2e_test -- --ignored

use anyhow::Result;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use tarbox::config::DatabaseConfig;
use tarbox::fuse::backend::TarboxBackend;
use tarbox::fuse::mount::{MountOptions, mount, unmount};
use tarbox::storage::{CreateTenantInput, DatabasePool, TenantOperations, TenantRepository};
use tempfile::TempDir;

async fn setup_test_db() -> Result<DatabasePool> {
    let config = DatabaseConfig {
        url: std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/tarbox".into()),
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

/// Run blocking filesystem operations in a separate thread to avoid deadlock with FUSE.
/// FUSE callbacks use block_in_place which can deadlock if called from the same tokio runtime.
async fn blocking<F, T>(f: F) -> Result<T>
where
    F: FnOnce() -> Result<T> + Send + 'static,
    T: Send + 'static,
{
    let handle = tokio::task::spawn_blocking(f);
    tokio::task::yield_now().await;
    handle.await?
}

async fn do_unmount(path: PathBuf) -> Result<()> {
    // Try to unmount, but ignore errors if already unmounted (by session drop)
    let _ = blocking(move || unmount(&path)).await;
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

    let options = MountOptions::default();
    let backend =
        Arc::new(TarboxBackend::new(Arc::new(pool.pool().clone()), tenant.tenant_id).await?);

    let session = mount(backend, &mount_path, options)?;

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Verify mount - use spawn_blocking to avoid deadlock with FUSE
    let path_clone = mount_path.clone();
    let entries: Vec<String> = blocking(move || {
        Ok(fs::read_dir(&path_clone)?
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect())
    })
    .await?;
    // Root directory should only contain the virtual .tarbox entry
    assert_eq!(entries.len(), 1);
    assert!(entries.contains(&".tarbox".to_string()));

    drop(session);
    do_unmount(mount_path).await?;

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
    let backend =
        Arc::new(TarboxBackend::new(Arc::new(pool.pool().clone()), tenant.tenant_id).await?);

    let session = mount(backend, &mount_path, options)?;

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Create file through FUSE
    let test_file = mount_path.join("test.txt");
    let test_file_clone = test_file.clone();
    blocking(move || {
        fs::File::create(&test_file_clone)?;
        Ok(())
    })
    .await?;

    // Verify file exists
    let exists = blocking(move || Ok(test_file.exists())).await?;
    assert!(exists);

    drop(session);
    do_unmount(mount_path).await?;
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
    let backend =
        Arc::new(TarboxBackend::new(Arc::new(pool.pool().clone()), tenant.tenant_id).await?);

    let session = mount(backend, &mount_path, options)?;

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Write through FUSE
    let test_file = mount_path.join("data.txt");
    let test_data = b"Hello, FUSE!".to_vec();
    let test_file_clone = test_file.clone();
    let test_data_clone = test_data.clone();
    blocking(move || {
        let mut file = fs::File::create(&test_file_clone)?;
        file.write_all(&test_data_clone)?;
        Ok(())
    })
    .await?;

    // Read through FUSE
    let buffer = blocking(move || {
        let mut file = fs::File::open(&test_file)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        Ok(buffer)
    })
    .await?;

    assert_eq!(buffer, test_data);

    drop(session);
    do_unmount(mount_path).await?;
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
    let backend =
        Arc::new(TarboxBackend::new(Arc::new(pool.pool().clone()), tenant.tenant_id).await?);

    let session = mount(backend, &mount_path, options)?;

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Create directory through FUSE
    let test_dir = mount_path.join("testdir");
    let test_dir_clone = test_dir.clone();
    blocking(move || {
        fs::create_dir(&test_dir_clone)?;
        fs::File::create(test_dir_clone.join("file1.txt"))?;
        fs::File::create(test_dir_clone.join("file2.txt"))?;
        Ok(())
    })
    .await?;

    // Read directory through FUSE
    let entries: Vec<String> = blocking(move || {
        let entries: Vec<String> = fs::read_dir(&test_dir)?
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        Ok(entries)
    })
    .await?;

    assert_eq!(entries.len(), 2);
    assert!(entries.contains(&"file1.txt".to_string()));
    assert!(entries.contains(&"file2.txt".to_string()));

    drop(session);
    do_unmount(mount_path).await?;
    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
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
    let backend =
        Arc::new(TarboxBackend::new(Arc::new(pool.pool().clone()), tenant.tenant_id).await?);

    let session = mount(backend, &mount_path, options)?;

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Create and delete file
    let test_file = mount_path.join("delete_me.txt");
    let test_file_clone = test_file.clone();
    let exists_before = blocking(move || {
        fs::File::create(&test_file_clone)?;
        Ok(test_file_clone.exists())
    })
    .await?;
    assert!(exists_before);

    let exists_after = blocking(move || {
        fs::remove_file(&test_file)?;
        Ok(test_file.exists())
    })
    .await?;
    assert!(!exists_after);

    drop(session);
    do_unmount(mount_path).await?;
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
    let backend =
        Arc::new(TarboxBackend::new(Arc::new(pool.pool().clone()), tenant.tenant_id).await?);

    let session = mount(backend, &mount_path, options)?;

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Create file
    let test_file = mount_path.join("meta.txt");
    let test_file_clone = test_file.clone();
    blocking(move || {
        let mut file = fs::File::create(&test_file_clone)?;
        file.write_all(b"test")?;
        Ok(())
    })
    .await?;

    // Get metadata
    let (is_file, len) = blocking(move || {
        let metadata = fs::metadata(&test_file)?;
        Ok((metadata.is_file(), metadata.len()))
    })
    .await?;

    assert!(is_file);
    assert_eq!(len, 4);

    drop(session);
    do_unmount(mount_path).await?;
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
    let backend =
        Arc::new(TarboxBackend::new(Arc::new(pool.pool().clone()), tenant.tenant_id).await?);

    let session = mount(backend, &mount_path, options)?;

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Create file and change permissions
    let test_file = mount_path.join("chmod.txt");
    let mode = blocking(move || {
        fs::File::create(&test_file)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o755);
            fs::set_permissions(&test_file, perms)?;

            let metadata = fs::metadata(&test_file)?;
            Ok(metadata.permissions().mode() & 0o777)
        }

        #[cfg(not(unix))]
        Ok(0o755u32)
    })
    .await?;

    assert_eq!(mode, 0o755);

    drop(session);
    do_unmount(mount_path).await?;
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
    let backend =
        Arc::new(TarboxBackend::new(Arc::new(pool.pool().clone()), tenant.tenant_id).await?);

    let session = mount(backend, &mount_path, options)?;

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Create nested directories
    let mount_path_clone = mount_path.clone();
    let (a_is_dir, ab_is_dir, abc_is_dir) = blocking(move || {
        let nested_path = mount_path_clone.join("a").join("b").join("c");
        fs::create_dir_all(&nested_path)?;

        Ok((
            mount_path_clone.join("a").is_dir(),
            mount_path_clone.join("a/b").is_dir(),
            nested_path.is_dir(),
        ))
    })
    .await?;

    assert!(a_is_dir);
    assert!(ab_is_dir);
    assert!(abc_is_dir);

    drop(session);
    do_unmount(mount_path).await?;
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
    let backend =
        Arc::new(TarboxBackend::new(Arc::new(pool.pool().clone()), tenant.tenant_id).await?);

    let session = mount(backend, &mount_path, options)?;

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Write large file (100KB)
    let test_file = mount_path.join("large.bin");
    let large_data: Vec<u8> = (0..102400).map(|i| (i % 256) as u8).collect();
    let large_data_clone = large_data.clone();
    let test_file_clone = test_file.clone();

    blocking(move || {
        let mut file = fs::File::create(&test_file_clone)?;
        file.write_all(&large_data_clone)?;
        Ok(())
    })
    .await?;

    // Read back
    let buffer = blocking(move || {
        let mut file = fs::File::open(&test_file)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        Ok(buffer)
    })
    .await?;

    assert_eq!(buffer.len(), large_data.len());
    assert_eq!(buffer, large_data);

    drop(session);
    do_unmount(mount_path).await?;
    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
#[ignore] // Requires FUSE permissions; rename not yet implemented (ENOSYS)
async fn test_fuse_rename() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_fuse_rename_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let mountpoint = TempDir::new()?;
    let mount_path = mountpoint.path().to_path_buf();

    let options = MountOptions::default();
    let backend =
        Arc::new(TarboxBackend::new(Arc::new(pool.pool().clone()), tenant.tenant_id).await?);

    let session = mount(backend, &mount_path, options)?;

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Create file
    let old_path = mount_path.join("old.txt");
    let new_path = mount_path.join("new.txt");
    let old_path_clone = old_path.clone();

    blocking(move || {
        let mut file = fs::File::create(&old_path_clone)?;
        file.write_all(b"test data")?;
        Ok(())
    })
    .await?;

    // Rename through FUSE
    let old_path_clone = old_path.clone();
    let new_path_clone = new_path.clone();
    let (old_exists, new_exists) = blocking(move || {
        fs::rename(&old_path_clone, &new_path_clone)?;
        Ok((old_path_clone.exists(), new_path_clone.exists()))
    })
    .await?;

    assert!(!old_exists);
    assert!(new_exists);

    // Verify content preserved
    let buffer = blocking(move || {
        let mut file = fs::File::open(&new_path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        Ok(buffer)
    })
    .await?;
    assert_eq!(buffer, b"test data");

    drop(session);
    do_unmount(mount_path).await?;
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
    let backend =
        Arc::new(TarboxBackend::new(Arc::new(pool.pool().clone()), tenant.tenant_id).await?);

    let session = mount(backend, &mount_path, options)?;

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Create and remove empty directory
    let test_dir = mount_path.join("emptydir");
    let test_dir_clone = test_dir.clone();
    let exists_before = blocking(move || {
        fs::create_dir(&test_dir_clone)?;
        Ok(test_dir_clone.exists())
    })
    .await?;
    assert!(exists_before);

    let exists_after = blocking(move || {
        fs::remove_dir(&test_dir)?;
        Ok(test_dir.exists())
    })
    .await?;
    assert!(!exists_after);

    drop(session);
    do_unmount(mount_path).await?;
    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

use anyhow::Result;
use std::sync::Arc;
use tarbox::config::DatabaseConfig;
use tarbox::fuse::backend::TarboxBackend;
use tarbox::fuse::interface::{FileType, FilesystemInterface};
use tarbox::storage::{CreateTenantInput, DatabasePool, TenantOperations, TenantRepository};

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

#[tokio::test]
async fn test_backend_lookup_root() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_backend_root_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let backend = TarboxBackend::new(Arc::new(pool.pool().clone()), tenant.tenant_id).await?;

    let attr = backend.get_attr("/").await?;
    assert_eq!(attr.kind, FileType::Directory);
    assert_eq!(attr.inode, tenant.root_inode_id as u64);

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_backend_create_and_lookup_file() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_backend_file_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let backend = TarboxBackend::new(Arc::new(pool.pool().clone()), tenant.tenant_id).await?;

    let file_attr = backend.create_file("/test.txt", 0o644).await?;
    assert_eq!(file_attr.kind, FileType::RegularFile);
    assert_eq!(file_attr.mode, 0o644);

    let lookup_attr = backend.get_attr("/test.txt").await?;
    assert_eq!(lookup_attr.inode, file_attr.inode);
    assert_eq!(lookup_attr.kind, FileType::RegularFile);

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_backend_write_and_read_file() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_backend_io_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let backend = TarboxBackend::new(Arc::new(pool.pool().clone()), tenant.tenant_id).await?;

    backend.create_file("/data.txt", 0o644).await?;

    let test_data = b"Hello from FUSE backend!";
    let written = backend.write_file("/data.txt", 0, test_data).await?;
    assert_eq!(written, test_data.len() as u32);

    let read_data = backend.read_file("/data.txt", 0, test_data.len() as u32).await?;
    assert_eq!(read_data, test_data);

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_backend_read_with_offset() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_backend_offset_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let backend = TarboxBackend::new(Arc::new(pool.pool().clone()), tenant.tenant_id).await?;

    backend.create_file("/offset_test.txt", 0o644).await?;

    let test_data = b"0123456789ABCDEF";
    backend.write_file("/offset_test.txt", 0, test_data).await?;

    // Read from offset 5, length 4 -> should get "5678"
    let partial = backend.read_file("/offset_test.txt", 5, 4).await?;
    assert_eq!(partial, b"5678");

    // Read from offset 10, length 100 -> should get "ABCDEF" (clipped to EOF)
    let to_end = backend.read_file("/offset_test.txt", 10, 100).await?;
    assert_eq!(to_end, b"ABCDEF");

    // Read past EOF -> should get empty
    let past_eof = backend.read_file("/offset_test.txt", 100, 10).await?;
    assert_eq!(past_eof.len(), 0);

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_backend_truncate() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_backend_truncate_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let backend = TarboxBackend::new(Arc::new(pool.pool().clone()), tenant.tenant_id).await?;

    backend.create_file("/truncate_test.txt", 0o644).await?;
    backend.write_file("/truncate_test.txt", 0, b"Some data").await?;

    backend.truncate("/truncate_test.txt", 0).await?;

    let data = backend.read_file("/truncate_test.txt", 0, 1000).await?;
    assert_eq!(data.len(), 0);

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_backend_delete_file() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_backend_delete_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let backend = TarboxBackend::new(Arc::new(pool.pool().clone()), tenant.tenant_id).await?;

    backend.create_file("/delete_me.txt", 0o644).await?;
    backend.write_file("/delete_me.txt", 0, b"data").await?;

    backend.delete_file("/delete_me.txt").await?;

    let result = backend.get_attr("/delete_me.txt").await;
    assert!(result.is_err());

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_backend_create_and_list_directory() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_backend_dir_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let backend = TarboxBackend::new(Arc::new(pool.pool().clone()), tenant.tenant_id).await?;

    let dir_attr = backend.create_dir("/testdir", 0o755).await?;
    assert_eq!(dir_attr.kind, FileType::Directory);
    assert_eq!(dir_attr.mode, 0o755);

    let lookup_attr = backend.get_attr("/testdir").await?;
    assert_eq!(lookup_attr.inode, dir_attr.inode);
    assert_eq!(lookup_attr.kind, FileType::Directory);

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_backend_read_directory() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_backend_readdir_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let backend = TarboxBackend::new(Arc::new(pool.pool().clone()), tenant.tenant_id).await?;

    backend.create_dir("/parent", 0o755).await?;
    backend.create_file("/parent/file1.txt", 0o644).await?;
    backend.create_file("/parent/file2.txt", 0o644).await?;
    backend.create_dir("/parent/subdir", 0o755).await?;

    let entries = backend.read_dir("/parent").await?;
    assert_eq!(entries.len(), 3);

    let names: Vec<String> = entries.iter().map(|e| e.name.clone()).collect();
    assert!(names.contains(&"file1.txt".to_string()));
    assert!(names.contains(&"file2.txt".to_string()));
    assert!(names.contains(&"subdir".to_string()));

    // Check file types
    for entry in entries {
        match entry.name.as_str() {
            "file1.txt" | "file2.txt" => assert_eq!(entry.kind, FileType::RegularFile),
            "subdir" => assert_eq!(entry.kind, FileType::Directory),
            _ => panic!("Unexpected entry: {}", entry.name),
        }
    }

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_backend_remove_directory() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_backend_rmdir_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let backend = TarboxBackend::new(Arc::new(pool.pool().clone()), tenant.tenant_id).await?;

    backend.create_dir("/emptydir", 0o755).await?;
    backend.remove_dir("/emptydir").await?;

    let result = backend.get_attr("/emptydir").await;
    assert!(result.is_err());

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_backend_setattr_mode() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_backend_setattr_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let backend = TarboxBackend::new(Arc::new(pool.pool().clone()), tenant.tenant_id).await?;

    backend.create_file("/chmod.txt", 0o644).await?;

    use tarbox::fuse::interface::SetAttr;
    let set_attr =
        SetAttr { mode: Some(0o755), uid: None, gid: None, size: None, atime: None, mtime: None };

    let updated = backend.set_attr("/chmod.txt", set_attr).await?;
    assert_eq!(updated.mode, 0o755);

    let lookup = backend.get_attr("/chmod.txt").await?;
    assert_eq!(lookup.mode, 0o755);

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_backend_setattr_size() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_backend_setsize_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let backend = TarboxBackend::new(Arc::new(pool.pool().clone()), tenant.tenant_id).await?;

    backend.create_file("/truncate_via_setattr.txt", 0o644).await?;
    backend.write_file("/truncate_via_setattr.txt", 0, b"Long content").await?;

    use tarbox::fuse::interface::SetAttr;
    let set_attr =
        SetAttr { mode: None, uid: None, gid: None, size: Some(0), atime: None, mtime: None };

    backend.set_attr("/truncate_via_setattr.txt", set_attr).await?;

    let data = backend.read_file("/truncate_via_setattr.txt", 0, 1000).await?;
    assert_eq!(data.len(), 0);

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_backend_setattr_uid_gid() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_backend_chown_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let backend = TarboxBackend::new(Arc::new(pool.pool().clone()), tenant.tenant_id).await?;

    backend.create_file("/chown.txt", 0o644).await?;

    use tarbox::fuse::interface::SetAttr;
    let set_attr = SetAttr {
        mode: None,
        uid: Some(1001),
        gid: Some(1002),
        size: None,
        atime: None,
        mtime: None,
    };

    let updated = backend.set_attr("/chown.txt", set_attr).await?;
    assert_eq!(updated.uid, 1001);
    assert_eq!(updated.gid, 1002);

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_backend_large_file() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_backend_large_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let backend = TarboxBackend::new(Arc::new(pool.pool().clone()), tenant.tenant_id).await?;

    backend.create_file("/large.bin", 0o644).await?;

    // 20KB file spanning multiple blocks
    let large_data: Vec<u8> = (0..20480).map(|i| (i % 256) as u8).collect();
    backend.write_file("/large.bin", 0, &large_data).await?;

    let read_data = backend.read_file("/large.bin", 0, large_data.len() as u32).await?;
    assert_eq!(read_data.len(), large_data.len());
    assert_eq!(read_data, large_data);

    let attr = backend.get_attr("/large.bin").await?;
    assert_eq!(attr.size, large_data.len() as u64);

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

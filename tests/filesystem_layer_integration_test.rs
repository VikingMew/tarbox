//! Integration tests for FileSystem + Layer integration
//!
//! Tests that verify FileSystem correctly uses LayerManager and CowHandler

use anyhow::Result;
use tarbox::config::DatabaseConfig;
use tarbox::fs::operations::FileSystem;
use tarbox::storage::{
    CreateTenantInput, DatabasePool, LayerOperations, LayerRepository, TenantOperations,
    TenantRepository,
};
use uuid::Uuid;

async fn setup_test_db() -> Result<DatabasePool> {
    let config = DatabaseConfig {
        url: std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/tarbox".into()),
        max_connections: 5,
        min_connections: 1,
    };
    DatabasePool::new(&config).await
}

async fn cleanup_tenant(pool: &DatabasePool, tenant_name: &str) -> Result<()> {
    let tenant_ops = TenantOperations::new(pool.pool());
    if let Some(tenant) = tenant_ops.get_by_name(tenant_name).await? {
        tenant_ops.delete(tenant.tenant_id).await?;
    }
    Ok(())
}

#[tokio::test]
async fn test_filesystem_auto_creates_base_layer() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());
    let layer_ops = LayerOperations::new(pool.pool());

    let tenant_name = format!("test_base_layer_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    // Before creating FileSystem, no layers should exist
    let layers_before = layer_ops.list(tenant.tenant_id).await?;
    assert_eq!(layers_before.len(), 0);

    // Create FileSystem - should auto-create base layer
    let _fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;

    // After creating FileSystem, base layer should exist
    let layers_after = layer_ops.list(tenant.tenant_id).await?;
    assert_eq!(layers_after.len(), 1);
    assert_eq!(layers_after[0].layer_name, "base");
    assert!(layers_after[0].parent_layer_id.is_none());

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_text_file_stored_in_text_blocks() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_text_storage_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;

    // Create and write a text file
    fs.create_file("/test.txt").await?;
    fs.write_file("/test.txt", b"hello\nworld\n").await?;

    // Verify it's stored in text_blocks
    let text_metadata_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM text_file_metadata WHERE tenant_id = $1")
            .bind(tenant.tenant_id)
            .fetch_one(pool.pool())
            .await?;

    assert_eq!(text_metadata_count, 1, "Text file should be in text_file_metadata");

    // Verify NOT in data_blocks
    let data_block_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM data_blocks WHERE tenant_id = $1")
            .bind(tenant.tenant_id)
            .fetch_one(pool.pool())
            .await?;

    assert_eq!(data_block_count, 0, "Text file should NOT be in data_blocks");

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_binary_file_stored_in_data_blocks() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_binary_storage_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;

    // Create and write a binary file (contains null byte)
    fs.create_file("/test.bin").await?;
    fs.write_file("/test.bin", b"binary\x00data").await?;

    // Verify it's stored in data_blocks
    let data_block_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM data_blocks WHERE tenant_id = $1")
            .bind(tenant.tenant_id)
            .fetch_one(pool.pool())
            .await?;

    assert!(data_block_count > 0, "Binary file should be in data_blocks");

    // Verify NOT in text_blocks
    let text_metadata_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM text_file_metadata WHERE tenant_id = $1")
            .bind(tenant.tenant_id)
            .fetch_one(pool.pool())
            .await?;

    assert_eq!(text_metadata_count, 0, "Binary file should NOT be in text_file_metadata");

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_new_file_records_layer_entry_add() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());
    let layer_ops = LayerOperations::new(pool.pool());

    let tenant_name = format!("test_layer_entry_add_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;

    // Get the base layer ID
    let layers = layer_ops.list(tenant.tenant_id).await?;
    let base_layer = &layers[0];

    let unique_file = format!("/new_{}.txt", Uuid::new_v4());
    fs.create_file(&unique_file).await?;
    fs.write_file(&unique_file, b"content").await?;

    // Verify layer_entry exists with Add change_type
    let entries = layer_ops.list_entries(tenant.tenant_id, base_layer.layer_id).await?;
    assert!(!entries.is_empty(), "Should have at least one entry");

    // Find our entry
    let our_entry = entries.iter().find(|e| e.path == unique_file).expect("Should find our entry");
    assert_eq!(our_entry.change_type, tarbox::storage::ChangeType::Add);

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_modify_file_records_layer_entry_modify() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());
    let layer_ops = LayerOperations::new(pool.pool());

    let tenant_name = format!("test_layer_entry_modify_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;

    // Get the base layer ID
    let layers = layer_ops.list(tenant.tenant_id).await?;
    let base_layer = &layers[0];

    fs.create_file("/modify.txt").await?;
    fs.write_file("/modify.txt", b"first").await?;

    // Modify the file
    fs.write_file("/modify.txt", b"second version").await?;

    // Verify layer_entry exists with Modify change_type
    let entries = layer_ops.list_entries(tenant.tenant_id, base_layer.layer_id).await?;
    assert_eq!(entries.len(), 1); // Should be 1 due to ON CONFLICT UPDATE
    assert_eq!(entries[0].path, "/modify.txt");
    assert_eq!(entries[0].change_type, tarbox::storage::ChangeType::Modify);

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_text_changes_recorded_in_layer_entry() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());
    let layer_ops = LayerOperations::new(pool.pool());

    let tenant_name = format!("test_text_changes_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;

    // Get the base layer ID
    let layers = layer_ops.list(tenant.tenant_id).await?;
    let base_layer = &layers[0];

    fs.create_file("/text.txt").await?;
    fs.write_file("/text.txt", b"line1\nline2\nline3\n").await?;

    // Verify layer_entry has text_changes
    let entries = layer_ops.list_entries(tenant.tenant_id, base_layer.layer_id).await?;
    assert_eq!(entries.len(), 1);

    let text_changes = entries[0].text_changes.as_ref().expect("Should have text_changes");
    assert_eq!(text_changes["lines_added"], 3);
    assert_eq!(text_changes["lines_deleted"], 0);
    assert_eq!(text_changes["total_lines"], 3);

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_read_text_file_from_text_blocks() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_read_text_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;

    // Use unique content to avoid hash collision with other tests
    let unique_id = Uuid::new_v4();
    let original_content = format!("unique_read_{}\nline2_{}\n", unique_id, unique_id);

    fs.create_file("/read.txt").await?;
    fs.write_file("/read.txt", original_content.as_bytes()).await?;

    // Read it back
    let read_content = fs.read_file("/read.txt").await?;
    assert_eq!(read_content, original_content.as_bytes());

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_read_binary_file_from_data_blocks() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_read_binary_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;

    let original_content = b"binary\x00data\xFF\xFE";

    fs.create_file("/read.bin").await?;
    fs.write_file("/read.bin", original_content).await?;

    // Read it back
    let read_content = fs.read_file("/read.bin").await?;
    assert_eq!(read_content, original_content);

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_empty_file_is_text() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_empty_file_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;

    fs.create_file("/empty.txt").await?;
    fs.write_file("/empty.txt", b"").await?;

    // Empty files should be stored as text
    let text_metadata_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM text_file_metadata WHERE tenant_id = $1")
            .bind(tenant.tenant_id)
            .fetch_one(pool.pool())
            .await?;

    assert_eq!(text_metadata_count, 1, "Empty file should be stored as text");

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_large_text_file() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_large_text_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;

    // Create 1000 lines of text (should still be text, under 10MB limit)
    let mut lines = Vec::new();
    for i in 0..1000 {
        lines.push(format!("This is line number {}\n", i));
    }
    let content = lines.join("");

    fs.create_file("/large.txt").await?;
    fs.write_file("/large.txt", content.as_bytes()).await?;

    // Should be stored as text
    let text_metadata_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM text_file_metadata WHERE tenant_id = $1")
            .bind(tenant.tenant_id)
            .fetch_one(pool.pool())
            .await?;

    assert_eq!(text_metadata_count, 1);

    // Verify we can read it back
    let read_content = fs.read_file("/large.txt").await?;
    assert_eq!(read_content, content.as_bytes());

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

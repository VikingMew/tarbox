//! Integration tests for cross-layer file type transitions
//!
//! Critical edge case: same file can be text in one layer, binary in another

use anyhow::Result;
use tarbox::config::DatabaseConfig;
use tarbox::fs::operations::FileSystem;
use tarbox::layer::LayerManager;
use tarbox::storage::{CreateTenantInput, DatabasePool, TenantOperations, TenantRepository};
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
async fn test_text_to_binary_transition() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_text_to_binary_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    let layer_mgr = LayerManager::new(pool.pool(), tenant.tenant_id);

    // Layer 1: Write as text
    fs.create_file("/transition.dat").await?;
    fs.write_file("/transition.dat", b"text content\n").await?;

    // Verify stored as text
    let text_count_l1: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM text_file_metadata WHERE tenant_id = $1")
            .bind(tenant.tenant_id)
            .fetch_one(pool.pool())
            .await?;
    assert_eq!(text_count_l1, 1);

    // Create checkpoint (Layer 2)
    layer_mgr.create_checkpoint("layer2", Some("After text write")).await?;

    // Layer 2: Overwrite with binary
    let fs2 = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    fs2.write_file("/transition.dat", b"binary\x00data\xFF").await?;

    // Verify now stored as binary in layer 2
    let data_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM data_blocks WHERE tenant_id = $1")
            .bind(tenant.tenant_id)
            .fetch_one(pool.pool())
            .await?;
    assert!(data_count > 0, "Should have binary blocks in layer 2");

    // Both text and binary should coexist (different layers)
    let text_count_l2: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM text_file_metadata WHERE tenant_id = $1")
            .bind(tenant.tenant_id)
            .fetch_one(pool.pool())
            .await?;
    assert!(text_count_l2 >= 1, "Text metadata from layer 1 should still exist");

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_binary_to_text_transition() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_binary_to_text_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    let layer_mgr = LayerManager::new(pool.pool(), tenant.tenant_id);

    // Layer 1: Write as binary
    fs.create_file("/transition2.dat").await?;
    fs.write_file("/transition2.dat", b"binary\x00content").await?;

    // Create checkpoint
    layer_mgr.create_checkpoint("layer2", Some("After binary write")).await?;

    // Layer 2: Overwrite with text
    let fs2 = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    fs2.write_file("/transition2.dat", b"now text content\n").await?;

    // Verify now stored as text in layer 2
    let text_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM text_file_metadata WHERE tenant_id = $1")
            .bind(tenant.tenant_id)
            .fetch_one(pool.pool())
            .await?;
    assert_eq!(text_count, 1, "Should have text metadata in layer 2");

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_multiple_type_switches() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_multiple_switches_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let layer_mgr = LayerManager::new(pool.pool(), tenant.tenant_id);

    // Layer 1: Text
    let fs1 = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    fs1.create_file("/multi.dat").await?;
    fs1.write_file("/multi.dat", b"text1\n").await?;
    layer_mgr.create_checkpoint("l2", None).await?;

    // Layer 2: Binary
    let fs2 = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    fs2.write_file("/multi.dat", b"bin\x00").await?;
    layer_mgr.create_checkpoint("l3", None).await?;

    // Layer 3: Text again
    let fs3 = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    fs3.write_file("/multi.dat", b"text2\n").await?;
    layer_mgr.create_checkpoint("l4", None).await?;

    // Layer 4: Binary again
    let fs4 = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    fs4.write_file("/multi.dat", b"bin2\x00\xFF").await?;

    // All layers should have independent storage
    let layers = layer_mgr.list_layers().await?;
    assert_eq!(layers.len(), 4);

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_switch_layer_read_correct_type() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_switch_read_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let layer_mgr = LayerManager::new(pool.pool(), tenant.tenant_id);

    // Layer 1: Text
    let fs1 = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    let layer1_id = layer_mgr.get_current_layer().await?.layer_id;

    fs1.create_file("/switch.dat").await?;
    fs1.write_file("/switch.dat", b"text in layer 1\n").await?;

    layer_mgr.create_checkpoint("l2", None).await?;

    // Layer 2: Binary
    let fs2 = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    fs2.write_file("/switch.dat", b"binary\x00layer2").await?;

    // Read from layer 2 (binary)
    let content_l2 = fs2.read_file("/switch.dat").await?;
    assert_eq!(content_l2, b"binary\x00layer2");

    // Switch back to layer 1
    layer_mgr.switch_to_layer(layer1_id).await?;

    // Read from layer 1 (text)
    let fs1_again = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    let content_l1 = fs1_again.read_file("/switch.dat").await?;
    assert_eq!(content_l1, b"text in layer 1\n");

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_layer_entry_records_type_change() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_entry_type_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    let layer_mgr = LayerManager::new(pool.pool(), tenant.tenant_id);

    let base_layer = layer_mgr.get_current_layer().await?;

    // Create text file
    fs.create_file("/entry.dat").await?;
    fs.write_file("/entry.dat", b"text\n").await?;

    // Check layer entry has text_changes
    let entries_l1 = layer_mgr.get_layer_entries(base_layer.layer_id).await?;
    assert!(entries_l1[0].text_changes.is_some(), "Text file should have text_changes");

    // Create new layer
    layer_mgr.create_checkpoint("l2", None).await?;

    // Overwrite with binary
    let fs2 = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    fs2.write_file("/entry.dat", b"bin\x00").await?;

    // Check layer entry has no text_changes
    let layer2 = layer_mgr.get_current_layer().await?;
    let entries_l2 = layer_mgr.get_layer_entries(layer2.layer_id).await?;
    assert!(entries_l2[0].text_changes.is_none(), "Binary file should not have text_changes");

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_empty_to_text_to_binary() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_empty_trans_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    let layer_mgr = LayerManager::new(pool.pool(), tenant.tenant_id);

    // Empty file (text)
    fs.create_file("/empty.dat").await?;
    fs.write_file("/empty.dat", b"").await?;
    layer_mgr.create_checkpoint("l2", None).await?;

    // Text content
    let fs2 = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    fs2.write_file("/empty.dat", b"now has text\n").await?;
    layer_mgr.create_checkpoint("l3", None).await?;

    // Binary content
    let fs3 = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    fs3.write_file("/empty.dat", b"binary\x00").await?;

    // Verify final state is binary
    let data_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM data_blocks WHERE tenant_id = $1")
            .bind(tenant.tenant_id)
            .fetch_one(pool.pool())
            .await?;
    assert!(data_count > 0);

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_large_file_type_transition() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_large_trans_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    let layer_mgr = LayerManager::new(pool.pool(), tenant.tenant_id);

    // Large text file (1000 lines)
    let mut text_lines = Vec::new();
    for i in 0..1000 {
        text_lines.push(format!("Line {}\n", i));
    }
    let text_content = text_lines.join("");

    fs.create_file("/large.dat").await?;
    fs.write_file("/large.dat", text_content.as_bytes()).await?;
    layer_mgr.create_checkpoint("l2", None).await?;

    // Overwrite with large binary (5KB)
    let binary_content: Vec<u8> = (0..5120).map(|i| (i % 256) as u8).collect();
    let fs2 = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    fs2.write_file("/large.dat", &binary_content).await?;

    // Verify transition
    let data_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM data_blocks WHERE tenant_id = $1")
            .bind(tenant.tenant_id)
            .fetch_one(pool.pool())
            .await?;
    assert_eq!(data_count, 2, "5KB binary should have 2 blocks");

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

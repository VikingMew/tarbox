//! UnionView integration tests - Test layer union semantics

use anyhow::Result;
use tarbox::config::DatabaseConfig;
use tarbox::fs::FileSystem;
use tarbox::layer::{LayerManager, UnionView};
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
    if let Ok(Some(tenant)) = tenant_ops.get_by_name(tenant_name).await {
        tenant_ops.delete(tenant.tenant_id).await?;
    }
    Ok(())
}

#[tokio::test]
async fn test_union_view_from_current() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("union_test_current_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let _fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;

    // Create union view from current layer
    let union = UnionView::from_current(pool.pool(), tenant.tenant_id)
        .await?
        .expect("Should have current layer");

    // Should have base layer
    assert!(union.current_layer_id().is_some());
    assert_eq!(union.layer_chain().len(), 1); // Just base layer

    cleanup_tenant(&pool, &tenant_name).await?;
    Ok(())
}

#[tokio::test]
async fn test_union_view_lookup_file_exists() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("union_test_lookup_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;

    // Create a file
    let unique_file = format!("/union_test_{}.txt", Uuid::new_v4());
    fs.create_file(&unique_file).await?;
    fs.write_file(&unique_file, b"content").await?;

    let union = UnionView::from_current(pool.pool(), tenant.tenant_id)
        .await?
        .expect("Should have current layer");

    // Lookup the file
    let state = union.lookup_file(&unique_file).await?;

    assert!(state.exists(), "File should exist in union view");
    assert!(state.inode_id().is_some());
    assert!(state.layer_id().is_some());

    cleanup_tenant(&pool, &tenant_name).await?;
    Ok(())
}

#[tokio::test]
async fn test_union_view_lookup_nonexistent_file() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("union_test_notfound_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let _fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;

    let union = UnionView::from_current(pool.pool(), tenant.tenant_id)
        .await?
        .expect("Should have current layer");

    // Lookup non-existent file
    let state = union.lookup_file("/does_not_exist.txt").await?;

    assert!(!state.exists(), "Non-existent file should not exist");
    assert!(state.inode_id().is_none());

    cleanup_tenant(&pool, &tenant_name).await?;
    Ok(())
}

#[tokio::test]
async fn test_union_view_file_deleted_in_later_layer() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("union_test_deleted_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    let layer_mgr = LayerManager::new(pool.pool(), tenant.tenant_id);

    // Create file in base layer
    let unique_file = format!("/deleted_test_{}.txt", Uuid::new_v4());
    fs.create_file(&unique_file).await?;
    fs.write_file(&unique_file, b"original").await?;

    // Create checkpoint
    layer_mgr.create_checkpoint("layer_with_delete", None).await?;

    // Delete file in new layer
    fs.delete_file(&unique_file).await?;

    let union = UnionView::from_current(pool.pool(), tenant.tenant_id)
        .await?
        .expect("Should have current layer");

    // File should show as deleted in union view
    let state = union.lookup_file(&unique_file).await?;

    // In union view, deleted files should not exist
    assert!(!state.exists(), "Deleted file should not exist in union view");

    cleanup_tenant(&pool, &tenant_name).await?;
    Ok(())
}

#[tokio::test]
async fn test_union_view_file_modified_across_layers() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("union_test_modified_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    let layer_mgr = LayerManager::new(pool.pool(), tenant.tenant_id);

    // Create file in base layer
    let unique_file = format!("/modified_test_{}.txt", Uuid::new_v4());
    fs.create_file(&unique_file).await?;
    fs.write_file(&unique_file, b"version1").await?;

    let base_layers = layer_mgr.list_layers().await?;
    let base_layer_id = base_layers[0].layer_id;

    // Create checkpoint
    layer_mgr.create_checkpoint("layer2", None).await?;

    // Modify file in new layer
    fs.write_file(&unique_file, b"version2").await?;

    let union = UnionView::from_current(pool.pool(), tenant.tenant_id)
        .await?
        .expect("Should have current layer");

    // Lookup should return the most recent version (from current layer)
    let state = union.lookup_file(&unique_file).await?;

    assert!(state.exists());
    let layer_id = state.layer_id().expect("Should have layer ID");
    // The file should be found in a layer that's NOT the base layer
    assert_ne!(layer_id, base_layer_id, "File should be from newer layer, not base");

    cleanup_tenant(&pool, &tenant_name).await?;
    Ok(())
}

#[tokio::test]
async fn test_union_view_list_directory() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("union_test_listdir_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;

    // Create multiple files
    let file1 = format!("/dir_file1_{}.txt", Uuid::new_v4());
    let file2 = format!("/dir_file2_{}.txt", Uuid::new_v4());

    fs.create_file(&file1).await?;
    fs.write_file(&file1, b"content1").await?;

    fs.create_file(&file2).await?;
    fs.write_file(&file2, b"content2").await?;

    let union = UnionView::from_current(pool.pool(), tenant.tenant_id)
        .await?
        .expect("Should have current layer");

    // List root directory
    let entries = union.list_directory("/").await?;

    // Should have at least our test files
    let entry_names: Vec<String> = entries.iter().map(|e| e.name.clone()).collect();
    assert!(entry_names.iter().any(|n| n.contains("dir_file1_")), "Should find file1");
    assert!(entry_names.iter().any(|n| n.contains("dir_file2_")), "Should find file2");

    cleanup_tenant(&pool, &tenant_name).await?;
    Ok(())
}

#[tokio::test]
async fn test_union_view_layer_chain() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("union_test_chain_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let _fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    let layer_mgr = LayerManager::new(pool.pool(), tenant.tenant_id);

    // Create multiple checkpoints
    layer_mgr.create_checkpoint("layer1", None).await?;
    layer_mgr.create_checkpoint("layer2", None).await?;
    layer_mgr.create_checkpoint("layer3", None).await?;

    let union = UnionView::from_current(pool.pool(), tenant.tenant_id)
        .await?
        .expect("Should have current layer");

    // Should have 4 layers in chain (base + 3 checkpoints)
    assert_eq!(union.layer_chain().len(), 4, "Should have base + 3 checkpoints in chain");

    // Current layer should be first
    let current_id = union.current_layer_id().expect("Should have current layer");
    assert_eq!(union.layer_chain()[0].layer_id, current_id);

    cleanup_tenant(&pool, &tenant_name).await?;
    Ok(())
}

#[tokio::test]
async fn test_union_view_from_specific_layer() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("union_test_specific_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let _fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    let layer_mgr = LayerManager::new(pool.pool(), tenant.tenant_id);

    // Create checkpoints
    let layer1 = layer_mgr.create_checkpoint("layer1", None).await?;
    layer_mgr.create_checkpoint("layer2", None).await?;

    // Create union view from specific layer (layer1, not current)
    let union = UnionView::from_layer(pool.pool(), tenant.tenant_id, layer1.layer_id).await?;

    // Should have layer1 as current in this view
    assert_eq!(
        union.current_layer_id(),
        Some(layer1.layer_id),
        "Union view should use specified layer"
    );

    // Should have 2 layers in chain (base + layer1), not including layer2
    assert_eq!(union.layer_chain().len(), 2, "Should have only layers up to specified layer");

    cleanup_tenant(&pool, &tenant_name).await?;
    Ok(())
}

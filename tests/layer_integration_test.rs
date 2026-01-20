use anyhow::Result;
use tarbox::config::DatabaseConfig;
use tarbox::storage::{
    ChangeType, CreateLayerEntryInput, CreateLayerInput, CreateTenantInput, DatabasePool,
    LayerOperations, LayerRepository, TenantOperations, TenantRepository,
};
use uuid::Uuid;

async fn setup_test_db() -> Result<(DatabasePool, Uuid)> {
    let config = DatabaseConfig {
        url: std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/tarbox".into()),
        max_connections: 5,
        min_connections: 1,
    };

    let pool = DatabasePool::new(&config).await?;
    pool.run_migrations().await?;

    // Create test tenant with unique name to avoid conflicts when tests run in parallel
    let tenant_ops = TenantOperations::new(pool.pool());
    let unique_name = format!("test-tenant-{}", Uuid::new_v4());
    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: unique_name }).await?;

    Ok((pool, tenant.tenant_id))
}

#[tokio::test]
async fn test_layer_create() -> Result<()> {
    let (pool, tenant_id) = setup_test_db().await?;
    let layer_ops = LayerOperations::new(pool.pool());

    let input = CreateLayerInput {
        tenant_id,
        parent_layer_id: None,
        layer_name: "base".to_string(),
        description: Some("Base layer".to_string()),
        tags: None,
        created_by: "test_user".to_string(),
    };

    let layer = layer_ops.create(input).await?;

    assert_eq!(layer.tenant_id, tenant_id);
    assert_eq!(layer.layer_name, "base");
    assert_eq!(layer.parent_layer_id, None);
    assert_eq!(layer.file_count, 0);
    assert_eq!(layer.total_size, 0);
    assert_eq!(layer.created_by, "test_user");

    Ok(())
}

#[tokio::test]
async fn test_layer_get() -> Result<()> {
    let (pool, tenant_id) = setup_test_db().await?;
    let layer_ops = LayerOperations::new(pool.pool());

    // Create a layer
    let input = CreateLayerInput {
        tenant_id,
        parent_layer_id: None,
        layer_name: "test_layer".to_string(),
        description: None,
        tags: None,
        created_by: "test".to_string(),
    };

    let created = layer_ops.create(input).await?;

    // Get the layer
    let found = layer_ops.get(tenant_id, created.layer_id).await?;

    assert!(found.is_some());
    let layer = found.unwrap();
    assert_eq!(layer.layer_id, created.layer_id);
    assert_eq!(layer.layer_name, "test_layer");

    Ok(())
}

#[tokio::test]
async fn test_layer_list() -> Result<()> {
    let (pool, tenant_id) = setup_test_db().await?;
    let layer_ops = LayerOperations::new(pool.pool());

    // Create multiple layers
    for i in 1..=3 {
        let input = CreateLayerInput {
            tenant_id,
            parent_layer_id: None,
            layer_name: format!("layer_{}", i),
            description: None,
            tags: None,
            created_by: "test".to_string(),
        };
        layer_ops.create(input).await?;
    }

    // List all layers
    let layers = layer_ops.list(tenant_id).await?;

    assert!(layers.len() >= 3);
    assert!(layers.iter().any(|l| l.layer_name == "layer_1"));
    assert!(layers.iter().any(|l| l.layer_name == "layer_2"));
    assert!(layers.iter().any(|l| l.layer_name == "layer_3"));

    Ok(())
}

#[tokio::test]
async fn test_layer_chain() -> Result<()> {
    let (pool, tenant_id) = setup_test_db().await?;
    let layer_ops = LayerOperations::new(pool.pool());

    // Create base layer
    let base_input = CreateLayerInput {
        tenant_id,
        parent_layer_id: None,
        layer_name: "base".to_string(),
        description: Some("Base layer".to_string()),
        tags: None,
        created_by: "test".to_string(),
    };
    let base = layer_ops.create(base_input).await?;

    // Create child layer
    let child_input = CreateLayerInput {
        tenant_id,
        parent_layer_id: Some(base.layer_id),
        layer_name: "child".to_string(),
        description: Some("Child layer".to_string()),
        tags: None,
        created_by: "test".to_string(),
    };
    let child = layer_ops.create(child_input).await?;

    // Create grandchild layer
    let grandchild_input = CreateLayerInput {
        tenant_id,
        parent_layer_id: Some(child.layer_id),
        layer_name: "grandchild".to_string(),
        description: Some("Grandchild layer".to_string()),
        tags: None,
        created_by: "test".to_string(),
    };
    let grandchild = layer_ops.create(grandchild_input).await?;

    // Get layer chain from grandchild
    let chain = layer_ops.get_layer_chain(tenant_id, grandchild.layer_id).await?;

    assert_eq!(chain.len(), 3);
    assert_eq!(chain[0].layer_id, grandchild.layer_id);
    assert_eq!(chain[1].layer_id, child.layer_id);
    assert_eq!(chain[2].layer_id, base.layer_id);

    Ok(())
}

#[tokio::test]
async fn test_layer_delete() -> Result<()> {
    let (pool, tenant_id) = setup_test_db().await?;
    let layer_ops = LayerOperations::new(pool.pool());

    // Create a layer
    let input = CreateLayerInput {
        tenant_id,
        parent_layer_id: None,
        layer_name: "to_delete".to_string(),
        description: None,
        tags: None,
        created_by: "test".to_string(),
    };
    let layer = layer_ops.create(input).await?;

    // Delete the layer
    let deleted = layer_ops.delete(tenant_id, layer.layer_id).await?;
    assert!(deleted);

    // Verify it's gone
    let found = layer_ops.get(tenant_id, layer.layer_id).await?;
    assert!(found.is_none());

    Ok(())
}

#[tokio::test]
async fn test_layer_add_entry() -> Result<()> {
    let (pool, tenant_id) = setup_test_db().await?;
    let layer_ops = LayerOperations::new(pool.pool());

    // Create a layer
    let layer_input = CreateLayerInput {
        tenant_id,
        parent_layer_id: None,
        layer_name: "test".to_string(),
        description: None,
        tags: None,
        created_by: "test".to_string(),
    };
    let layer = layer_ops.create(layer_input).await?;

    // Add an entry
    let entry_input = CreateLayerEntryInput {
        layer_id: layer.layer_id,
        tenant_id,
        inode_id: 123,
        path: "/test/file.txt".to_string(),
        change_type: ChangeType::Add,
        size_delta: Some(1024),
        text_changes: None,
    };

    let entry = layer_ops.add_entry(entry_input).await?;

    assert_eq!(entry.layer_id, layer.layer_id);
    assert_eq!(entry.path, "/test/file.txt");
    assert_eq!(entry.change_type, ChangeType::Add);
    assert_eq!(entry.size_delta, Some(1024));

    Ok(())
}

#[tokio::test]
async fn test_layer_list_entries() -> Result<()> {
    let (pool, tenant_id) = setup_test_db().await?;
    let layer_ops = LayerOperations::new(pool.pool());

    // Create a layer
    let layer_input = CreateLayerInput {
        tenant_id,
        parent_layer_id: None,
        layer_name: "test".to_string(),
        description: None,
        tags: None,
        created_by: "test".to_string(),
    };
    let layer = layer_ops.create(layer_input).await?;

    // Add multiple entries
    for i in 1..=3 {
        let entry_input = CreateLayerEntryInput {
            layer_id: layer.layer_id,
            tenant_id,
            inode_id: i,
            path: format!("/file{}.txt", i),
            change_type: ChangeType::Add,
            size_delta: Some(100 * i),
            text_changes: None,
        };
        layer_ops.add_entry(entry_input).await?;
    }

    // List entries
    let entries = layer_ops.list_entries(tenant_id, layer.layer_id).await?;

    assert_eq!(entries.len(), 3);
    assert!(entries.iter().any(|e| e.path == "/file1.txt"));
    assert!(entries.iter().any(|e| e.path == "/file2.txt"));
    assert!(entries.iter().any(|e| e.path == "/file3.txt"));

    Ok(())
}

#[tokio::test]
async fn test_current_layer_tracking() -> Result<()> {
    let (pool, tenant_id) = setup_test_db().await?;
    let layer_ops = LayerOperations::new(pool.pool());

    // Initially no current layer
    let current = layer_ops.get_current_layer(tenant_id).await?;
    assert!(current.is_none());

    // Create a layer
    let layer_input = CreateLayerInput {
        tenant_id,
        parent_layer_id: None,
        layer_name: "current".to_string(),
        description: None,
        tags: None,
        created_by: "test".to_string(),
    };
    let layer = layer_ops.create(layer_input).await?;

    // Set as current layer
    layer_ops.set_current_layer(tenant_id, layer.layer_id).await?;

    // Verify current layer
    let current = layer_ops.get_current_layer(tenant_id).await?;
    assert_eq!(current, Some(layer.layer_id));

    // Create another layer and switch to it
    let layer2_input = CreateLayerInput {
        tenant_id,
        parent_layer_id: Some(layer.layer_id),
        layer_name: "new_current".to_string(),
        description: None,
        tags: None,
        created_by: "test".to_string(),
    };
    let layer2 = layer_ops.create(layer2_input).await?;

    layer_ops.set_current_layer(tenant_id, layer2.layer_id).await?;

    // Verify switched
    let current = layer_ops.get_current_layer(tenant_id).await?;
    assert_eq!(current, Some(layer2.layer_id));

    Ok(())
}

#[tokio::test]
async fn test_layer_entry_change_types() -> Result<()> {
    let (pool, tenant_id) = setup_test_db().await?;
    let layer_ops = LayerOperations::new(pool.pool());

    // Create a layer
    let layer_input = CreateLayerInput {
        tenant_id,
        parent_layer_id: None,
        layer_name: "test".to_string(),
        description: None,
        tags: None,
        created_by: "test".to_string(),
    };
    let layer = layer_ops.create(layer_input).await?;

    // Add entry with Add change type
    let add_input = CreateLayerEntryInput {
        layer_id: layer.layer_id,
        tenant_id,
        inode_id: 1,
        path: "/added.txt".to_string(),
        change_type: ChangeType::Add,
        size_delta: Some(100),
        text_changes: None,
    };
    layer_ops.add_entry(add_input).await?;

    // Add entry with Modify change type
    let modify_input = CreateLayerEntryInput {
        layer_id: layer.layer_id,
        tenant_id,
        inode_id: 2,
        path: "/modified.txt".to_string(),
        change_type: ChangeType::Modify,
        size_delta: Some(50),
        text_changes: None,
    };
    layer_ops.add_entry(modify_input).await?;

    // Add entry with Delete change type
    let delete_input = CreateLayerEntryInput {
        layer_id: layer.layer_id,
        tenant_id,
        inode_id: 3,
        path: "/deleted.txt".to_string(),
        change_type: ChangeType::Delete,
        size_delta: Some(-200),
        text_changes: None,
    };
    layer_ops.add_entry(delete_input).await?;

    // List and verify
    let entries = layer_ops.list_entries(tenant_id, layer.layer_id).await?;
    assert_eq!(entries.len(), 3);

    let add_entry = entries.iter().find(|e| e.path == "/added.txt").unwrap();
    assert_eq!(add_entry.change_type, ChangeType::Add);

    let modify_entry = entries.iter().find(|e| e.path == "/modified.txt").unwrap();
    assert_eq!(modify_entry.change_type, ChangeType::Modify);

    let delete_entry = entries.iter().find(|e| e.path == "/deleted.txt").unwrap();
    assert_eq!(delete_entry.change_type, ChangeType::Delete);

    Ok(())
}

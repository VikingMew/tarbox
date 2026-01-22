//! Hooks integration tests - Test /.tarbox/ virtual filesystem operations

use anyhow::Result;
use tarbox::config::DatabaseConfig;
use tarbox::fs::FileSystem;
use tarbox::layer::{HookResult, HooksHandler, LayerManager};
use tarbox::storage::{CreateTenantInput, DatabasePool, TenantOperations, TenantRepository};
use uuid::Uuid;

/// Setup test database pool
async fn setup_test_db() -> Result<DatabasePool> {
    let config = DatabaseConfig {
        url: std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/tarbox".into()),
        max_connections: 5,
        min_connections: 1,
    };
    DatabasePool::new(&config).await
}

/// Cleanup test tenant by name
async fn cleanup_tenant(pool: &DatabasePool, tenant_name: &str) -> Result<()> {
    let tenant_ops = TenantOperations::new(pool.pool());
    if let Ok(Some(tenant)) = tenant_ops.get_by_name(tenant_name).await {
        tenant_ops.delete(tenant.tenant_id).await?;
    }
    Ok(())
}

#[tokio::test]
async fn test_read_tarbox_layers_current() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("hooks_test_current_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let _fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    let hooks = HooksHandler::new(pool.pool(), tenant.tenant_id);

    // Read current layer info
    let result = hooks.handle_read("/.tarbox/layers/current").await;

    match result {
        HookResult::Content(content) => {
            // Content should contain layer info (ID, name, created_at)
            assert!(content.contains("Layer ID:") || content.contains("layer_id"));
            assert!(content.contains("base") || content.contains("Base"));
        }
        _ => panic!("Expected Content result, got {:?}", result),
    }

    cleanup_tenant(&pool, &tenant_name).await?;
    Ok(())
}

#[tokio::test]
async fn test_switch_layer_by_name() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("hooks_test_switch_name_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let _fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    let layer_mgr = LayerManager::new(pool.pool(), tenant.tenant_id);

    // Create a checkpoint
    let checkpoint_name = format!("named_layer_{}", Uuid::new_v4());
    layer_mgr.create_checkpoint(&checkpoint_name, Some("Test layer")).await?;

    let hooks = HooksHandler::new(pool.pool(), tenant.tenant_id);

    // Switch to the checkpoint by name (not UUID)
    let result = hooks.handle_write("/.tarbox/layers/switch", checkpoint_name.as_bytes()).await;

    match result {
        HookResult::WriteSuccess { .. } => {
            // Verify switch happened
            let current_result = hooks.handle_read("/.tarbox/layers/current").await;
            match current_result {
                HookResult::Content(content) => {
                    assert!(
                        content.contains(&checkpoint_name),
                        "Should have switched to named layer"
                    );
                }
                _ => panic!("Expected Content when reading current layer"),
            }
        }
        _ => panic!("Expected WriteSuccess result, got {:?}", result),
    }

    cleanup_tenant(&pool, &tenant_name).await?;
    Ok(())
}

#[tokio::test]
async fn test_read_stats_usage() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("hooks_test_stats_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;

    // Create some files to have stats
    fs.create_file("/stats_test.txt").await?;
    fs.write_file("/stats_test.txt", b"content").await?;

    let hooks = HooksHandler::new(pool.pool(), tenant.tenant_id);

    // Read stats
    let result = hooks.handle_read("/.tarbox/stats/usage").await;

    match result {
        HookResult::Content(content) => {
            // Should be JSON with stats
            assert!(content.contains("layer_count") || content.contains("total_size"));
            assert!(content.contains("tenant_id"));
        }
        _ => panic!("Expected Content result, got {:?}", result),
    }

    cleanup_tenant(&pool, &tenant_name).await?;
    Ok(())
}

#[tokio::test]
async fn test_write_invalid_utf8_fails() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("hooks_test_utf8_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let _fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    let hooks = HooksHandler::new(pool.pool(), tenant.tenant_id);

    // Try to write invalid UTF-8
    let invalid_utf8 = vec![0xFF, 0xFE, 0xFD];
    let result = hooks.handle_write("/.tarbox/layers/new", &invalid_utf8).await;

    match result {
        HookResult::Error(_) => {
            // Expected error for invalid UTF-8
        }
        _ => panic!("Expected Error result for invalid UTF-8, got {:?}", result),
    }

    cleanup_tenant(&pool, &tenant_name).await?;
    Ok(())
}

#[tokio::test]
async fn test_write_invalid_json_fails() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("hooks_test_json_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let _fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    let hooks = HooksHandler::new(pool.pool(), tenant.tenant_id);

    // Try to write invalid JSON (starts with { but invalid)
    let result = hooks.handle_write("/.tarbox/layers/new", b"{invalid json}").await;

    match result {
        HookResult::Error(_) => {
            // Expected error for invalid JSON
        }
        _ => panic!("Expected Error result for invalid JSON, got {:?}", result),
    }

    cleanup_tenant(&pool, &tenant_name).await?;
    Ok(())
}

#[tokio::test]
async fn test_switch_to_nonexistent_layer_name_fails() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("hooks_test_noname_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let _fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    let hooks = HooksHandler::new(pool.pool(), tenant.tenant_id);

    // Try to switch to non-existent layer by name
    let result = hooks.handle_write("/.tarbox/layers/switch", b"nonexistent_layer_name").await;

    match result {
        HookResult::Error(_) => {
            // Expected error
        }
        _ => panic!("Expected Error for non-existent layer name, got {:?}", result),
    }

    cleanup_tenant(&pool, &tenant_name).await?;
    Ok(())
}

#[tokio::test]
async fn test_get_attr_for_hook_paths() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("hooks_test_attr_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let hooks = HooksHandler::new(pool.pool(), tenant.tenant_id);

    // Check attributes for various hook paths
    assert!(hooks.get_attr("/.tarbox").is_some(), "Should have attr for /.tarbox");
    assert!(hooks.get_attr("/.tarbox/layers").is_some(), "Should have attr for layers dir");
    assert!(
        hooks.get_attr("/.tarbox/layers/current").is_some(),
        "Should have attr for current file"
    );
    assert!(hooks.get_attr("/.tarbox/layers/new").is_some(), "Should have attr for new file");
    assert!(hooks.get_attr("/normal/path").is_none(), "Should not have attr for normal path");

    cleanup_tenant(&pool, &tenant_name).await?;
    Ok(())
}

#[tokio::test]
async fn test_write_tarbox_layers_new() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("hooks_test_new_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let _fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    let hooks = HooksHandler::new(pool.pool(), tenant.tenant_id);

    // Create a checkpoint via hooks (simple text input = layer name)
    let checkpoint_name = format!("hook_checkpoint_{}", Uuid::new_v4());
    let result = hooks.handle_write("/.tarbox/layers/new", checkpoint_name.as_bytes()).await;

    match result {
        HookResult::WriteSuccess { .. } => {
            // Success - verify new layer was created
            let layer_mgr = LayerManager::new(pool.pool(), tenant.tenant_id);
            let layers = layer_mgr.list_layers().await?;
            assert!(layers.len() >= 2, "Should have base layer + new checkpoint");
            assert!(
                layers.iter().any(|l| l.layer_name == checkpoint_name),
                "Should find new checkpoint by name"
            );
        }
        _ => panic!("Expected WriteSuccess result, got {:?}", result),
    }

    cleanup_tenant(&pool, &tenant_name).await?;
    Ok(())
}

#[tokio::test]
async fn test_write_tarbox_layers_switch() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("hooks_test_switch_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let _fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    let layer_mgr = LayerManager::new(pool.pool(), tenant.tenant_id);

    // Create a checkpoint first
    let checkpoint_name = format!("checkpoint_for_switch_{}", Uuid::new_v4());
    let layer_info = layer_mgr.create_checkpoint(&checkpoint_name, Some("Test layer")).await?;

    let hooks = HooksHandler::new(pool.pool(), tenant.tenant_id);

    // Switch to the checkpoint via hooks
    let result = hooks
        .handle_write("/.tarbox/layers/switch", layer_info.layer_id.to_string().as_bytes())
        .await;

    match result {
        HookResult::WriteSuccess { .. } => {
            // Success - verify switch happened by reading current layer
            let current_result = hooks.handle_read("/.tarbox/layers/current").await;
            match current_result {
                HookResult::Content(content) => {
                    assert!(
                        content.contains(&layer_info.layer_id.to_string()),
                        "Current layer should be the switched layer"
                    );
                }
                _ => panic!("Expected Content when reading current layer"),
            }
        }
        _ => panic!("Expected WriteSuccess result, got {:?}", result),
    }

    cleanup_tenant(&pool, &tenant_name).await?;
    Ok(())
}

#[tokio::test]
async fn test_read_layers_list() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("hooks_test_list_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let _fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    let layer_mgr = LayerManager::new(pool.pool(), tenant.tenant_id);

    // Create multiple checkpoints
    let name1 = format!("checkpoint1_{}", Uuid::new_v4());
    let name2 = format!("checkpoint2_{}", Uuid::new_v4());
    layer_mgr.create_checkpoint(&name1, None).await?;
    layer_mgr.create_checkpoint(&name2, None).await?;

    let hooks = HooksHandler::new(pool.pool(), tenant.tenant_id);

    // Read layers list
    let result = hooks.handle_read("/.tarbox/layers/list").await;

    match result {
        HookResult::Content(content) => {
            // Should contain all layer names
            assert!(content.contains("base"));
            assert!(content.contains(&name1));
            assert!(content.contains(&name2));
        }
        _ => panic!("Expected Content result, got {:?}", result),
    }

    cleanup_tenant(&pool, &tenant_name).await?;
    Ok(())
}

#[tokio::test]
async fn test_read_layers_tree() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("hooks_test_tree_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let _fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    let layer_mgr = LayerManager::new(pool.pool(), tenant.tenant_id);

    // Create a checkpoint
    let checkpoint_name = format!("tree_checkpoint_{}", Uuid::new_v4());
    layer_mgr.create_checkpoint(&checkpoint_name, Some("For tree test")).await?;

    let hooks = HooksHandler::new(pool.pool(), tenant.tenant_id);

    // Read layer tree
    let result = hooks.handle_read("/.tarbox/layers/tree").await;

    match result {
        HookResult::Content(content) => {
            // Should show layer hierarchy
            assert!(!content.is_empty());
            assert!(content.contains("base") || content.contains(&checkpoint_name));
        }
        _ => panic!("Expected Content result, got {:?}", result),
    }

    cleanup_tenant(&pool, &tenant_name).await?;
    Ok(())
}

#[tokio::test]
async fn test_write_invalid_layer_switch_fails() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("hooks_test_invalid_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let _fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    let hooks = HooksHandler::new(pool.pool(), tenant.tenant_id);

    // Try to switch to non-existent layer
    let fake_layer_id = Uuid::new_v4();
    let result =
        hooks.handle_write("/.tarbox/layers/switch", fake_layer_id.to_string().as_bytes()).await;

    match result {
        HookResult::Error(_) => {
            // Expected error
        }
        _ => panic!("Expected Error result for invalid layer switch, got {:?}", result),
    }

    cleanup_tenant(&pool, &tenant_name).await?;
    Ok(())
}

#[tokio::test]
async fn test_create_checkpoint_without_description() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("hooks_test_no_desc_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let _fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    let hooks = HooksHandler::new(pool.pool(), tenant.tenant_id);

    // Create checkpoint with just name, no description
    let checkpoint_name = format!("no_desc_{}", Uuid::new_v4());
    let result = hooks.handle_write("/.tarbox/layers/new", checkpoint_name.as_bytes()).await;

    match result {
        HookResult::WriteSuccess { .. } => {
            // Verify layer was created
            let layer_mgr = LayerManager::new(pool.pool(), tenant.tenant_id);
            let layers = layer_mgr.list_layers().await?;
            assert!(layers.iter().any(|l| l.layer_name == checkpoint_name));
        }
        _ => panic!("Expected WriteSuccess result, got {:?}", result),
    }

    cleanup_tenant(&pool, &tenant_name).await?;
    Ok(())
}

#[tokio::test]
async fn test_write_to_readonly_file_fails() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("hooks_test_readonly_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let _fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;
    let hooks = HooksHandler::new(pool.pool(), tenant.tenant_id);

    // Try to write to read-only "current" file
    let result = hooks.handle_write("/.tarbox/layers/current", b"should fail").await;

    match result {
        HookResult::Error(_) => {
            // Expected error for writing to readonly file
        }
        _ => panic!("Expected Error result for writing to readonly file, got {:?}", result),
    }

    cleanup_tenant(&pool, &tenant_name).await?;
    Ok(())
}

#[tokio::test]
async fn test_is_hook_path() -> Result<()> {
    assert!(HooksHandler::is_hook_path("/.tarbox"));
    assert!(HooksHandler::is_hook_path("/.tarbox/layers"));
    assert!(HooksHandler::is_hook_path("/.tarbox/layers/current"));
    assert!(HooksHandler::is_hook_path("/.tarbox/stats/usage"));

    assert!(!HooksHandler::is_hook_path("/normal/path"));
    assert!(!HooksHandler::is_hook_path("/tarbox"));
    assert!(!HooksHandler::is_hook_path("/.tar"));

    Ok(())
}

#[tokio::test]
async fn test_read_nonhook_path_returns_not_a_hook() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("hooks_test_nonhook_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let hooks = HooksHandler::new(pool.pool(), tenant.tenant_id);

    // Try to read a normal path
    let result = hooks.handle_read("/normal/file.txt").await;

    match result {
        HookResult::NotAHook => {
            // Expected - this is not a hook path
        }
        _ => panic!("Expected NotAHook result, got {:?}", result),
    }

    cleanup_tenant(&pool, &tenant_name).await?;
    Ok(())
}

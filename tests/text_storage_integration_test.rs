use anyhow::Result;
use tarbox::config::DatabaseConfig;
use tarbox::storage::{
    CreateLayerInput, CreateTenantInput, CreateTextBlockInput, CreateTextMetadataInput,
    DatabasePool, LayerOperations, LayerRepository, TenantOperations, TenantRepository,
    TextBlockOperations, TextBlockRepository,
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
async fn test_text_block_create() -> Result<()> {
    let (pool, _tenant_id) = setup_test_db().await?;
    let text_ops = TextBlockOperations::new(pool.pool());

    let input = CreateTextBlockInput {
        content: "Line 1\nLine 2\nLine 3\n".to_string(),
        encoding: "UTF-8".to_string(),
    };

    let block = text_ops.create_block(input).await?;

    assert_eq!(block.line_count, 3);
    assert_eq!(block.encoding, "UTF-8");
    assert_eq!(block.ref_count, 0);
    assert!(!block.content_hash.is_empty());
    assert_eq!(block.content, "Line 1\nLine 2\nLine 3\n");

    Ok(())
}

#[tokio::test]
async fn test_text_block_deduplication() -> Result<()> {
    let (pool, _tenant_id) = setup_test_db().await?;
    let text_ops = TextBlockOperations::new(pool.pool());

    let content = "Hello, world!\n".to_string();

    // Create first block
    let input1 = CreateTextBlockInput { content: content.clone(), encoding: "UTF-8".to_string() };
    let block1 = text_ops.create_block(input1).await?;

    // Try to create same content again - should return existing block
    let input2 = CreateTextBlockInput { content: content.clone(), encoding: "UTF-8".to_string() };
    let block2 = text_ops.create_block(input2).await?;

    // Should be the same block (deduplication)
    assert_eq!(block1.block_id, block2.block_id);
    assert_eq!(block1.content_hash, block2.content_hash);

    Ok(())
}

#[tokio::test]
async fn test_text_block_get() -> Result<()> {
    let (pool, _tenant_id) = setup_test_db().await?;
    let text_ops = TextBlockOperations::new(pool.pool());

    let input = CreateTextBlockInput {
        content: "Test content\n".to_string(),
        encoding: "UTF-8".to_string(),
    };

    let created = text_ops.create_block(input).await?;

    // Get by ID
    let found = text_ops.get_block(created.block_id).await?;
    assert!(found.is_some());

    let block = found.unwrap();
    assert_eq!(block.block_id, created.block_id);
    assert_eq!(block.content, "Test content\n");

    Ok(())
}

#[tokio::test]
async fn test_text_block_get_by_hash() -> Result<()> {
    let (pool, _tenant_id) = setup_test_db().await?;
    let text_ops = TextBlockOperations::new(pool.pool());

    let input = CreateTextBlockInput {
        content: "Unique content\n".to_string(),
        encoding: "UTF-8".to_string(),
    };

    let created = text_ops.create_block(input).await?;

    // Get by hash
    let found = text_ops.get_block_by_hash(&created.content_hash).await?;
    assert!(found.is_some());

    let block = found.unwrap();
    assert_eq!(block.block_id, created.block_id);
    assert_eq!(block.content_hash, created.content_hash);

    Ok(())
}

#[tokio::test]
async fn test_text_block_ref_count() -> Result<()> {
    let (pool, _tenant_id) = setup_test_db().await?;
    let text_ops = TextBlockOperations::new(pool.pool());

    let input = CreateTextBlockInput {
        content: "Reference counted\n".to_string(),
        encoding: "UTF-8".to_string(),
    };

    let block = text_ops.create_block(input).await?;
    assert_eq!(block.ref_count, 0);

    // Increment ref count
    text_ops.increment_ref_count(block.block_id).await?;
    let after_inc = text_ops.get_block(block.block_id).await?.unwrap();
    assert_eq!(after_inc.ref_count, 1);

    // Increment again
    text_ops.increment_ref_count(block.block_id).await?;
    let after_inc2 = text_ops.get_block(block.block_id).await?.unwrap();
    assert_eq!(after_inc2.ref_count, 2);

    // Decrement ref count
    let new_count = text_ops.decrement_ref_count(block.block_id).await?;
    assert_eq!(new_count, 1);

    // Decrement again
    let new_count2 = text_ops.decrement_ref_count(block.block_id).await?;
    assert_eq!(new_count2, 0);

    // Decrement when already 0 - should stay 0
    let new_count3 = text_ops.decrement_ref_count(block.block_id).await?;
    assert_eq!(new_count3, 0);

    Ok(())
}

#[tokio::test]
async fn test_text_file_metadata_create() -> Result<()> {
    let (pool, tenant_id) = setup_test_db().await?;
    let text_ops = TextBlockOperations::new(pool.pool());
    let layer_ops = LayerOperations::new(pool.pool());

    // Create a real layer
    let layer = layer_ops
        .create(CreateLayerInput {
            tenant_id,
            parent_layer_id: None,
            layer_name: "test-layer".to_string(),
            description: Some("Test layer for text metadata".to_string()),
            tags: None,
            created_by: "test".to_string(),
        })
        .await?;

    let input = CreateTextMetadataInput {
        tenant_id,
        inode_id: 123,
        layer_id: layer.layer_id,
        total_lines: 10,
        encoding: "UTF-8".to_string(),
        line_ending: "LF".to_string(),
        has_trailing_newline: true,
    };

    let metadata = text_ops.create_metadata(input).await?;

    assert_eq!(metadata.tenant_id, tenant_id);
    assert_eq!(metadata.inode_id, 123);
    assert_eq!(metadata.layer_id, layer.layer_id);
    assert_eq!(metadata.total_lines, 10);
    assert_eq!(metadata.encoding, "UTF-8");
    assert_eq!(metadata.line_ending, "LF");
    assert!(metadata.has_trailing_newline);

    Ok(())
}

#[tokio::test]
async fn test_text_file_metadata_get() -> Result<()> {
    let (pool, tenant_id) = setup_test_db().await?;
    let text_ops = TextBlockOperations::new(pool.pool());
    let layer_ops = LayerOperations::new(pool.pool());

    // Create a real layer
    let layer = layer_ops
        .create(CreateLayerInput {
            tenant_id,
            parent_layer_id: None,
            layer_name: "test-layer-get".to_string(),
            description: Some("Test layer for get metadata".to_string()),
            tags: None,
            created_by: "test".to_string(),
        })
        .await?;

    let inode_id = 456;

    let input = CreateTextMetadataInput {
        tenant_id,
        inode_id,
        layer_id: layer.layer_id,
        total_lines: 5,
        encoding: "UTF-8".to_string(),
        line_ending: "CRLF".to_string(),
        has_trailing_newline: false,
    };

    text_ops.create_metadata(input).await?;

    // Get metadata
    let found = text_ops.get_metadata(tenant_id, inode_id, layer.layer_id).await?;
    assert!(found.is_some());

    let metadata = found.unwrap();
    assert_eq!(metadata.total_lines, 5);
    assert_eq!(metadata.line_ending, "CRLF");
    assert!(!metadata.has_trailing_newline);

    Ok(())
}

#[tokio::test]
async fn test_text_line_mappings() -> Result<()> {
    let (pool, tenant_id) = setup_test_db().await?;
    let text_ops = TextBlockOperations::new(pool.pool());
    let layer_ops = LayerOperations::new(pool.pool());

    // Create a real layer
    let layer = layer_ops
        .create(CreateLayerInput {
            tenant_id,
            parent_layer_id: None,
            layer_name: "test-layer-mappings".to_string(),
            description: Some("Test layer for line mappings".to_string()),
            tags: None,
            created_by: "test".to_string(),
        })
        .await?;

    let inode_id = 789;

    // Create metadata first
    let metadata_input = CreateTextMetadataInput {
        tenant_id,
        inode_id,
        layer_id: layer.layer_id,
        total_lines: 3,
        encoding: "UTF-8".to_string(),
        line_ending: "LF".to_string(),
        has_trailing_newline: true,
    };
    text_ops.create_metadata(metadata_input).await?;

    // Create text blocks
    let block1_input =
        CreateTextBlockInput { content: "Line 1\n".to_string(), encoding: "UTF-8".to_string() };
    let block1 = text_ops.create_block(block1_input).await?;

    let block2_input =
        CreateTextBlockInput { content: "Line 2\n".to_string(), encoding: "UTF-8".to_string() };
    let block2 = text_ops.create_block(block2_input).await?;

    let block3_input =
        CreateTextBlockInput { content: "Line 3\n".to_string(), encoding: "UTF-8".to_string() };
    let block3 = text_ops.create_block(block3_input).await?;

    // Create line mappings
    let mappings = vec![(1, block1.block_id, 0), (2, block2.block_id, 0), (3, block3.block_id, 0)];

    let count =
        text_ops.create_line_mappings(tenant_id, inode_id, layer.layer_id, mappings).await?;
    assert_eq!(count, 3);

    // Get line mappings
    let retrieved = text_ops.get_line_mappings(tenant_id, inode_id, layer.layer_id).await?;
    assert_eq!(retrieved.len(), 3);

    // Verify order and content
    assert_eq!(retrieved[0].line_number, 1);
    assert_eq!(retrieved[0].block_id, block1.block_id);

    assert_eq!(retrieved[1].line_number, 2);
    assert_eq!(retrieved[1].block_id, block2.block_id);

    assert_eq!(retrieved[2].line_number, 3);
    assert_eq!(retrieved[2].block_id, block3.block_id);

    Ok(())
}

#[tokio::test]
async fn test_text_block_multiline() -> Result<()> {
    let (pool, _tenant_id) = setup_test_db().await?;
    let text_ops = TextBlockOperations::new(pool.pool());

    // Create a block with multiple lines
    let input = CreateTextBlockInput {
        content: "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n".to_string(),
        encoding: "UTF-8".to_string(),
    };

    let block = text_ops.create_block(input).await?;

    assert_eq!(block.line_count, 5);
    assert!(block.byte_size > 0);

    Ok(())
}

#[tokio::test]
async fn test_text_line_mapping_with_block_offsets() -> Result<()> {
    let (pool, tenant_id) = setup_test_db().await?;
    let text_ops = TextBlockOperations::new(pool.pool());
    let layer_ops = LayerOperations::new(pool.pool());

    // Create a real layer
    let layer = layer_ops
        .create(CreateLayerInput {
            tenant_id,
            parent_layer_id: None,
            layer_name: "test-layer-offsets".to_string(),
            description: Some("Test layer for block offsets".to_string()),
            tags: None,
            created_by: "test".to_string(),
        })
        .await?;

    let inode_id = 999;

    // Create metadata
    let metadata_input = CreateTextMetadataInput {
        tenant_id,
        inode_id,
        layer_id: layer.layer_id,
        total_lines: 5,
        encoding: "UTF-8".to_string(),
        line_ending: "LF".to_string(),
        has_trailing_newline: true,
    };
    text_ops.create_metadata(metadata_input).await?;

    // Create a multi-line block
    let block_input = CreateTextBlockInput {
        content: "Block Line 1\nBlock Line 2\nBlock Line 3\n".to_string(),
        encoding: "UTF-8".to_string(),
    };
    let block = text_ops.create_block(block_input).await?;

    // Map different lines in the file to different offsets within the same block
    let mappings = vec![
        (1, block.block_id, 0), // First line of block
        (2, block.block_id, 1), // Second line of block
        (3, block.block_id, 2), // Third line of block
    ];

    let count =
        text_ops.create_line_mappings(tenant_id, inode_id, layer.layer_id, mappings).await?;
    assert_eq!(count, 3);

    // Retrieve and verify
    let retrieved = text_ops.get_line_mappings(tenant_id, inode_id, layer.layer_id).await?;
    assert_eq!(retrieved.len(), 3);

    assert_eq!(retrieved[0].block_line_offset, 0);
    assert_eq!(retrieved[1].block_line_offset, 1);
    assert_eq!(retrieved[2].block_line_offset, 2);

    Ok(())
}

#[tokio::test]
async fn test_empty_line_mappings() -> Result<()> {
    let (pool, tenant_id) = setup_test_db().await?;
    let text_ops = TextBlockOperations::new(pool.pool());

    let layer_id = Uuid::new_v4();
    let inode_id = 111;

    // Try to create empty mappings
    let count = text_ops.create_line_mappings(tenant_id, inode_id, layer_id, vec![]).await?;
    assert_eq!(count, 0);

    Ok(())
}

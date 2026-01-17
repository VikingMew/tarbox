use anyhow::Result;
use tarbox::config::DatabaseConfig;
use tarbox::storage::{
    BlockOperations, CreateBlockInput, CreateInodeInput, CreateTenantInput, DatabasePool,
    InodeOperations, InodeType, TenantOperations, TenantRepository, UpdateInodeInput,
};

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
async fn test_tenant_crud() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_tenant_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    assert_eq!(tenant.tenant_name, tenant_name);
    assert!(tenant.root_inode_id > 0);

    let found: Option<_> = tenant_ops.get_by_id(tenant.tenant_id).await?;
    assert!(found.is_some());
    assert_eq!(found.unwrap().tenant_name, tenant_name);

    let found_by_name: Option<_> = tenant_ops.get_by_name(&tenant_name).await?;
    assert!(found_by_name.is_some());
    assert_eq!(found_by_name.unwrap().tenant_id, tenant.tenant_id);

    let deleted = tenant_ops.delete(tenant.tenant_id).await?;
    assert!(deleted);

    let not_found: Option<_> = tenant_ops.get_by_id(tenant.tenant_id).await?;
    assert!(not_found.is_none());

    Ok(())
}

#[tokio::test]
async fn test_inode_crud() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());
    let inode_ops = InodeOperations::new(pool.pool());

    let tenant_name = format!("test_inode_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let root_inode: Option<_> = inode_ops.get(tenant.tenant_id, tenant.root_inode_id).await?;
    assert!(root_inode.is_some());
    let root = root_inode.unwrap();
    assert_eq!(root.name, "/");
    assert_eq!(root.inode_type, InodeType::Dir);

    let file_inode = inode_ops
        .create(CreateInodeInput {
            tenant_id: tenant.tenant_id,
            parent_id: Some(tenant.root_inode_id),
            name: "test.txt".to_string(),
            inode_type: InodeType::File,
            mode: 0o644,
            uid: 1000,
            gid: 1000,
        })
        .await?;

    assert_eq!(file_inode.name, "test.txt");
    assert_eq!(file_inode.inode_type, InodeType::File);
    assert_eq!(file_inode.parent_id, Some(tenant.root_inode_id));

    let found: Option<_> = inode_ops
        .get_by_parent_and_name(tenant.tenant_id, tenant.root_inode_id, "test.txt")
        .await?;
    assert!(found.is_some());
    assert_eq!(found.unwrap().inode_id, file_inode.inode_id);

    let updated = inode_ops
        .update(
            tenant.tenant_id,
            file_inode.inode_id,
            UpdateInodeInput {
                size: Some(1024),
                mode: None,
                uid: None,
                gid: None,
                atime: None,
                mtime: None,
                ctime: None,
            },
        )
        .await?;
    assert_eq!(updated.size, 1024);

    let children: Vec<_> = inode_ops.list_children(tenant.tenant_id, tenant.root_inode_id).await?;
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].name, "test.txt");

    let deleted = inode_ops.delete(tenant.tenant_id, file_inode.inode_id).await?;
    assert!(deleted);

    tenant_ops.delete(tenant.tenant_id).await?;

    Ok(())
}

#[tokio::test]
async fn test_data_block_crud() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());
    let inode_ops = InodeOperations::new(pool.pool());
    let block_ops = BlockOperations::new(pool.pool());

    let tenant_name = format!("test_block_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let file_inode = inode_ops
        .create(CreateInodeInput {
            tenant_id: tenant.tenant_id,
            parent_id: Some(tenant.root_inode_id),
            name: "data.bin".to_string(),
            inode_type: InodeType::File,
            mode: 0o644,
            uid: 1000,
            gid: 1000,
        })
        .await?;

    let data1 = b"Hello, world!".to_vec();
    let block1 = block_ops
        .create(CreateBlockInput {
            tenant_id: tenant.tenant_id,
            inode_id: file_inode.inode_id,
            block_index: 0,
            data: data1.clone(),
        })
        .await?;

    assert_eq!(block1.size, data1.len() as i32);
    assert_eq!(block1.data, data1);
    assert_eq!(block1.block_index, 0);

    let data2 = b"Second block".to_vec();
    let block2 = block_ops
        .create(CreateBlockInput {
            tenant_id: tenant.tenant_id,
            inode_id: file_inode.inode_id,
            block_index: 1,
            data: data2.clone(),
        })
        .await?;

    assert_eq!(block2.block_index, 1);

    let found_block: Option<_> = block_ops.get(tenant.tenant_id, file_inode.inode_id, 0).await?;
    assert!(found_block.is_some());
    assert_eq!(found_block.unwrap().data, data1);

    let all_blocks: Vec<_> = block_ops.list(tenant.tenant_id, file_inode.inode_id).await?;
    assert_eq!(all_blocks.len(), 2);
    assert_eq!(all_blocks[0].block_index, 0);
    assert_eq!(all_blocks[1].block_index, 1);

    let deleted_count = block_ops.delete(tenant.tenant_id, file_inode.inode_id).await?;
    assert_eq!(deleted_count, 2);

    let no_blocks: Vec<_> = block_ops.list(tenant.tenant_id, file_inode.inode_id).await?;
    assert_eq!(no_blocks.len(), 0);

    tenant_ops.delete(tenant.tenant_id).await?;

    Ok(())
}

#[tokio::test]
async fn test_transaction() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_name = format!("test_tx_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let mut tx = pool.begin_transaction().await?;

    let tenant_id = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO tenants (tenant_id, tenant_name, root_inode_id) VALUES ($1, $2, $3)")
        .bind(tenant_id)
        .bind(&tenant_name)
        .bind(1_i64)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    let tenant_ops = TenantOperations::new(pool.pool());
    let found: Option<_> = tenant_ops.get_by_id(tenant_id).await?;
    assert!(found.is_some());

    tenant_ops.delete(tenant_id).await?;

    Ok(())
}

#[tokio::test]
async fn test_transaction_rollback() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_name = format!("test_rollback_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant_id = uuid::Uuid::new_v4();

    {
        let mut tx = pool.begin_transaction().await?;

        sqlx::query(
            "INSERT INTO tenants (tenant_id, tenant_name, root_inode_id) VALUES ($1, $2, $3)",
        )
        .bind(tenant_id)
        .bind(&tenant_name)
        .bind(1_i64)
        .execute(&mut *tx)
        .await?;

        tx.rollback().await?;
    }

    let tenant_ops = TenantOperations::new(pool.pool());
    let not_found: Option<_> = tenant_ops.get_by_id(tenant_id).await?;
    assert!(not_found.is_none());

    Ok(())
}

#[tokio::test]
async fn test_content_hash_deduplication() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());
    let inode_ops = InodeOperations::new(pool.pool());
    let block_ops = BlockOperations::new(pool.pool());

    let tenant_name = format!("test_dedup_{}", uuid::Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;

    let file1 = inode_ops
        .create(CreateInodeInput {
            tenant_id: tenant.tenant_id,
            parent_id: Some(tenant.root_inode_id),
            name: "file1.txt".to_string(),
            inode_type: InodeType::File,
            mode: 0o644,
            uid: 1000,
            gid: 1000,
        })
        .await?;

    let file2 = inode_ops
        .create(CreateInodeInput {
            tenant_id: tenant.tenant_id,
            parent_id: Some(tenant.root_inode_id),
            name: "file2.txt".to_string(),
            inode_type: InodeType::File,
            mode: 0o644,
            uid: 1000,
            gid: 1000,
        })
        .await?;

    let same_data = b"identical content".to_vec();

    let block1 = block_ops
        .create(CreateBlockInput {
            tenant_id: tenant.tenant_id,
            inode_id: file1.inode_id,
            block_index: 0,
            data: same_data.clone(),
        })
        .await?;

    let block2 = block_ops
        .create(CreateBlockInput {
            tenant_id: tenant.tenant_id,
            inode_id: file2.inode_id,
            block_index: 0,
            data: same_data.clone(),
        })
        .await?;

    assert_eq!(block1.content_hash, block2.content_hash);

    tenant_ops.delete(tenant.tenant_id).await?;

    Ok(())
}

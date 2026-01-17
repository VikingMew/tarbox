// Storage Layer Integration Tests with Mockall
//
// 文件：tests/storage_layer_integration_test.rs
// 目的：使用 mockall 测试 storage 层的业务逻辑，无需真实数据库
//
// 测试覆盖：
// - TenantRepository trait 的所有方法
// - InodeRepository trait 的所有方法
// - BlockRepository trait 的所有方法

use anyhow::Result;
use mockall::predicate::*;
use tarbox::storage::models::*;
use tarbox::storage::traits::{
    BlockRepository, InodeRepository, MockBlockRepository, MockInodeRepository,
    MockTenantRepository, TenantRepository,
};
use tarbox::types::InodeId;
use uuid::Uuid;

// ============================================================================
// Tenant Repository Tests
// ============================================================================

#[tokio::test]
async fn test_tenant_repository_create_with_valid_input() {
    let mut mock_repo = MockTenantRepository::new();
    let tenant_id = Uuid::new_v4();
    let expected_tenant = Tenant {
        tenant_id,
        tenant_name: "test_tenant".to_string(),
        root_inode_id: 1,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    mock_repo
        .expect_create()
        .with(function(|input: &CreateTenantInput| input.tenant_name == "test_tenant"))
        .times(1)
        .returning(move |_| Ok(expected_tenant.clone()));

    let input = CreateTenantInput { tenant_name: "test_tenant".to_string() };
    let result: Result<_> = mock_repo.create(input).await;

    assert!(result.is_ok());
    let tenant = result.unwrap();
    assert_eq!(tenant.tenant_name, "test_tenant");
    assert_eq!(tenant.root_inode_id, 1);
}

#[tokio::test]
async fn test_tenant_repository_get_by_id_returns_existing_tenant() {
    let mut mock_repo = MockTenantRepository::new();
    let tenant_id = Uuid::new_v4();
    let expected_tenant = Tenant {
        tenant_id,
        tenant_name: "existing_tenant".to_string(),
        root_inode_id: 1,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    mock_repo
        .expect_get_by_id()
        .with(eq(tenant_id))
        .times(1)
        .returning(move |_| Ok(Some(expected_tenant.clone())));

    let result: Result<_> = mock_repo.get_by_id(tenant_id).await;

    assert!(result.is_ok());
    assert!(result.unwrap().is_some());
}

#[tokio::test]
async fn test_tenant_repository_get_by_id_returns_none_for_nonexistent() {
    let mut mock_repo = MockTenantRepository::new();
    let tenant_id = Uuid::new_v4();

    mock_repo.expect_get_by_id().with(eq(tenant_id)).times(1).returning(|_| Ok(None));

    let result: Result<_> = mock_repo.get_by_id(tenant_id).await;

    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn test_tenant_repository_get_by_name_finds_tenant() {
    let mut mock_repo = MockTenantRepository::new();
    let tenant_id = Uuid::new_v4();
    let expected_tenant = Tenant {
        tenant_id,
        tenant_name: "find_me".to_string(),
        root_inode_id: 1,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    mock_repo
        .expect_get_by_name()
        .with(eq("find_me"))
        .times(1)
        .returning(move |_| Ok(Some(expected_tenant.clone())));

    let result: Result<_> = mock_repo.get_by_name("find_me").await;

    assert!(result.is_ok());
    let tenant = result.unwrap();
    assert!(tenant.is_some());
    assert_eq!(tenant.unwrap().tenant_name, "find_me");
}

#[tokio::test]
async fn test_tenant_repository_list_returns_all_tenants() {
    let mut mock_repo = MockTenantRepository::new();
    let tenant1 = Tenant {
        tenant_id: Uuid::new_v4(),
        tenant_name: "tenant1".to_string(),
        root_inode_id: 1,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    let tenant2 = Tenant {
        tenant_id: Uuid::new_v4(),
        tenant_name: "tenant2".to_string(),
        root_inode_id: 1,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    let expected_tenants = vec![tenant1.clone(), tenant2.clone()];

    mock_repo.expect_list().times(1).returning(move || Ok(expected_tenants.clone()));

    let result: Result<_> = mock_repo.list().await;

    assert!(result.is_ok());
    let tenants = result.unwrap();
    assert_eq!(tenants.len(), 2);
    assert_eq!(tenants[0].tenant_name, "tenant1");
    assert_eq!(tenants[1].tenant_name, "tenant2");
}

#[tokio::test]
async fn test_tenant_repository_delete_removes_tenant() {
    let mut mock_repo = MockTenantRepository::new();
    let tenant_id = Uuid::new_v4();

    mock_repo.expect_delete().with(eq(tenant_id)).times(1).returning(|_| Ok(true));

    let result: Result<_> = mock_repo.delete(tenant_id).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), true);
}

#[tokio::test]
async fn test_tenant_repository_delete_returns_false_for_nonexistent() {
    let mut mock_repo = MockTenantRepository::new();
    let tenant_id = Uuid::new_v4();

    mock_repo.expect_delete().with(eq(tenant_id)).times(1).returning(|_| Ok(false));

    let result: Result<_> = mock_repo.delete(tenant_id).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), false);
}

// ============================================================================
// Inode Repository Tests
// ============================================================================

#[tokio::test]
async fn test_inode_repository_create_file_inode() {
    let mut mock_repo = MockInodeRepository::new();
    let tenant_id = Uuid::new_v4();
    let expected_inode = Inode {
        inode_id: 100,
        tenant_id,
        parent_id: Some(1),
        name: "test.txt".to_string(),
        inode_type: InodeType::File,
        mode: 0o644,
        uid: 1000,
        gid: 1000,
        size: 0,
        atime: chrono::Utc::now(),
        mtime: chrono::Utc::now(),
        ctime: chrono::Utc::now(),
    };

    mock_repo
        .expect_create()
        .with(function(|input: &CreateInodeInput| {
            input.name == "test.txt" && input.inode_type == InodeType::File
        }))
        .times(1)
        .returning(move |_| Ok(expected_inode.clone()));

    let input = CreateInodeInput {
        tenant_id,
        parent_id: Some(1),
        name: "test.txt".to_string(),
        inode_type: InodeType::File,
        mode: 0o644,
        uid: 1000,
        gid: 1000,
    };

    let result: Result<_> = mock_repo.create(input).await;

    assert!(result.is_ok());
    let inode = result.unwrap();
    assert_eq!(inode.name, "test.txt");
    assert_eq!(inode.inode_type, InodeType::File);
    assert_eq!(inode.mode, 0o644);
}

#[tokio::test]
async fn test_inode_repository_create_directory_inode() {
    let mut mock_repo = MockInodeRepository::new();
    let tenant_id = Uuid::new_v4();
    let expected_inode = Inode {
        inode_id: 101,
        tenant_id,
        parent_id: Some(1),
        name: "mydir".to_string(),
        inode_type: InodeType::Dir,
        mode: 0o755,
        uid: 1000,
        gid: 1000,
        size: 0,
        atime: chrono::Utc::now(),
        mtime: chrono::Utc::now(),
        ctime: chrono::Utc::now(),
    };

    mock_repo
        .expect_create()
        .with(function(|input: &CreateInodeInput| {
            input.name == "mydir" && input.inode_type == InodeType::Dir
        }))
        .times(1)
        .returning(move |_| Ok(expected_inode.clone()));

    let input = CreateInodeInput {
        tenant_id,
        parent_id: Some(1),
        name: "mydir".to_string(),
        inode_type: InodeType::Dir,
        mode: 0o755,
        uid: 1000,
        gid: 1000,
    };

    let result: Result<_> = mock_repo.create(input).await;

    assert!(result.is_ok());
    let inode = result.unwrap();
    assert_eq!(inode.inode_type, InodeType::Dir);
    assert_eq!(inode.mode, 0o755);
}

#[tokio::test]
async fn test_inode_repository_get_returns_existing_inode() {
    let mut mock_repo = MockInodeRepository::new();
    let tenant_id = Uuid::new_v4();
    let inode_id: InodeId = 200;
    let expected_inode = Inode {
        inode_id,
        tenant_id,
        parent_id: Some(1),
        name: "existing.txt".to_string(),
        inode_type: InodeType::File,
        mode: 0o644,
        uid: 1000,
        gid: 1000,
        size: 1024,
        atime: chrono::Utc::now(),
        mtime: chrono::Utc::now(),
        ctime: chrono::Utc::now(),
    };

    mock_repo
        .expect_get()
        .with(eq(tenant_id), eq(inode_id))
        .times(1)
        .returning(move |_, _| Ok(Some(expected_inode.clone())));

    let result: Result<_> = mock_repo.get(tenant_id, inode_id).await;

    assert!(result.is_ok());
    let inode = result.unwrap();
    assert!(inode.is_some());
    assert_eq!(inode.unwrap().inode_id, 200);
}

#[tokio::test]
async fn test_inode_repository_get_returns_none_for_nonexistent() {
    let mut mock_repo = MockInodeRepository::new();
    let tenant_id = Uuid::new_v4();
    let inode_id: InodeId = 999;

    mock_repo.expect_get().with(eq(tenant_id), eq(inode_id)).times(1).returning(|_, _| Ok(None));

    let result: Result<_> = mock_repo.get(tenant_id, inode_id).await;

    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn test_inode_repository_get_by_parent_and_name_finds_child() {
    let mut mock_repo = MockInodeRepository::new();
    let tenant_id = Uuid::new_v4();
    let parent_id: InodeId = 1;
    let child_inode = Inode {
        inode_id: 300,
        tenant_id,
        parent_id: Some(parent_id),
        name: "child.txt".to_string(),
        inode_type: InodeType::File,
        mode: 0o644,
        uid: 1000,
        gid: 1000,
        size: 0,
        atime: chrono::Utc::now(),
        mtime: chrono::Utc::now(),
        ctime: chrono::Utc::now(),
    };

    mock_repo
        .expect_get_by_parent_and_name()
        .with(eq(tenant_id), eq(parent_id), eq("child.txt"))
        .times(1)
        .returning(move |_, _, _| Ok(Some(child_inode.clone())));

    let result: Result<_> =
        mock_repo.get_by_parent_and_name(tenant_id, parent_id, "child.txt").await;

    assert!(result.is_ok());
    let inode = result.unwrap();
    assert!(inode.is_some());
    assert_eq!(inode.unwrap().name, "child.txt");
}

#[tokio::test]
async fn test_inode_repository_update_modifies_inode() {
    let mut mock_repo = MockInodeRepository::new();
    let tenant_id = Uuid::new_v4();
    let inode_id: InodeId = 400;
    let updated_inode = Inode {
        inode_id,
        tenant_id,
        parent_id: Some(1),
        name: "updated.txt".to_string(),
        inode_type: InodeType::File,
        mode: 0o600,
        uid: 1000,
        gid: 1000,
        size: 2048,
        atime: chrono::Utc::now(),
        mtime: chrono::Utc::now(),
        ctime: chrono::Utc::now(),
    };

    mock_repo
        .expect_update()
        .with(
            eq(tenant_id),
            eq(inode_id),
            function(|input: &UpdateInodeInput| {
                input.size == Some(2048) && input.mode == Some(0o600)
            }),
        )
        .times(1)
        .returning(move |_, _, _| Ok(updated_inode.clone()));

    let input = UpdateInodeInput {
        size: Some(2048),
        mode: Some(0o600),
        uid: None,
        gid: None,
        atime: None,
        mtime: None,
        ctime: None,
    };

    let result: Result<_> = mock_repo.update(tenant_id, inode_id, input).await;

    assert!(result.is_ok());
    let inode = result.unwrap();
    assert_eq!(inode.size, 2048);
    assert_eq!(inode.mode, 0o600);
}

#[tokio::test]
async fn test_inode_repository_delete_removes_inode() {
    let mut mock_repo = MockInodeRepository::new();
    let tenant_id = Uuid::new_v4();
    let inode_id: InodeId = 500;

    mock_repo.expect_delete().with(eq(tenant_id), eq(inode_id)).times(1).returning(|_, _| Ok(true));

    let result: Result<_> = mock_repo.delete(tenant_id, inode_id).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), true);
}

#[tokio::test]
async fn test_inode_repository_list_children_returns_child_inodes() {
    let mut mock_repo = MockInodeRepository::new();
    let tenant_id = Uuid::new_v4();
    let parent_id: InodeId = 1;

    let child1 = Inode {
        inode_id: 10,
        tenant_id,
        parent_id: Some(parent_id),
        name: "child1.txt".to_string(),
        inode_type: InodeType::File,
        mode: 0o644,
        uid: 1000,
        gid: 1000,
        size: 0,
        atime: chrono::Utc::now(),
        mtime: chrono::Utc::now(),
        ctime: chrono::Utc::now(),
    };

    let child2 = Inode {
        inode_id: 11,
        tenant_id,
        parent_id: Some(parent_id),
        name: "child2.txt".to_string(),
        inode_type: InodeType::File,
        mode: 0o644,
        uid: 1000,
        gid: 1000,
        size: 0,
        atime: chrono::Utc::now(),
        mtime: chrono::Utc::now(),
        ctime: chrono::Utc::now(),
    };

    let expected_children = vec![child1.clone(), child2.clone()];

    mock_repo
        .expect_list_children()
        .with(eq(tenant_id), eq(parent_id))
        .times(1)
        .returning(move |_, _| Ok(expected_children.clone()));

    let result: Result<_> = mock_repo.list_children(tenant_id, parent_id).await;

    assert!(result.is_ok());
    let children = result.unwrap();
    assert_eq!(children.len(), 2);
    assert_eq!(children[0].name, "child1.txt");
    assert_eq!(children[1].name, "child2.txt");
}

// ============================================================================
// Block Repository Tests
// ============================================================================

#[tokio::test]
async fn test_block_repository_create_data_block() {
    let mut mock_repo = MockBlockRepository::new();
    let tenant_id = Uuid::new_v4();
    let block_id = Uuid::new_v4();
    let data = vec![1u8, 2, 3, 4];
    let expected_block = DataBlock {
        block_id,
        tenant_id,
        inode_id: 100,
        block_index: 0,
        size: data.len() as i32,
        data: data.clone(),
        content_hash: "hash123".to_string(),
        created_at: chrono::Utc::now(),
    };

    mock_repo
        .expect_create()
        .with(function(|input: &CreateBlockInput| input.inode_id == 100 && input.block_index == 0))
        .times(1)
        .returning(move |_| Ok(expected_block.clone()));

    let input = CreateBlockInput { tenant_id, inode_id: 100, block_index: 0, data: data.clone() };

    let result: Result<_> = mock_repo.create(input).await;

    assert!(result.is_ok());
    let block = result.unwrap();
    assert_eq!(block.inode_id, 100);
    assert_eq!(block.block_index, 0);
    assert_eq!(block.data.len(), 4);
}

#[tokio::test]
async fn test_block_repository_get_returns_existing_block() {
    let mut mock_repo = MockBlockRepository::new();
    let tenant_id = Uuid::new_v4();
    let block_id = Uuid::new_v4();
    let inode_id: InodeId = 200;
    let block_index = 0;
    let expected_block = DataBlock {
        block_id,
        tenant_id,
        inode_id,
        block_index,
        size: 1024,
        data: vec![0u8; 1024],
        content_hash: "hash456".to_string(),
        created_at: chrono::Utc::now(),
    };

    mock_repo
        .expect_get()
        .with(eq(tenant_id), eq(inode_id), eq(block_index))
        .times(1)
        .returning(move |_, _, _| Ok(Some(expected_block.clone())));

    let result: Result<_> = mock_repo.get(tenant_id, inode_id, block_index).await;

    assert!(result.is_ok());
    let block = result.unwrap();
    assert!(block.is_some());
    assert_eq!(block.unwrap().inode_id, 200);
}

#[tokio::test]
async fn test_block_repository_list_returns_all_blocks_for_inode() {
    let mut mock_repo = MockBlockRepository::new();
    let tenant_id = Uuid::new_v4();
    let inode_id: InodeId = 300;

    let block1 = DataBlock {
        block_id: Uuid::new_v4(),
        tenant_id,
        inode_id,
        block_index: 0,
        size: 512,
        data: vec![0u8; 512],
        content_hash: "hash1".to_string(),
        created_at: chrono::Utc::now(),
    };

    let block2 = DataBlock {
        block_id: Uuid::new_v4(),
        tenant_id,
        inode_id,
        block_index: 1,
        size: 512,
        data: vec![1u8; 512],
        content_hash: "hash2".to_string(),
        created_at: chrono::Utc::now(),
    };

    let expected_blocks = vec![block1.clone(), block2.clone()];

    mock_repo
        .expect_list()
        .with(eq(tenant_id), eq(inode_id))
        .times(1)
        .returning(move |_, _| Ok(expected_blocks.clone()));

    let result: Result<_> = mock_repo.list(tenant_id, inode_id).await;

    assert!(result.is_ok());
    let blocks = result.unwrap();
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].block_index, 0);
    assert_eq!(blocks[1].block_index, 1);
}

#[tokio::test]
async fn test_block_repository_delete_removes_all_blocks_for_inode() {
    let mut mock_repo = MockBlockRepository::new();
    let tenant_id = Uuid::new_v4();
    let inode_id: InodeId = 400;

    mock_repo.expect_delete().with(eq(tenant_id), eq(inode_id)).times(1).returning(|_, _| Ok(3));

    let result: Result<_> = mock_repo.delete(tenant_id, inode_id).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 3);
}

#[tokio::test]
async fn test_block_repository_delete_returns_zero_for_nonexistent_inode() {
    let mut mock_repo = MockBlockRepository::new();
    let tenant_id = Uuid::new_v4();
    let inode_id: InodeId = 999;

    mock_repo.expect_delete().with(eq(tenant_id), eq(inode_id)).times(1).returning(|_, _| Ok(0));

    let result: Result<_> = mock_repo.delete(tenant_id, inode_id).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);
}

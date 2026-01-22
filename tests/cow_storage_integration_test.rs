//! Integration tests for COW + Storage integration
//!
//! Tests text file line-level storage and binary file block storage

use anyhow::Result;
use tarbox::config::DatabaseConfig;
use tarbox::fs::operations::FileSystem;
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
async fn test_text_file_line_level_storage() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_line_storage_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;

    fs.create_file("/lines.txt").await?;
    fs.write_file("/lines.txt", b"line1\nline2\nline3\n").await?;

    // Verify 3 text_line_map entries
    let line_map_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM text_line_map WHERE tenant_id = $1")
            .bind(tenant.tenant_id)
            .fetch_one(pool.pool())
            .await?;

    assert_eq!(line_map_count, 3, "Should have 3 line mappings");

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_text_file_deduplication() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_text_dedup_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;

    // Create two files with same lines
    fs.create_file("/file1.txt").await?;
    fs.write_file("/file1.txt", b"same line\n").await?;

    fs.create_file("/file2.txt").await?;
    fs.write_file("/file2.txt", b"same line\n").await?;

    // Count unique text blocks with content "same line"
    let block_count: i64 =
        sqlx::query_scalar("SELECT COUNT(DISTINCT block_id) FROM text_blocks WHERE content = $1")
            .bind("same line")
            .fetch_one(pool.pool())
            .await?;

    // Should only be 1 unique block (deduplication works)
    assert_eq!(block_count, 1, "Same lines should share one text block");

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_binary_file_block_storage() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_binary_blocks_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;

    // Create 5KB binary file (should span 2 blocks: 4KB + 1KB)
    let binary_data: Vec<u8> = (0..5120).map(|i| (i % 256) as u8).collect();

    fs.create_file("/binary.bin").await?;
    fs.write_file("/binary.bin", &binary_data).await?;

    // Verify 2 data_blocks entries
    let block_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM data_blocks WHERE tenant_id = $1")
            .bind(tenant.tenant_id)
            .fetch_one(pool.pool())
            .await?;

    assert_eq!(block_count, 2, "5KB file should span 2 blocks");

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_binary_file_deduplication() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_binary_dedup_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;

    // Create two files with same binary content
    let content = b"binary\x00content";

    fs.create_file("/bin1.bin").await?;
    fs.write_file("/bin1.bin", content).await?;

    fs.create_file("/bin2.bin").await?;
    fs.write_file("/bin2.bin", content).await?;

    // Both files should have blocks with same content_hash
    let hash_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT content_hash) FROM data_blocks WHERE tenant_id = $1",
    )
    .bind(tenant.tenant_id)
    .fetch_one(pool.pool())
    .await?;

    // Should be 1 unique hash (same content)
    assert_eq!(hash_count, 1, "Same binary content should have same hash");

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_text_file_encoding_detection() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_encoding_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;

    // ASCII content
    fs.create_file("/ascii.txt").await?;
    fs.write_file("/ascii.txt", b"hello world\n").await?;

    // UTF-8 content
    fs.create_file("/utf8.txt").await?;
    fs.write_file("/utf8.txt", "你好世界\n".as_bytes()).await?;

    // Verify encodings in text_file_metadata
    let ascii_encoding: String = sqlx::query_scalar(
        "SELECT encoding FROM text_file_metadata tm
         JOIN inodes i ON tm.inode_id = i.inode_id
         WHERE tm.tenant_id = $1 AND i.name = 'ascii.txt'",
    )
    .bind(tenant.tenant_id)
    .fetch_one(pool.pool())
    .await?;

    assert_eq!(ascii_encoding, "ascii");

    let utf8_encoding: String = sqlx::query_scalar(
        "SELECT encoding FROM text_file_metadata tm
         JOIN inodes i ON tm.inode_id = i.inode_id
         WHERE tm.tenant_id = $1 AND i.name = 'utf8.txt'",
    )
    .bind(tenant.tenant_id)
    .fetch_one(pool.pool())
    .await?;

    assert_eq!(utf8_encoding, "utf-8");

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

#[tokio::test]
async fn test_text_file_line_ending_detection() -> Result<()> {
    let pool = setup_test_db().await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant_name = format!("test_line_ending_{}", Uuid::new_v4());
    cleanup_tenant(&pool, &tenant_name).await?;

    let tenant = tenant_ops.create(CreateTenantInput { tenant_name: tenant_name.clone() }).await?;
    let fs = FileSystem::new(pool.pool(), tenant.tenant_id).await?;

    // LF (Unix)
    fs.create_file("/lf.txt").await?;
    fs.write_file("/lf.txt", b"line1\nline2\n").await?;

    // CRLF (Windows)
    fs.create_file("/crlf.txt").await?;
    fs.write_file("/crlf.txt", b"line1\r\nline2\r\n").await?;

    // Verify line endings
    let lf_ending: String = sqlx::query_scalar(
        "SELECT line_ending FROM text_file_metadata tm
         JOIN inodes i ON tm.inode_id = i.inode_id
         WHERE tm.tenant_id = $1 AND i.name = 'lf.txt'",
    )
    .bind(tenant.tenant_id)
    .fetch_one(pool.pool())
    .await?;

    assert_eq!(lf_ending, "LF");

    let crlf_ending: String = sqlx::query_scalar(
        "SELECT line_ending FROM text_file_metadata tm
         JOIN inodes i ON tm.inode_id = i.inode_id
         WHERE tm.tenant_id = $1 AND i.name = 'crlf.txt'",
    )
    .bind(tenant.tenant_id)
    .fetch_one(pool.pool())
    .await?;

    assert_eq!(crlf_ending, "CRLF");

    tenant_ops.delete(tenant.tenant_id).await?;
    Ok(())
}

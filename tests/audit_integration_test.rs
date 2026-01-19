use anyhow::Result;
use chrono::Utc;
use tarbox::config::DatabaseConfig;
use tarbox::storage::{
    AuditLogOperations, AuditLogRepository, CreateAuditLogInput, CreateTenantInput, DatabasePool,
    QueryAuditLogsInput, TenantOperations, TenantRepository,
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
async fn test_audit_log_create() -> Result<()> {
    let (pool, tenant_id) = setup_test_db().await?;
    let audit_ops = AuditLogOperations::new(pool.pool());

    let input = CreateAuditLogInput {
        tenant_id,
        inode_id: None,
        operation: "write".to_string(),
        uid: 1000,
        gid: 1000,
        pid: Some(12345),
        path: "/test/file.txt".to_string(),
        success: true,
        error_code: None,
        error_message: None,
        bytes_read: None,
        bytes_written: Some(1024),
        duration_ms: Some(50),
        text_changes: None,
        is_native_mount: false,
        native_source_path: None,
        metadata: None,
    };

    let log = audit_ops.create(input).await?;

    assert_eq!(log.tenant_id, tenant_id);
    assert_eq!(log.operation, "write");
    assert_eq!(log.path, "/test/file.txt");
    assert!(log.success);
    assert_eq!(log.bytes_written, Some(1024));

    Ok(())
}

#[tokio::test]
async fn test_audit_log_batch_create() -> Result<()> {
    let (pool, tenant_id) = setup_test_db().await?;
    let audit_ops = AuditLogOperations::new(pool.pool());

    let inputs = vec![
        CreateAuditLogInput {
            tenant_id,
            inode_id: None,
            operation: "read".to_string(),
            uid: 1000,
            gid: 1000,
            pid: None,
            path: "/file1.txt".to_string(),
            success: true,
            error_code: None,
            error_message: None,
            bytes_read: Some(512),
            bytes_written: None,
            duration_ms: Some(10),
            text_changes: None,
            is_native_mount: false,
            native_source_path: None,
            metadata: None,
        },
        CreateAuditLogInput {
            tenant_id,
            inode_id: None,
            operation: "write".to_string(),
            uid: 1000,
            gid: 1000,
            pid: None,
            path: "/file2.txt".to_string(),
            success: true,
            error_code: None,
            error_message: None,
            bytes_read: None,
            bytes_written: Some(2048),
            duration_ms: Some(25),
            text_changes: None,
            is_native_mount: false,
            native_source_path: None,
            metadata: None,
        },
        CreateAuditLogInput {
            tenant_id,
            inode_id: None,
            operation: "delete".to_string(),
            uid: 1000,
            gid: 1000,
            pid: None,
            path: "/file3.txt".to_string(),
            success: false,
            error_code: Some(2),
            error_message: Some("File not found".to_string()),
            bytes_read: None,
            bytes_written: None,
            duration_ms: Some(5),
            text_changes: None,
            is_native_mount: false,
            native_source_path: None,
            metadata: None,
        },
    ];

    let count = audit_ops.batch_create(inputs).await?;
    assert_eq!(count, 3);

    Ok(())
}

#[tokio::test]
async fn test_audit_log_query() -> Result<()> {
    let (pool, tenant_id) = setup_test_db().await?;
    let audit_ops = AuditLogOperations::new(pool.pool());

    // Create some test logs
    let inputs = vec![
        CreateAuditLogInput {
            tenant_id,
            inode_id: None,
            operation: "read".to_string(),
            uid: 1000,
            gid: 1000,
            pid: None,
            path: "/test1.txt".to_string(),
            success: true,
            error_code: None,
            error_message: None,
            bytes_read: Some(100),
            bytes_written: None,
            duration_ms: Some(10),
            text_changes: None,
            is_native_mount: false,
            native_source_path: None,
            metadata: None,
        },
        CreateAuditLogInput {
            tenant_id,
            inode_id: None,
            operation: "write".to_string(),
            uid: 1000,
            gid: 1000,
            pid: None,
            path: "/test2.txt".to_string(),
            success: true,
            error_code: None,
            error_message: None,
            bytes_read: None,
            bytes_written: Some(200),
            duration_ms: Some(20),
            text_changes: None,
            is_native_mount: false,
            native_source_path: None,
            metadata: None,
        },
    ];

    audit_ops.batch_create(inputs).await?;

    // Query all logs for this tenant
    let query = QueryAuditLogsInput {
        tenant_id,
        start_time: None,
        end_time: None,
        operation: None,
        uid: None,
        path_pattern: None,
        success: None,
        limit: Some(10),
    };

    let logs = audit_ops.query(query).await?;
    assert!(logs.len() >= 2);

    // Query only write operations
    let query_write = QueryAuditLogsInput {
        tenant_id,
        start_time: None,
        end_time: None,
        operation: Some("write".to_string()),
        uid: None,
        path_pattern: None,
        success: None,
        limit: Some(10),
    };

    let write_logs = audit_ops.query(query_write).await?;
    assert!(write_logs.iter().all(|log| log.operation == "write"));

    Ok(())
}

#[tokio::test]
async fn test_audit_log_aggregate_stats() -> Result<()> {
    let (pool, tenant_id) = setup_test_db().await?;
    let audit_ops = AuditLogOperations::new(pool.pool());

    let start_time = Utc::now();

    // Create test logs with various operations
    let inputs = vec![
        CreateAuditLogInput {
            tenant_id,
            inode_id: None,
            operation: "read".to_string(),
            uid: 1000,
            gid: 1000,
            pid: None,
            path: "/file1.txt".to_string(),
            success: true,
            error_code: None,
            error_message: None,
            bytes_read: Some(1000),
            bytes_written: None,
            duration_ms: Some(10),
            text_changes: None,
            is_native_mount: false,
            native_source_path: None,
            metadata: None,
        },
        CreateAuditLogInput {
            tenant_id,
            inode_id: None,
            operation: "write".to_string(),
            uid: 1000,
            gid: 1000,
            pid: None,
            path: "/file2.txt".to_string(),
            success: true,
            error_code: None,
            error_message: None,
            bytes_read: None,
            bytes_written: Some(2000),
            duration_ms: Some(20),
            text_changes: None,
            is_native_mount: false,
            native_source_path: None,
            metadata: None,
        },
        CreateAuditLogInput {
            tenant_id,
            inode_id: None,
            operation: "delete".to_string(),
            uid: 1000,
            gid: 1000,
            pid: None,
            path: "/file3.txt".to_string(),
            success: false,
            error_code: Some(1),
            error_message: Some("Permission denied".to_string()),
            bytes_read: None,
            bytes_written: None,
            duration_ms: Some(5),
            text_changes: None,
            is_native_mount: false,
            native_source_path: None,
            metadata: None,
        },
    ];

    audit_ops.batch_create(inputs).await?;

    let end_time = Utc::now();

    // Get statistics
    let stats = audit_ops.aggregate_stats(tenant_id, start_time, end_time).await?;

    assert!(stats.total_operations >= 3);
    assert!(stats.successful_operations >= 2);
    assert!(stats.failed_operations >= 1);
    assert!(stats.total_bytes_read >= 1000);
    assert!(stats.total_bytes_written >= 2000);
    assert!(stats.avg_duration_ms.is_some());

    Ok(())
}

#[tokio::test]
async fn test_audit_log_query_with_filters() -> Result<()> {
    let (pool, tenant_id) = setup_test_db().await?;
    let audit_ops = AuditLogOperations::new(pool.pool());

    // Create test logs
    let inputs = vec![
        CreateAuditLogInput {
            tenant_id,
            inode_id: None,
            operation: "read".to_string(),
            uid: 1000,
            gid: 1000,
            pid: None,
            path: "/success.txt".to_string(),
            success: true,
            error_code: None,
            error_message: None,
            bytes_read: Some(100),
            bytes_written: None,
            duration_ms: Some(10),
            text_changes: None,
            is_native_mount: false,
            native_source_path: None,
            metadata: None,
        },
        CreateAuditLogInput {
            tenant_id,
            inode_id: None,
            operation: "write".to_string(),
            uid: 2000,
            gid: 2000,
            pid: None,
            path: "/failed.txt".to_string(),
            success: false,
            error_code: Some(13),
            error_message: Some("Permission denied".to_string()),
            bytes_read: None,
            bytes_written: None,
            duration_ms: Some(5),
            text_changes: None,
            is_native_mount: false,
            native_source_path: None,
            metadata: None,
        },
    ];

    audit_ops.batch_create(inputs).await?;

    // Query only successful operations
    let query_success = QueryAuditLogsInput {
        tenant_id,
        start_time: None,
        end_time: None,
        operation: None,
        uid: None,
        path_pattern: None,
        success: Some(true),
        limit: Some(10),
    };

    let success_logs = audit_ops.query(query_success).await?;
    assert!(success_logs.iter().all(|log| log.success));

    // Query only failed operations
    let query_failed = QueryAuditLogsInput {
        tenant_id,
        start_time: None,
        end_time: None,
        operation: None,
        uid: None,
        path_pattern: None,
        success: Some(false),
        limit: Some(10),
    };

    let failed_logs = audit_ops.query(query_failed).await?;
    assert!(failed_logs.iter().all(|log| !log.success));

    // Query by user
    let query_user = QueryAuditLogsInput {
        tenant_id,
        start_time: None,
        end_time: None,
        operation: None,
        uid: Some(1000),
        path_pattern: None,
        success: None,
        limit: Some(10),
    };

    let user_logs = audit_ops.query(query_user).await?;
    assert!(user_logs.iter().all(|log| log.uid == 1000));

    Ok(())
}

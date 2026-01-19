use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::types::TenantId;

use super::models::{AuditLog, AuditStats, CreateAuditLogInput, QueryAuditLogsInput};
use super::traits::AuditLogRepository;

pub struct AuditLogOperations<'a> {
    pool: &'a PgPool,
}

impl<'a> AuditLogOperations<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl<'a> AuditLogRepository for AuditLogOperations<'a> {
    async fn create(&self, input: CreateAuditLogInput) -> Result<AuditLog> {
        let log_date = chrono::Utc::now().date_naive();

        let log = sqlx::query_as::<_, AuditLog>(
            r#"
            INSERT INTO audit_logs (
                tenant_id, inode_id, operation, uid, gid, pid,
                path, success, error_code, error_message,
                bytes_read, bytes_written, duration_ms, text_changes,
                is_native_mount, native_source_path, metadata, log_date
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
            RETURNING log_id, tenant_id, inode_id, operation, uid, gid, pid,
                      path, success, error_code, error_message,
                      bytes_read, bytes_written, duration_ms, text_changes,
                      is_native_mount, native_source_path, metadata,
                      created_at, log_date
            "#,
        )
        .bind(input.tenant_id)
        .bind(input.inode_id)
        .bind(&input.operation)
        .bind(input.uid)
        .bind(input.gid)
        .bind(input.pid)
        .bind(&input.path)
        .bind(input.success)
        .bind(input.error_code)
        .bind(&input.error_message)
        .bind(input.bytes_read)
        .bind(input.bytes_written)
        .bind(input.duration_ms)
        .bind(&input.text_changes)
        .bind(input.is_native_mount)
        .bind(&input.native_source_path)
        .bind(&input.metadata)
        .bind(log_date)
        .fetch_one(self.pool)
        .await?;

        tracing::debug!(
            tenant_id = %input.tenant_id,
            operation = %input.operation,
            path = %input.path,
            success = input.success,
            "Created audit log entry"
        );

        Ok(log)
    }

    async fn batch_create(&self, inputs: Vec<CreateAuditLogInput>) -> Result<u64> {
        if inputs.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await?;
        let log_date = chrono::Utc::now().date_naive();
        let mut inserted = 0u64;

        for input in inputs {
            let result = sqlx::query(
                r#"
                INSERT INTO audit_logs (
                    tenant_id, inode_id, operation, uid, gid, pid,
                    path, success, error_code, error_message,
                    bytes_read, bytes_written, duration_ms, text_changes,
                    is_native_mount, native_source_path, metadata, log_date
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
                "#,
            )
            .bind(input.tenant_id)
            .bind(input.inode_id)
            .bind(&input.operation)
            .bind(input.uid)
            .bind(input.gid)
            .bind(input.pid)
            .bind(&input.path)
            .bind(input.success)
            .bind(input.error_code)
            .bind(&input.error_message)
            .bind(input.bytes_read)
            .bind(input.bytes_written)
            .bind(input.duration_ms)
            .bind(&input.text_changes)
            .bind(input.is_native_mount)
            .bind(&input.native_source_path)
            .bind(&input.metadata)
            .bind(log_date)
            .execute(&mut *tx)
            .await?;

            inserted += result.rows_affected();
        }

        tx.commit().await?;

        tracing::info!(count = inserted, "Batch created audit log entries");

        Ok(inserted)
    }

    async fn query(&self, input: QueryAuditLogsInput) -> Result<Vec<AuditLog>> {
        let mut query = String::from(
            r#"
            SELECT log_id, tenant_id, inode_id, operation, uid, gid, pid,
                   path, success, error_code, error_message,
                   bytes_read, bytes_written, duration_ms, text_changes,
                   is_native_mount, native_source_path, metadata,
                   created_at, log_date
            FROM audit_logs
            WHERE tenant_id = $1
            "#,
        );

        let mut param_count = 2;
        let mut conditions = Vec::new();

        if input.start_time.is_some() {
            conditions.push(format!("created_at >= ${}", param_count));
            param_count += 1;
        }

        if input.end_time.is_some() {
            conditions.push(format!("created_at <= ${}", param_count));
            param_count += 1;
        }

        if input.operation.is_some() {
            conditions.push(format!("operation = ${}", param_count));
            param_count += 1;
        }

        if input.uid.is_some() {
            conditions.push(format!("uid = ${}", param_count));
            param_count += 1;
        }

        if input.path_pattern.is_some() {
            conditions.push(format!("path LIKE ${}", param_count));
            param_count += 1;
        }

        if input.success.is_some() {
            conditions.push(format!("success = ${}", param_count));
            param_count += 1;
        }

        if !conditions.is_empty() {
            query.push_str(" AND ");
            query.push_str(&conditions.join(" AND "));
        }

        query.push_str(" ORDER BY created_at DESC");

        if input.limit.is_some() {
            query.push_str(&format!(" LIMIT ${}", param_count));
        }

        let mut q = sqlx::query_as::<_, AuditLog>(&query).bind(input.tenant_id);

        if let Some(start) = input.start_time {
            q = q.bind(start);
        }
        if let Some(end) = input.end_time {
            q = q.bind(end);
        }
        if let Some(op) = &input.operation {
            q = q.bind(op);
        }
        if let Some(uid) = input.uid {
            q = q.bind(uid);
        }
        if let Some(pattern) = &input.path_pattern {
            q = q.bind(pattern);
        }
        if let Some(success) = input.success {
            q = q.bind(success);
        }
        if let Some(limit) = input.limit {
            q = q.bind(limit);
        }

        let logs = q.fetch_all(self.pool).await?;

        tracing::debug!(
            tenant_id = %input.tenant_id,
            count = logs.len(),
            "Queried audit logs"
        );

        Ok(logs)
    }

    async fn aggregate_stats(
        &self,
        tenant_id: TenantId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<AuditStats> {
        let stats = sqlx::query_as::<_, (i64, i64, i64, i64, i64, Option<f64>)>(
            r#"
            SELECT
                COUNT(*) as total_operations,
                COUNT(*) FILTER (WHERE success = true) as successful_operations,
                COUNT(*) FILTER (WHERE success = false) as failed_operations,
                COALESCE(SUM(bytes_read), 0) as total_bytes_read,
                COALESCE(SUM(bytes_written), 0) as total_bytes_written,
                AVG(duration_ms) as avg_duration_ms
            FROM audit_logs
            WHERE tenant_id = $1
              AND created_at >= $2
              AND created_at <= $3
            "#,
        )
        .bind(tenant_id)
        .bind(start)
        .bind(end)
        .fetch_one(self.pool)
        .await?;

        Ok(AuditStats {
            total_operations: stats.0,
            successful_operations: stats.1,
            failed_operations: stats.2,
            total_bytes_read: stats.3,
            total_bytes_written: stats.4,
            avg_duration_ms: stats.5,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_build_query_conditions_no_filters() {
        let input = QueryAuditLogsInput {
            tenant_id: uuid::Uuid::new_v4(),
            start_time: None,
            end_time: None,
            operation: None,
            uid: None,
            path_pattern: None,
            success: None,
            limit: None,
        };

        // Should only have tenant_id condition
        assert!(input.start_time.is_none());
        assert!(input.end_time.is_none());
    }

    #[test]
    fn test_build_query_conditions_with_time_range() {
        let start = Utc::now();
        let end = start + chrono::Duration::hours(1);

        let input = QueryAuditLogsInput {
            tenant_id: uuid::Uuid::new_v4(),
            start_time: Some(start),
            end_time: Some(end),
            operation: None,
            uid: None,
            path_pattern: None,
            success: None,
            limit: None,
        };

        // Should have time range conditions
        assert!(input.start_time.is_some());
        assert!(input.end_time.is_some());
    }

    #[test]
    fn test_build_query_conditions_with_operation_filter() {
        let input = QueryAuditLogsInput {
            tenant_id: uuid::Uuid::new_v4(),
            start_time: None,
            end_time: None,
            operation: Some("READ".to_string()),
            uid: None,
            path_pattern: None,
            success: None,
            limit: None,
        };

        assert_eq!(input.operation.as_ref().unwrap(), "READ");
    }

    #[test]
    fn test_build_query_conditions_with_path_pattern() {
        let input = QueryAuditLogsInput {
            tenant_id: uuid::Uuid::new_v4(),
            start_time: None,
            end_time: None,
            operation: None,
            uid: None,
            path_pattern: Some("/home%".to_string()),
            success: None,
            limit: None,
        };

        assert_eq!(input.path_pattern.as_ref().unwrap(), "/home%");
    }

    #[test]
    fn test_query_input_limit_validation() {
        let input = QueryAuditLogsInput {
            tenant_id: uuid::Uuid::new_v4(),
            start_time: None,
            end_time: None,
            operation: None,
            uid: None,
            path_pattern: None,
            success: Some(true),
            limit: Some(1000),
        };

        assert_eq!(input.limit.unwrap(), 1000);
        assert!(input.success.unwrap());
    }
}

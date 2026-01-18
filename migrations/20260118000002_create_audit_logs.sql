-- Migration 002: Audit Logging System
-- Creates audit_logs table with partitioning support for operation auditing

-- Create audit_logs parent table (partitioned by log_date)
CREATE TABLE audit_logs (
    log_id BIGSERIAL,
    tenant_id UUID NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    inode_id BIGINT,
    operation VARCHAR(50) NOT NULL,
    uid INTEGER NOT NULL,
    gid INTEGER NOT NULL,
    pid INTEGER,
    path TEXT NOT NULL,
    success BOOLEAN NOT NULL DEFAULT true,
    error_code INTEGER,
    error_message TEXT,
    bytes_read BIGINT,
    bytes_written BIGINT,
    duration_ms INTEGER,
    text_changes JSONB,
    is_native_mount BOOLEAN NOT NULL DEFAULT false,
    native_source_path TEXT,
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    log_date DATE NOT NULL DEFAULT CURRENT_DATE,

    PRIMARY KEY (tenant_id, log_id, log_date),
    FOREIGN KEY (tenant_id, inode_id) REFERENCES inodes(tenant_id, inode_id) ON DELETE SET NULL
) PARTITION BY RANGE (log_date);

-- Create indexes on parent table
CREATE INDEX idx_audit_tenant_created ON audit_logs(tenant_id, created_at);
CREATE INDEX idx_audit_tenant_operation ON audit_logs(tenant_id, operation, created_at);
CREATE INDEX idx_audit_tenant_user ON audit_logs(tenant_id, uid, created_at);
CREATE INDEX idx_audit_tenant_path ON audit_logs(tenant_id, path);
CREATE INDEX idx_audit_success ON audit_logs(success) WHERE success = false;

-- Create initial partitions (current month + next 2 months)
CREATE TABLE audit_logs_2026_01 PARTITION OF audit_logs
    FOR VALUES FROM ('2026-01-01') TO ('2026-02-01');

CREATE TABLE audit_logs_2026_02 PARTITION OF audit_logs
    FOR VALUES FROM ('2026-02-01') TO ('2026-03-01');

CREATE TABLE audit_logs_2026_03 PARTITION OF audit_logs
    FOR VALUES FROM ('2026-03-01') TO ('2026-04-01');

-- Function to create new audit log partitions
CREATE OR REPLACE FUNCTION create_audit_log_partition(start_date DATE)
RETURNS TEXT AS $$
DECLARE
    partition_name TEXT;
    end_date DATE;
BEGIN
    partition_name := 'audit_logs_' || to_char(start_date, 'YYYY_MM');
    end_date := start_date + INTERVAL '1 month';

    -- Check if partition already exists
    IF EXISTS (
        SELECT 1 FROM pg_tables
        WHERE tablename = partition_name
    ) THEN
        RETURN 'Partition ' || partition_name || ' already exists';
    END IF;

    -- Create partition
    EXECUTE format(
        'CREATE TABLE %I PARTITION OF audit_logs FOR VALUES FROM (%L) TO (%L)',
        partition_name,
        start_date,
        end_date
    );

    RETURN 'Created partition ' || partition_name;
END;
$$ LANGUAGE plpgsql;

-- Function to clean up old audit log partitions
CREATE OR REPLACE FUNCTION cleanup_old_audit_partitions(retention_days INTEGER DEFAULT 90)
RETURNS TEXT AS $$
DECLARE
    partition_record RECORD;
    cutoff_date DATE;
    dropped_count INTEGER := 0;
BEGIN
    cutoff_date := CURRENT_DATE - retention_days;

    FOR partition_record IN
        SELECT tablename
        FROM pg_tables
        WHERE schemaname = 'public'
          AND tablename LIKE 'audit_logs_%'
          AND tablename ~ '^\d{4}_\d{2}$'
    LOOP
        -- Extract date from partition name and check if it's old enough to drop
        DECLARE
            partition_date DATE;
        BEGIN
            partition_date := to_date(
                substring(partition_record.tablename from 'audit_logs_(\d{4}_\d{2})'),
                'YYYY_MM'
            );

            IF partition_date < cutoff_date THEN
                EXECUTE 'DROP TABLE IF EXISTS ' || partition_record.tablename;
                dropped_count := dropped_count + 1;
                RAISE NOTICE 'Dropped old partition: %', partition_record.tablename;
            END IF;
        EXCEPTION WHEN OTHERS THEN
            RAISE WARNING 'Failed to drop partition %: %', partition_record.tablename, SQLERRM;
        END;
    END LOOP;

    RETURN format('Dropped %s old partition(s)', dropped_count);
END;
$$ LANGUAGE plpgsql;

-- Comments for documentation
COMMENT ON TABLE audit_logs IS 'Audit log for all filesystem operations, partitioned by date';
COMMENT ON COLUMN audit_logs.log_id IS 'Auto-incrementing log entry ID';
COMMENT ON COLUMN audit_logs.tenant_id IS 'Tenant who performed the operation';
COMMENT ON COLUMN audit_logs.inode_id IS 'Target inode (nullable for tenant-level operations)';
COMMENT ON COLUMN audit_logs.operation IS 'Operation type (read, write, mkdir, etc.)';
COMMENT ON COLUMN audit_logs.uid IS 'User ID who performed the operation';
COMMENT ON COLUMN audit_logs.gid IS 'Group ID of the user';
COMMENT ON COLUMN audit_logs.pid IS 'Process ID (nullable)';
COMMENT ON COLUMN audit_logs.path IS 'File path of the operation';
COMMENT ON COLUMN audit_logs.success IS 'Whether the operation succeeded';
COMMENT ON COLUMN audit_logs.error_code IS 'Error code if operation failed';
COMMENT ON COLUMN audit_logs.error_message IS 'Error message if operation failed';
COMMENT ON COLUMN audit_logs.bytes_read IS 'Number of bytes read (for read operations)';
COMMENT ON COLUMN audit_logs.bytes_written IS 'Number of bytes written (for write operations)';
COMMENT ON COLUMN audit_logs.duration_ms IS 'Operation duration in milliseconds';
COMMENT ON COLUMN audit_logs.text_changes IS 'JSON details of text file changes (lines added/deleted/modified)';
COMMENT ON COLUMN audit_logs.is_native_mount IS 'Whether this operation was on a native mount';
COMMENT ON COLUMN audit_logs.native_source_path IS 'Source path if native mount operation';
COMMENT ON COLUMN audit_logs.metadata IS 'Additional metadata (layer_id, session_id, etc.)';
COMMENT ON COLUMN audit_logs.created_at IS 'Timestamp when the operation occurred';
COMMENT ON COLUMN audit_logs.log_date IS 'Date for partitioning (derived from created_at)';

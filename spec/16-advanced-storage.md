# 数据库设计规范 - 高级存储特性

> **基于**: [spec/01-database-schema.md](01-database-schema.md) - 核心 MVP Schema
>
> 本文档描述 Tarbox 的高级存储特性，包括分层文件系统、文本优化、审计日志和原生挂载。
> 
> MVP 核心表（tenants, inodes, blocks）的设计见 [spec/01](01-database-schema.md)。

## 概述

高级存储特性在 MVP 核心之上提供：
- **分层文件系统**: Docker 风格的 COW 层
- **文本文件优化**: 行级存储和跨层去重
- **审计日志**: 完整的操作审计和合规报告
- **原生挂载**: 性能优化的原生 FS 直通
- **快照管理**: 时间点恢复

## 分层文件系统

### 1. layers 表（层元数据）

```sql
CREATE TABLE layers (
    layer_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    
    -- 层关系
    parent_layer_id UUID REFERENCES layers(layer_id) ON DELETE RESTRICT,
    
    -- 层信息
    layer_name VARCHAR(255),
    description TEXT,
    
    -- 统计信息
    file_count INTEGER NOT NULL DEFAULT 0,
    total_size BIGINT NOT NULL DEFAULT 0,
    
    -- 状态
    status VARCHAR(20) NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'creating', 'deleting', 'archived')),
    is_readonly BOOLEAN NOT NULL DEFAULT FALSE,
    
    -- 标签
    tags JSONB,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_by VARCHAR(255),
    
    -- 确保租户内层名唯一
    UNIQUE(tenant_id, layer_name)
);

CREATE INDEX idx_layers_tenant ON layers(tenant_id, created_at);
CREATE INDEX idx_layers_parent ON layers(parent_layer_id);
CREATE INDEX idx_layers_status ON layers(status) WHERE status = 'active';

COMMENT ON TABLE layers IS '分层文件系统的层元数据';
COMMENT ON COLUMN layers.parent_layer_id IS '父层 ID，形成单向链表';
COMMENT ON COLUMN layers.is_readonly IS '只读层不能修改，适合作为基础镜像';
```

### 2. layer_entries 表（层条目）

```sql
CREATE TABLE layer_entries (
    entry_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    layer_id UUID NOT NULL REFERENCES layers(layer_id) ON DELETE CASCADE,
    tenant_id UUID NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    inode_id BIGINT NOT NULL,
    
    -- 路径信息
    path TEXT NOT NULL,
    
    -- 变更类型
    change_type VARCHAR(10) NOT NULL CHECK (change_type IN ('add', 'modify', 'delete')),
    
    -- 差异信息
    size_delta BIGINT,                       -- 大小变化
    
    -- 文本文件差异
    text_changes JSONB,                      -- { "lines_added": 10, "lines_deleted": 5, "chunks": [...] }
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (tenant_id, inode_id) REFERENCES inodes(tenant_id, inode_id) ON DELETE CASCADE,
    UNIQUE(layer_id, path)
);

CREATE INDEX idx_layer_entries_layer ON layer_entries(layer_id, change_type);
CREATE INDEX idx_layer_entries_inode ON layer_entries(tenant_id, inode_id);
CREATE INDEX idx_layer_entries_path ON layer_entries(path);

COMMENT ON TABLE layer_entries IS '层中的文件变更记录';
COMMENT ON COLUMN layer_entries.change_type IS 'add: 新增, modify: 修改, delete: 删除标记';
COMMENT ON COLUMN layer_entries.text_changes IS '文本文件的详细变更信息';
```

### 3. tenant_current_layer 表（租户当前层）

```sql
CREATE TABLE tenant_current_layer (
    tenant_id UUID PRIMARY KEY REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    current_layer_id UUID NOT NULL REFERENCES layers(layer_id) ON DELETE RESTRICT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_tenant_current_layer_layer ON tenant_current_layer(current_layer_id);

COMMENT ON TABLE tenant_current_layer IS '记录每个租户当前工作的层';
```

## 文本文件优化

### 4. text_blocks 表（文本块存储）

```sql
CREATE TABLE text_blocks (
    block_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    content_hash VARCHAR(64) NOT NULL UNIQUE,
    content TEXT NOT NULL,
    line_count INTEGER NOT NULL,
    byte_size INTEGER NOT NULL,
    encoding VARCHAR(20) NOT NULL DEFAULT 'UTF-8',
    ref_count INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    last_accessed_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_text_blocks_hash ON text_blocks(content_hash);
CREATE INDEX idx_text_blocks_ref_count ON text_blocks(ref_count);
CREATE INDEX idx_text_blocks_last_accessed ON text_blocks(last_accessed_at);

COMMENT ON TABLE text_blocks IS '文本文件内容块存储，支持跨文件和跨层去重';
COMMENT ON COLUMN text_blocks.content_hash IS 'BLAKE3 哈希，用于内容去重';
COMMENT ON COLUMN text_blocks.ref_count IS '引用计数，为 0 时可以清理';
```

### 5. text_file_metadata 表（文本文件元数据）

```sql
CREATE TABLE text_file_metadata (
    tenant_id UUID NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    inode_id BIGINT NOT NULL,
    layer_id UUID NOT NULL REFERENCES layers(layer_id) ON DELETE CASCADE,
    total_lines INTEGER NOT NULL,
    encoding VARCHAR(20) NOT NULL DEFAULT 'UTF-8',
    line_ending VARCHAR(10) NOT NULL DEFAULT 'LF' CHECK (line_ending IN ('LF', 'CRLF', 'CR')),
    has_trailing_newline BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    PRIMARY KEY (tenant_id, inode_id, layer_id),
    FOREIGN KEY (tenant_id, inode_id) REFERENCES inodes(tenant_id, inode_id) ON DELETE CASCADE
);

CREATE INDEX idx_text_file_metadata_layer ON text_file_metadata(layer_id);
CREATE INDEX idx_text_file_metadata_inode ON text_file_metadata(tenant_id, inode_id);

COMMENT ON TABLE text_file_metadata IS '文本文件的元数据信息';
COMMENT ON COLUMN text_file_metadata.line_ending IS '行结束符类型：LF (Unix), CRLF (Windows), CR (旧Mac)';
```

### 6. text_line_map 表（文本行映射）

```sql
CREATE TABLE text_line_map (
    tenant_id UUID NOT NULL,
    inode_id BIGINT NOT NULL,
    layer_id UUID NOT NULL,
    line_number INTEGER NOT NULL,
    block_id UUID NOT NULL REFERENCES text_blocks(block_id) ON DELETE RESTRICT,
    block_line_offset INTEGER NOT NULL,
    PRIMARY KEY (tenant_id, inode_id, layer_id, line_number),
    FOREIGN KEY (tenant_id, inode_id, layer_id) 
        REFERENCES text_file_metadata(tenant_id, inode_id, layer_id) ON DELETE CASCADE
);

CREATE INDEX idx_text_line_map_lookup 
    ON text_line_map(tenant_id, inode_id, layer_id, line_number);
CREATE INDEX idx_text_line_map_block 
    ON text_line_map(block_id);

COMMENT ON TABLE text_line_map IS '文本文件的行到 TextBlock 的映射';
COMMENT ON COLUMN text_line_map.line_number IS '逻辑行号（从 1 开始）';
COMMENT ON COLUMN text_line_map.block_line_offset IS '在 TextBlock 内的行偏移（从 0 开始）';
```

## 审计日志

### 7. audit_logs 表（审计日志）

```sql
CREATE TABLE audit_logs (
    log_id BIGSERIAL,
    tenant_id UUID NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    inode_id BIGINT,
    operation VARCHAR(50) NOT NULL,          -- 操作类型（read, write, mkdir, etc.）
    
    -- 用户信息
    uid INTEGER NOT NULL,
    gid INTEGER NOT NULL,
    pid INTEGER,                             -- 进程 ID
    
    -- 操作详情
    path TEXT,                               -- 文件路径
    success BOOLEAN NOT NULL,                -- 操作是否成功
    error_code INTEGER,                      -- 错误码
    error_message TEXT,                      -- 错误信息
    
    -- 性能数据
    bytes_read BIGINT,
    bytes_written BIGINT,
    duration_ms INTEGER,                     -- 操作耗时（毫秒）
    
    -- 文本文件变更
    text_changes JSONB,                      -- 文本文件的变更详情
    
    -- 原生挂载相关
    is_native_mount BOOLEAN DEFAULT false,   -- 是否为原生挂载操作
    native_source_path TEXT,                 -- 原生文件系统路径
    
    -- 附加信息
    metadata JSONB,                          -- 额外的元数据
    
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    
    -- 分区键
    log_date DATE NOT NULL DEFAULT CURRENT_DATE,
    
    PRIMARY KEY (tenant_id, log_id, log_date),
    FOREIGN KEY (tenant_id, inode_id) REFERENCES inodes(tenant_id, inode_id) ON DELETE SET NULL
) PARTITION BY RANGE (log_date);

-- 创建分区（示例：2026年1月）
CREATE TABLE audit_logs_2026_01 PARTITION OF audit_logs
    FOR VALUES FROM ('2026-01-01') TO ('2026-02-01');

-- 索引
CREATE INDEX idx_audit_tenant_created ON audit_logs(tenant_id, created_at);
CREATE INDEX idx_audit_tenant_inode ON audit_logs(tenant_id, inode_id, created_at);
CREATE INDEX idx_audit_tenant_operation ON audit_logs(tenant_id, operation, created_at);
CREATE INDEX idx_audit_tenant_user ON audit_logs(tenant_id, uid, created_at);
CREATE INDEX idx_audit_tenant_path ON audit_logs(tenant_id, path, created_at);

COMMENT ON TABLE audit_logs IS '操作审计日志，按日期分区';
COMMENT ON COLUMN audit_logs.text_changes IS '文本文件的 diff 摘要：{ "lines_added": 10, "lines_deleted": 5 }';
COMMENT ON COLUMN audit_logs.is_native_mount IS '标记操作是否通过原生挂载透传';
```

### 审计日志分区管理

```sql
-- 自动创建分区函数
CREATE OR REPLACE FUNCTION create_audit_log_partitions()
RETURNS void AS $$
DECLARE
    start_date DATE;
    end_date DATE;
    partition_name TEXT;
BEGIN
    -- 为未来3个月创建分区
    FOR i IN 0..2 LOOP
        start_date := DATE_TRUNC('month', CURRENT_DATE + (i || ' months')::INTERVAL);
        end_date := start_date + INTERVAL '1 month';
        
        partition_name := 'audit_logs_' || TO_CHAR(start_date, 'YYYY_MM');
        
        EXECUTE format(
            'CREATE TABLE IF NOT EXISTS %I PARTITION OF audit_logs FOR VALUES FROM (%L) TO (%L)',
            partition_name, start_date, end_date
        );
        
        RAISE NOTICE 'Created partition: %', partition_name;
    END LOOP;
END;
$$ LANGUAGE plpgsql;

-- 清理过期分区函数
CREATE OR REPLACE FUNCTION drop_old_audit_log_partitions(retention_days INTEGER)
RETURNS void AS $$
DECLARE
    partition record;
    cutoff_date DATE;
BEGIN
    cutoff_date := CURRENT_DATE - retention_days;
    
    FOR partition IN
        SELECT tablename 
        FROM pg_tables
        WHERE schemaname = 'public'
        AND tablename LIKE 'audit_logs_%'
        AND tablename < 'audit_logs_' || TO_CHAR(cutoff_date, 'YYYY_MM')
    LOOP
        EXECUTE format('DROP TABLE IF EXISTS %I', partition.tablename);
        RAISE NOTICE 'Dropped partition: %', partition.tablename;
    END LOOP;
END;
$$ LANGUAGE plpgsql;

-- 定期任务（需要 pg_cron 扩展）
-- SELECT cron.schedule('create-audit-partitions', '0 0 1 * *', 'SELECT create_audit_log_partitions()');
-- SELECT cron.schedule('cleanup-audit-partitions', '0 2 1 * *', 'SELECT drop_old_audit_log_partitions(365)');
```

## 原生挂载

### 8. native_mounts 表（原生文件系统挂载）

```sql
CREATE TABLE native_mounts (
    mount_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- 挂载路径（虚拟路径）
    mount_path TEXT NOT NULL,
    
    -- 原生系统路径
    source_path TEXT NOT NULL,
    
    -- 访问模式
    mode VARCHAR(2) NOT NULL CHECK (mode IN ('ro', 'rw')),
    
    -- 是否跨租户共享
    is_shared BOOLEAN NOT NULL DEFAULT false,
    
    -- 如果不是 shared，可以指定特定租户
    tenant_id UUID REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    
    -- 启用状态
    enabled BOOLEAN NOT NULL DEFAULT true,
    
    -- 优先级（用于路径匹配，数字越小优先级越高）
    priority INTEGER NOT NULL DEFAULT 100,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    
    -- 确保路径唯一性
    UNIQUE(mount_path, tenant_id),
    
    -- 确保共享挂载没有 tenant_id
    CHECK (NOT is_shared OR tenant_id IS NULL)
);

CREATE INDEX idx_native_mounts_path ON native_mounts(mount_path) WHERE enabled = true;
CREATE INDEX idx_native_mounts_tenant ON native_mounts(tenant_id) WHERE enabled = true;
CREATE INDEX idx_native_mounts_priority ON native_mounts(priority, mount_path) WHERE enabled = true;

COMMENT ON TABLE native_mounts IS '原生文件系统挂载配置';
COMMENT ON COLUMN native_mounts.mount_path IS '在 Tarbox 中的虚拟路径';
COMMENT ON COLUMN native_mounts.source_path IS '宿主机的实际路径，支持变量 {tenant_id}';
COMMENT ON COLUMN native_mounts.is_shared IS '是否跨租户共享（如系统目录）';
COMMENT ON COLUMN native_mounts.priority IS '路径匹配优先级，越小越优先';
```

## 快照管理

### 9. snapshots 表（快照）

```sql
CREATE TABLE snapshots (
    snapshot_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    layer_id UUID NOT NULL REFERENCES layers(layer_id) ON DELETE RESTRICT,
    
    name VARCHAR(255) NOT NULL,
    description TEXT,
    
    -- 快照范围
    root_inode_id BIGINT NOT NULL,
    
    -- 快照元数据
    inode_count BIGINT NOT NULL,
    total_size BIGINT NOT NULL,
    
    -- 状态
    status VARCHAR(20) NOT NULL CHECK (status IN ('creating', 'ready', 'deleting', 'failed')),
    
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    created_by VARCHAR(255),
    expires_at TIMESTAMP,                    -- 过期时间（可选）
    
    -- 快照数据（存储层状态）
    metadata JSONB NOT NULL,
    
    UNIQUE(tenant_id, name),
    FOREIGN KEY (tenant_id, root_inode_id) REFERENCES inodes(tenant_id, inode_id) ON DELETE RESTRICT
);

CREATE INDEX idx_snapshots_tenant ON snapshots(tenant_id, created_at);
CREATE INDEX idx_snapshots_layer ON snapshots(layer_id);
CREATE INDEX idx_snapshots_status ON snapshots(status);
CREATE INDEX idx_snapshots_expires ON snapshots(expires_at) WHERE expires_at IS NOT NULL;

COMMENT ON TABLE snapshots IS '文件系统快照';
COMMENT ON COLUMN snapshots.layer_id IS '快照关联的层 ID';
COMMENT ON COLUMN snapshots.metadata IS '快照时的层链状态';
```

## 统计信息

### 10. statistics 表（统计信息）

```sql
CREATE TABLE statistics (
    stat_id BIGSERIAL,
    tenant_id UUID NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    
    metric_name VARCHAR(100) NOT NULL,
    metric_value BIGINT NOT NULL,
    
    -- 维度
    layer_id UUID REFERENCES layers(layer_id) ON DELETE CASCADE,
    inode_id BIGINT,
    
    -- 标签
    labels JSONB,
    
    recorded_at TIMESTAMP NOT NULL DEFAULT NOW(),
    
    -- 时间分区
    stat_date DATE NOT NULL DEFAULT CURRENT_DATE,
    
    PRIMARY KEY (tenant_id, stat_id, stat_date),
    FOREIGN KEY (tenant_id, inode_id) REFERENCES inodes(tenant_id, inode_id) ON DELETE SET NULL
) PARTITION BY RANGE (stat_date);

-- 创建分区（示例）
CREATE TABLE statistics_2026_01 PARTITION OF statistics
    FOR VALUES FROM ('2026-01-01') TO ('2026-02-01');

-- 索引
CREATE INDEX idx_statistics_tenant_metric ON statistics(tenant_id, metric_name, recorded_at);
CREATE INDEX idx_statistics_tenant_layer ON statistics(tenant_id, layer_id, recorded_at);

COMMENT ON TABLE statistics IS '性能和使用统计信息';
COMMENT ON COLUMN statistics.metric_name IS '指标名称：iops_read, iops_write, bytes_read, etc.';
```

## 索引优化策略

### 复合索引

基于常见查询模式的复合索引：

```sql
-- 层链遍历优化
CREATE INDEX idx_layers_tenant_parent_status 
    ON layers(tenant_id, parent_layer_id, status) 
    WHERE status = 'active';

-- 文本行快速查找
CREATE INDEX idx_text_line_map_fast_lookup 
    ON text_line_map(tenant_id, inode_id, layer_id) 
    INCLUDE (line_number, block_id);

-- 审计日志常见查询
CREATE INDEX idx_audit_tenant_time_op 
    ON audit_logs(tenant_id, created_at DESC, operation);

-- 原生挂载路径匹配
CREATE INDEX idx_native_mounts_path_priority 
    ON native_mounts(mount_path, priority) 
    WHERE enabled = true;
```

### 部分索引

针对常见过滤条件的部分索引：

```sql
-- 只索引活跃的层
CREATE INDEX idx_layers_active 
    ON layers(tenant_id, created_at) 
    WHERE status = 'active';

-- 只索引最近的审计日志（最近30天）
CREATE INDEX idx_audit_recent 
    ON audit_logs(tenant_id, created_at) 
    WHERE created_at > NOW() - INTERVAL '30 days';

-- 只索引有引用的文本块
CREATE INDEX idx_text_blocks_referenced 
    ON text_blocks(block_id, content_hash) 
    WHERE ref_count > 0;
```

## 触发器和约束

### 引用计数管理

```sql
-- TextBlock 引用计数自动更新
CREATE OR REPLACE FUNCTION update_text_block_refcount()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        UPDATE text_blocks SET ref_count = ref_count + 1 WHERE block_id = NEW.block_id;
    ELSIF TG_OP = 'DELETE' THEN
        UPDATE text_blocks SET ref_count = ref_count - 1 WHERE block_id = OLD.block_id;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_text_line_map_refcount
AFTER INSERT OR DELETE ON text_line_map
FOR EACH ROW EXECUTE FUNCTION update_text_block_refcount();
```

### 层统计自动更新

```sql
-- 层统计信息自动更新
CREATE OR REPLACE FUNCTION update_layer_stats()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        UPDATE layers 
        SET file_count = file_count + 1,
            total_size = total_size + COALESCE(NEW.size_delta, 0)
        WHERE layer_id = NEW.layer_id;
    ELSIF TG_OP = 'DELETE' THEN
        UPDATE layers 
        SET file_count = file_count - 1,
            total_size = total_size - COALESCE(OLD.size_delta, 0)
        WHERE layer_id = OLD.layer_id;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_layer_entries_stats
AFTER INSERT OR DELETE ON layer_entries
FOR EACH ROW EXECUTE FUNCTION update_layer_stats();
```

### 更新时间戳

```sql
-- 自动更新 updated_at 字段
CREATE OR REPLACE FUNCTION update_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_native_mounts_timestamp
BEFORE UPDATE ON native_mounts
FOR EACH ROW EXECUTE FUNCTION update_timestamp();
```

## 数据清理

### 孤立文本块清理

```sql
-- 清理引用计数为 0 的文本块
CREATE OR REPLACE FUNCTION cleanup_orphan_text_blocks()
RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER;
BEGIN
    DELETE FROM text_blocks
    WHERE ref_count = 0
    AND last_accessed_at < NOW() - INTERVAL '7 days';
    
    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

-- 定期执行（使用 pg_cron）
-- SELECT cron.schedule('cleanup-text-blocks', '0 3 * * *', 'SELECT cleanup_orphan_text_blocks()');
```

### 过期快照清理

```sql
-- 清理过期快照
CREATE OR REPLACE FUNCTION cleanup_expired_snapshots()
RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER;
BEGIN
    DELETE FROM snapshots
    WHERE expires_at IS NOT NULL
    AND expires_at < NOW()
    AND status = 'ready';
    
    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;
```

## 监控查询

### 层统计

```sql
-- 租户的层统计
SELECT 
    l.layer_id,
    l.layer_name,
    l.file_count,
    pg_size_pretty(l.total_size) as size,
    l.created_at,
    l.status
FROM layers l
WHERE l.tenant_id = $1
ORDER BY l.created_at DESC;

-- 层链深度
WITH RECURSIVE layer_chain AS (
    SELECT layer_id, parent_layer_id, 0 as depth
    FROM layers WHERE layer_id = $1
    UNION ALL
    SELECT l.layer_id, l.parent_layer_id, lc.depth + 1
    FROM layers l
    JOIN layer_chain lc ON l.layer_id = lc.parent_layer_id
)
SELECT MAX(depth) + 1 as chain_depth FROM layer_chain;
```

### 文本块去重率

```sql
-- 文本块去重统计
SELECT 
    COUNT(*) as total_blocks,
    COUNT(DISTINCT content_hash) as unique_blocks,
    ROUND(100.0 * (1 - COUNT(DISTINCT content_hash)::NUMERIC / COUNT(*)), 2) as dedup_rate_percent,
    pg_size_pretty(SUM(byte_size)) as total_size,
    pg_size_pretty(SUM(DISTINCT byte_size)) as unique_size
FROM text_blocks
WHERE ref_count > 0;
```

### 审计统计

```sql
-- 最近24小时的操作统计
SELECT 
    operation,
    COUNT(*) as count,
    AVG(duration_ms)::INTEGER as avg_duration_ms,
    PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY duration_ms)::INTEGER as p50_ms,
    PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY duration_ms)::INTEGER as p99_ms,
    SUM(CASE WHEN success THEN 1 ELSE 0 END) as success_count,
    SUM(CASE WHEN NOT success THEN 1 ELSE 0 END) as error_count
FROM audit_logs
WHERE tenant_id = $1
AND created_at > NOW() - INTERVAL '24 hours'
GROUP BY operation
ORDER BY count DESC;
```

## 关联任务

- **Task 06**: 审计系统实现（audit_logs）
- **Task 08**: 分层文件系统实现（layers, layer_entries）
- **Task 09**: 文本文件优化实现（text_blocks, text_file_metadata, text_line_map）
- **Task 11**: 性能优化（索引、分区、缓存策略）

## 相关规范

- [spec/01: 数据库 Schema (MVP)](01-database-schema.md) - 核心表设计
- [spec/03: 审计系统](03-audit-system.md) - 审计日志详细设计
- [spec/04: 分层文件系统](04-layered-filesystem.md) - 层管理详细设计
- [spec/07: 性能优化](07-performance.md) - 原生挂载详细设计（含原生挂载章节）
- [spec/10: 文本文件优化](10-text-file-optimization.md) - 文本存储详细设计

# 数据库设计规范

## 概述

Tarbox 使用 PostgreSQL 作为存储后端，利用其 ACID 特性保证数据一致性。数据库设计遵循元数据与数据分离的原则，优化查询性能。

## 设计原则

### 1. 元数据与数据分离

- **元数据表**：存储文件属性、目录结构等轻量信息，查询频繁
- **数据块表**：存储实际文件内容，读写密集
- **分离优势**：独立优化、分别缓存、便于扩展

### 2. 范式与反范式平衡

- **元数据表**：遵循 3NF，保证数据一致性
- **日志表**：适度反范式，提高查询性能
- **冗余字段**：在性能关键路径上适当冗余

### 3. 分区策略

- **时序数据**：按时间分区（审计日志、统计数据）
- **大表分区**：避免单表过大影响性能
- **自动管理**：使用触发器或定时任务自动创建分区

### 4. 索引设计

- **主键索引**：所有表都有主键
- **外键索引**：关联查询的外键建立索引
- **复合索引**：常见查询条件的组合索引
- **部分索引**：使用 WHERE 条件过滤的索引

## 多租户支持

### 租户隔离原则

所有数据表都包含 `tenant_id` 字段，实现完全的数据隔离：

- **主键包含租户 ID**：`PRIMARY KEY (tenant_id, inode_id)`
- **外键包含租户 ID**：确保跨表引用在同一租户内
- **索引以租户 ID 开头**：优化租户级查询性能
- **所有查询必须指定租户**：WHERE tenant_id = ?

### 租户表

```sql
CREATE TABLE tenants (
    tenant_id UUID PRIMARY KEY,
    tenant_name VARCHAR(253) NOT NULL UNIQUE,
    
    -- 组织信息
    organization VARCHAR(253),
    project VARCHAR(253),
    
    -- 根 inode
    root_inode_id BIGINT NOT NULL,
    
    -- 配额
    quota_bytes BIGINT,
    quota_inodes BIGINT,
    quota_layers INTEGER,
    
    -- 使用统计
    used_bytes BIGINT NOT NULL DEFAULT 0,
    used_inodes BIGINT NOT NULL DEFAULT 0,
    used_layers INTEGER NOT NULL DEFAULT 0,
    
    -- 状态
    status VARCHAR(20) NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'suspended', 'deleted')),
    
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    
    -- 配置
    config JSONB
);

CREATE INDEX idx_tenants_name ON tenants(tenant_name) WHERE status = 'active';
CREATE INDEX idx_tenants_status ON tenants(status);
```

## 核心数据模型

### 1. inodes 表（文件/目录元数据）

```sql
CREATE TABLE inodes (
    inode_id BIGSERIAL,
    tenant_id UUID NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    parent_id BIGINT,
    name VARCHAR(255) NOT NULL,
    inode_type VARCHAR(10) NOT NULL CHECK (inode_type IN ('file', 'dir', 'symlink', 'hardlink')),
    
    -- POSIX 属性
    mode INTEGER NOT NULL,                    -- 权限位（如 0755）
    uid INTEGER NOT NULL,                     -- 用户 ID
    gid INTEGER NOT NULL,                     -- 组 ID
    size BIGINT NOT NULL DEFAULT 0,          -- 文件大小（字节）
    
    -- 时间戳
    atime TIMESTAMP NOT NULL DEFAULT NOW(),   -- 最后访问时间
    mtime TIMESTAMP NOT NULL DEFAULT NOW(),   -- 最后修改时间
    ctime TIMESTAMP NOT NULL DEFAULT NOW(),   -- 最后状态改变时间
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    
    -- 链接相关
    link_target TEXT,                         -- 符号链接目标
    hardlink_target BIGINT REFERENCES inodes(inode_id), -- 硬链接目标
    nlinks INTEGER NOT NULL DEFAULT 1,        -- 硬链接计数
    
    -- 扩展属性
    xattrs JSONB,                            -- 扩展属性
    
    -- 版本控制
    version INTEGER NOT NULL DEFAULT 1,
    is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
    deleted_at TIMESTAMP,
    
    PRIMARY KEY (tenant_id, inode_id),
    FOREIGN KEY (tenant_id, parent_id) REFERENCES inodes(tenant_id, inode_id) ON DELETE CASCADE,
    UNIQUE(tenant_id, parent_id, name) WHERE is_deleted = FALSE
);

-- 索引（所有索引都以 tenant_id 开头）
CREATE INDEX idx_inodes_tenant_parent ON inodes(tenant_id, parent_id) WHERE is_deleted = FALSE;
CREATE INDEX idx_inodes_tenant_name ON inodes(tenant_id, name) WHERE is_deleted = FALSE;
CREATE INDEX idx_inodes_tenant_type ON inodes(tenant_id, inode_type) WHERE is_deleted = FALSE;
CREATE INDEX idx_inodes_tenant_parent_name ON inodes(tenant_id, parent_id, name) WHERE is_deleted = FALSE;
```

### 2. blocks 表（数据块存储）

```sql
CREATE TABLE blocks (
    block_id BIGSERIAL,
    tenant_id UUID NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    inode_id BIGINT NOT NULL,
    block_index INTEGER NOT NULL,            -- 块在文件中的索引（0, 1, 2...）
    block_size INTEGER NOT NULL,             -- 实际数据大小
    data BYTEA NOT NULL,                     -- 数据块内容
    checksum VARCHAR(64) NOT NULL,           -- SHA256 校验和
    
    -- 压缩和去重
    is_compressed BOOLEAN NOT NULL DEFAULT FALSE,
    compression_type VARCHAR(20),            -- 压缩算法（zstd, lz4, etc.）
    original_size INTEGER,                   -- 压缩前大小
    
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    last_accessed_at TIMESTAMP NOT NULL DEFAULT NOW(),
    
    PRIMARY KEY (tenant_id, block_id),
    FOREIGN KEY (tenant_id, inode_id) REFERENCES inodes(tenant_id, inode_id) ON DELETE CASCADE,
    UNIQUE(tenant_id, inode_id, block_index)
);

-- 索引
CREATE INDEX idx_blocks_tenant_inode ON blocks(tenant_id, inode_id);
CREATE INDEX idx_blocks_tenant_checksum ON blocks(tenant_id, checksum); -- 租户内去重
CREATE INDEX idx_blocks_tenant_accessed ON blocks(tenant_id, last_accessed_at);
```

### 3. audit_logs 表（审计日志）

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
    
    -- 元数据
    bytes_read BIGINT,
    bytes_written BIGINT,
    duration_ms INTEGER,                     -- 操作耗时（毫秒）
    
    -- 附加信息
    metadata JSONB,                          -- 额外的元数据
    
    -- 原生挂载相关
    is_native_mount BOOLEAN DEFAULT false,   -- 是否为原生挂载操作
    native_source_path TEXT,                 -- 原生文件系统路径
    
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    
    -- 分区键
    log_date DATE NOT NULL DEFAULT CURRENT_DATE,
    
    PRIMARY KEY (tenant_id, log_id),
    FOREIGN KEY (tenant_id, inode_id) REFERENCES inodes(tenant_id, inode_id) ON DELETE SET NULL
) PARTITION BY HASH (tenant_id);

-- 按租户分区（4个分区）
CREATE TABLE audit_logs_0 PARTITION OF audit_logs
    FOR VALUES WITH (MODULUS 4, REMAINDER 0);
CREATE TABLE audit_logs_1 PARTITION OF audit_logs
    FOR VALUES WITH (MODULUS 4, REMAINDER 1);
CREATE TABLE audit_logs_2 PARTITION OF audit_logs
    FOR VALUES WITH (MODULUS 4, REMAINDER 2);
CREATE TABLE audit_logs_3 PARTITION OF audit_logs
    FOR VALUES WITH (MODULUS 4, REMAINDER 3);

-- 索引
CREATE INDEX idx_audit_tenant_inode ON audit_logs(tenant_id, inode_id, created_at);
CREATE INDEX idx_audit_tenant_operation ON audit_logs(tenant_id, operation, created_at);
CREATE INDEX idx_audit_tenant_user ON audit_logs(tenant_id, uid, created_at);
CREATE INDEX idx_audit_tenant_created ON audit_logs(tenant_id, created_at);
```

### 4. snapshots 表（快照）

```sql
CREATE TABLE snapshots (
    snapshot_id BIGSERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    
    -- 快照范围
    root_inode_id BIGINT NOT NULL REFERENCES inodes(inode_id),
    
    -- 快照元数据
    inode_count BIGINT NOT NULL,
    total_size BIGINT NOT NULL,
    
    -- 状态
    status VARCHAR(20) NOT NULL CHECK (status IN ('creating', 'ready', 'deleting', 'failed')),
    
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMP,                    -- 过期时间（可选）
    
    -- 快照数据（存储 inode 状态的 JSONB）
    metadata JSONB NOT NULL
);

-- 索引
CREATE INDEX idx_snapshots_created ON snapshots(created_at);
CREATE INDEX idx_snapshots_status ON snapshots(status);
```

### 5. mount_points 表（挂载点管理）

```sql
CREATE TABLE mount_points (
    mount_id SERIAL PRIMARY KEY,
    mount_path TEXT NOT NULL UNIQUE,
    root_inode_id BIGINT NOT NULL REFERENCES inodes(inode_id),
    
    -- K8s 相关
    namespace VARCHAR(253),
    pvc_name VARCHAR(253),
    pod_name VARCHAR(253),
    
    -- 挂载选项
    read_only BOOLEAN NOT NULL DEFAULT FALSE,
    options JSONB,                           -- 挂载选项
    
    -- 状态
    status VARCHAR(20) NOT NULL CHECK (status IN ('active', 'inactive', 'error')),
    
    mounted_at TIMESTAMP NOT NULL DEFAULT NOW(),
    unmounted_at TIMESTAMP,
    last_accessed_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- 索引
CREATE INDEX idx_mount_points_status ON mount_points(status);
CREATE INDEX idx_mount_points_namespace ON mount_points(namespace, pvc_name);
```

### 6. text_blocks 表（文本块存储）

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
COMMENT ON COLUMN text_blocks.content_hash IS 'SHA-256 哈希，用于内容去重';
COMMENT ON COLUMN text_blocks.ref_count IS '引用计数，为 0 时可以清理';
```

### 7. text_file_metadata 表（文本文件元数据）

```sql
CREATE TABLE text_file_metadata (
    tenant_id UUID NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    inode_id BIGINT NOT NULL,
    layer_id UUID NOT NULL,
    total_lines INTEGER NOT NULL,
    encoding VARCHAR(20) NOT NULL DEFAULT 'UTF-8',
    line_ending VARCHAR(10) NOT NULL DEFAULT 'LF' CHECK (line_ending IN ('LF', 'CRLF', 'CR')),
    has_trailing_newline BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    PRIMARY KEY (tenant_id, inode_id, layer_id),
    FOREIGN KEY (tenant_id, inode_id) REFERENCES inodes(tenant_id, inode_id) ON DELETE CASCADE
);

CREATE INDEX idx_text_file_metadata_layer ON text_file_metadata(layer_id);

COMMENT ON TABLE text_file_metadata IS '文本文件的元数据信息';
COMMENT ON COLUMN text_file_metadata.line_ending IS '行结束符类型：LF (Unix), CRLF (Windows), CR (旧Mac)';
```

### 8. text_line_map 表（文本行映射）

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

### 9. native_mounts 表（原生文件系统挂载）

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
CREATE INDEX idx_native_mounts_priority ON native_mounts(priority, mount_path);

COMMENT ON TABLE native_mounts IS '原生文件系统挂载配置';
COMMENT ON COLUMN native_mounts.mount_path IS '在 Tarbox 中的虚拟路径';
COMMENT ON COLUMN native_mounts.source_path IS '宿主机的实际路径，支持变量 {tenant_id}';
COMMENT ON COLUMN native_mounts.is_shared IS '是否跨租户共享（如系统目录）';
COMMENT ON COLUMN native_mounts.priority IS '路径匹配优先级，越小越优先';
```

### 10. statistics 表（统计信息）

```sql
CREATE TABLE statistics (
    stat_id BIGSERIAL PRIMARY KEY,
    metric_name VARCHAR(100) NOT NULL,
    metric_value BIGINT NOT NULL,
    
    -- 维度
    mount_id INTEGER REFERENCES mount_points(mount_id) ON DELETE CASCADE,
    layer_id UUID,  -- 关联的层
    
    -- 标签
    labels JSONB,
    
    recorded_at TIMESTAMP NOT NULL DEFAULT NOW(),
    
    -- 时间分区
    stat_date DATE NOT NULL DEFAULT CURRENT_DATE
) PARTITION BY RANGE (stat_date);

-- 创建分区
CREATE TABLE statistics_2026_01 PARTITION OF statistics
    FOR VALUES FROM ('2026-01-01') TO ('2026-02-01');

-- 索引
CREATE INDEX idx_statistics_metric ON statistics(metric_name, recorded_at);
CREATE INDEX idx_statistics_layer ON statistics(layer_id, recorded_at);
```

## 初始化脚本

### 创建根目录

```sql
-- 插入根 inode（inode_id = 1）
INSERT INTO inodes (inode_id, parent_id, name, inode_type, mode, uid, gid, size)
VALUES (1, NULL, '/', 'dir', 493, 0, 0, 4096); -- mode 493 = 0755

-- 重置序列
SELECT setval('inodes_inode_id_seq', 1, true);
```



## 性能优化

### 连接池配置

```toml
[database.pool]
max_connections = 50
min_connections = 10
connection_timeout = 30
idle_timeout = 600
max_lifetime = 1800
```

### 查询优化

```sql
-- 启用并行查询
SET max_parallel_workers_per_gather = 4;

-- 统计信息更新
ANALYZE inodes;
ANALYZE blocks;
ANALYZE audit_logs;

-- 定期 VACUUM
-- 建议使用 pg_cron 或外部调度器
VACUUM ANALYZE;
```

### 分区管理

创建自动分区管理函数：

```sql
CREATE OR REPLACE FUNCTION create_monthly_partitions()
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
        
        -- audit_logs 分区
        partition_name := 'audit_logs_' || TO_CHAR(start_date, 'YYYY_MM');
        EXECUTE format(
            'CREATE TABLE IF NOT EXISTS %I PARTITION OF audit_logs FOR VALUES FROM (%L) TO (%L)',
            partition_name, start_date, end_date
        );
        
        -- statistics 分区
        partition_name := 'statistics_' || TO_CHAR(start_date, 'YYYY_MM');
        EXECUTE format(
            'CREATE TABLE IF NOT EXISTS %I PARTITION OF statistics FOR VALUES FROM (%L) TO (%L)',
            partition_name, start_date, end_date
        );
    END LOOP;
END;
$$ LANGUAGE plpgsql;

-- 定期调用（使用 pg_cron）
SELECT cron.schedule('create-partitions', '0 0 1 * *', 'SELECT create_monthly_partitions()');
```

## 备份策略

### 逻辑备份

```bash
# 完整备份
pg_dump -Fc tarbox > tarbox_backup_$(date +%Y%m%d).dump

# 仅备份架构
pg_dump -s tarbox > tarbox_schema.sql
```

### 物理备份（推荐）

使用 pgBackRest 或 WAL-G 进行连续归档和时间点恢复（PITR）。

## 监控指标

关键监控查询：

```sql
-- 文件统计
SELECT inode_type, 
       COUNT(*) as file_count,
       SUM(size) as total_size,
       AVG(size) as avg_size
FROM inodes 
WHERE is_deleted = FALSE
GROUP BY inode_type;

-- 最近活跃的文件
SELECT inode_id, name, size, atime
FROM inodes
WHERE inode_type = 'file' AND is_deleted = FALSE
ORDER BY atime DESC
LIMIT 100;

-- 数据块统计
SELECT COUNT(*) as block_count,
       SUM(block_size) as total_size,
       COUNT(DISTINCT inode_id) as file_count,
       SUM(CASE WHEN is_compressed THEN 1 ELSE 0 END) as compressed_blocks
FROM blocks;

-- 审计统计（最近24小时）
SELECT operation,
       COUNT(*) as count,
       AVG(duration_ms) as avg_duration_ms,
       SUM(CASE WHEN success THEN 1 ELSE 0 END) as success_count
FROM audit_logs
WHERE created_at > NOW() - INTERVAL '24 hours'
GROUP BY operation;

-- 层统计
SELECT COUNT(*) as layer_count,
       SUM(file_count) as total_files,
       SUM(total_size) as total_size
FROM layers;
```

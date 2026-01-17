# 数据库设计规范 - MVP 核心

## 概述

Tarbox 使用 PostgreSQL 作为存储后端，利用其 ACID 特性保证数据一致性。本文档描述 MVP 阶段的核心数据库设计，包括租户管理、文件元数据和数据块存储。

**高级特性**（分层、文本优化、审计日志等）见 [spec/01-advanced-storage.md](01-advanced-storage.md)。

## 设计原则

### 1. 元数据与数据分离

- **元数据表**：存储文件属性、目录结构等轻量信息，查询频繁
- **数据块表**：存储实际文件内容，读写密集
- **分离优势**：独立优化、分别缓存、便于扩展

### 2. 范式与反范式平衡

- **元数据表**：遵循 3NF，保证数据一致性
- **日志表**：适度反范式，提高查询性能
- **冗余字段**：在性能关键路径上适当冗余

### 3. 索引设计

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
- **所有查询必须指定租户**：`WHERE tenant_id = ?`

## MVP 核心表

### 1. tenants 表（租户管理）

```sql
CREATE TABLE tenants (
    tenant_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_name VARCHAR(253) NOT NULL UNIQUE,
    
    -- 组织信息
    organization VARCHAR(253),
    project VARCHAR(253),
    
    -- 根 inode
    root_inode_id BIGINT NOT NULL,
    
    -- 配额
    quota_bytes BIGINT,
    quota_inodes BIGINT,
    
    -- 使用统计
    used_bytes BIGINT NOT NULL DEFAULT 0,
    used_inodes BIGINT NOT NULL DEFAULT 0,
    
    -- 状态
    status VARCHAR(20) NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'suspended', 'deleted')),
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    
    -- 配置
    config JSONB
);

CREATE INDEX idx_tenants_name ON tenants(tenant_name) WHERE status = 'active';
CREATE INDEX idx_tenants_status ON tenants(status);

COMMENT ON TABLE tenants IS 'MVP: 租户管理，实现多租户隔离';
COMMENT ON COLUMN tenants.tenant_name IS '租户名称，全局唯一，符合 DNS 标签规范';
COMMENT ON COLUMN tenants.root_inode_id IS '租户根目录的 inode ID';
COMMENT ON COLUMN tenants.quota_bytes IS '存储配额（字节），NULL 表示无限制';
```

### 2. inodes 表（文件/目录元数据）

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
    atime TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,   -- 最后访问时间
    mtime TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,   -- 最后修改时间
    ctime TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,   -- 最后状态改变时间
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    
    -- 链接相关
    link_target TEXT,                         -- 符号链接目标
    hardlink_target BIGINT,                   -- 硬链接目标 inode_id
    nlinks INTEGER NOT NULL DEFAULT 1,        -- 硬链接计数
    
    -- 扩展属性
    xattrs JSONB,                            -- 扩展属性
    
    -- 版本控制
    version INTEGER NOT NULL DEFAULT 1,
    is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
    deleted_at TIMESTAMPTZ,
    
    PRIMARY KEY (tenant_id, inode_id),
    FOREIGN KEY (tenant_id, parent_id) REFERENCES inodes(tenant_id, inode_id) ON DELETE CASCADE,
    UNIQUE(tenant_id, parent_id, name) WHERE is_deleted = FALSE
);

-- 索引（所有索引都以 tenant_id 开头）
CREATE INDEX idx_inodes_tenant_parent ON inodes(tenant_id, parent_id) WHERE is_deleted = FALSE;
CREATE INDEX idx_inodes_tenant_name ON inodes(tenant_id, name) WHERE is_deleted = FALSE;
CREATE INDEX idx_inodes_tenant_type ON inodes(tenant_id, inode_type) WHERE is_deleted = FALSE;
CREATE INDEX idx_inodes_tenant_parent_name ON inodes(tenant_id, parent_id, name) WHERE is_deleted = FALSE;

COMMENT ON TABLE inodes IS 'MVP: 文件和目录的元数据';
COMMENT ON COLUMN inodes.inode_id IS 'Inode ID，租户内唯一（BIGSERIAL）';
COMMENT ON COLUMN inodes.parent_id IS '父目录的 inode_id，根目录为 NULL';
COMMENT ON COLUMN inodes.mode IS 'POSIX 权限位，八进制表示（如 0755 = 493）';
COMMENT ON COLUMN inodes.is_deleted IS '软删除标记';
```

### 3. data_blocks 表（数据块存储）

```sql
CREATE TABLE data_blocks (
    block_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    inode_id BIGINT NOT NULL,
    block_index INTEGER NOT NULL,            -- 块在文件中的索引（0, 1, 2...）
    block_size INTEGER NOT NULL,             -- 实际数据大小
    data BYTEA NOT NULL,                     -- 数据块内容
    content_hash VARCHAR(64) NOT NULL,       -- BLAKE3 哈希
    
    -- 压缩
    is_compressed BOOLEAN NOT NULL DEFAULT FALSE,
    compression_type VARCHAR(20),            -- 压缩算法（zstd, lz4, etc.）
    original_size INTEGER,                   -- 压缩前大小
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_accessed_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (tenant_id, inode_id) REFERENCES inodes(tenant_id, inode_id) ON DELETE CASCADE,
    UNIQUE(tenant_id, inode_id, block_index)
);

-- 索引
CREATE INDEX idx_data_blocks_tenant_inode ON data_blocks(tenant_id, inode_id, block_index);
CREATE INDEX idx_data_blocks_tenant_hash ON data_blocks(tenant_id, content_hash);
CREATE INDEX idx_data_blocks_accessed ON data_blocks(tenant_id, last_accessed_at);

COMMENT ON TABLE data_blocks IS 'MVP: 文件数据块存储';
COMMENT ON COLUMN data_blocks.block_index IS '块在文件中的索引，从 0 开始';
COMMENT ON COLUMN data_blocks.content_hash IS 'BLAKE3 内容哈希，用于去重和校验';
COMMENT ON COLUMN data_blocks.data IS '实际数据内容（压缩后）';
```

## 初始化脚本

### 创建数据库

```sql
CREATE DATABASE tarbox
    ENCODING = 'UTF8'
    LC_COLLATE = 'en_US.UTF-8'
    LC_CTYPE = 'en_US.UTF-8';

\c tarbox

-- 启用 UUID 扩展
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";
```

### 创建租户和根目录

```sql
-- 创建租户
DO $$
DECLARE
    new_tenant_id UUID;
    root_inode_id BIGINT;
BEGIN
    -- 生成租户 ID
    new_tenant_id := gen_random_uuid();
    
    -- 插入根 inode（inode_id 将由 SERIAL 生成）
    INSERT INTO inodes (tenant_id, parent_id, name, inode_type, mode, uid, gid, size)
    VALUES (new_tenant_id, NULL, '/', 'dir', 493, 0, 0, 4096) -- mode 493 = 0o755
    RETURNING inode_id INTO root_inode_id;
    
    -- 插入租户
    INSERT INTO tenants (tenant_id, tenant_name, root_inode_id, organization, project)
    VALUES (new_tenant_id, 'default', root_inode_id, 'Tarbox', 'Default Project');
    
    RAISE NOTICE 'Created tenant: % with root inode: %', new_tenant_id, root_inode_id;
END $$;
```

### 示例数据

```sql
-- 假设租户 ID 和根 inode ID 已知
DO $$
DECLARE
    tenant UUID := '550e8400-e29b-41d4-a716-446655440000'; -- 替换为实际 tenant_id
    root_inode BIGINT := 1; -- 替换为实际 root_inode_id
    data_inode BIGINT;
    file_inode BIGINT;
BEGIN
    -- 创建 /data 目录
    INSERT INTO inodes (tenant_id, parent_id, name, inode_type, mode, uid, gid, size)
    VALUES (tenant, root_inode, 'data', 'dir', 493, 1000, 1000, 4096)
    RETURNING inode_id INTO data_inode;
    
    -- 创建 /data/hello.txt 文件
    INSERT INTO inodes (tenant_id, parent_id, name, inode_type, mode, uid, gid, size)
    VALUES (tenant, data_inode, 'hello.txt', 'file', 420, 1000, 1000, 13) -- mode 420 = 0o644
    RETURNING inode_id INTO file_inode;
    
    -- 插入文件数据
    INSERT INTO data_blocks (tenant_id, inode_id, block_index, block_size, data, content_hash)
    VALUES (
        tenant,
        file_inode,
        0,
        13,
        'Hello, World!'::bytea,
        encode(digest('Hello, World!', 'sha256'), 'hex')
    );
    
    RAISE NOTICE 'Created /data/hello.txt';
END $$;
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

### PostgreSQL 配置优化

```ini
# 内存配置
shared_buffers = 4GB              # 共享缓冲区（服务器内存的 25%）
effective_cache_size = 12GB       # 操作系统缓存（服务器内存的 75%）
work_mem = 256MB                  # 每个操作的工作内存
maintenance_work_mem = 1GB        # 维护操作内存

# WAL 配置
wal_buffers = 16MB
checkpoint_timeout = 10min
checkpoint_completion_target = 0.9
max_wal_size = 4GB
min_wal_size = 1GB

# 并发配置
max_connections = 200
max_parallel_workers_per_gather = 4
max_parallel_workers = 8

# 查询优化
random_page_cost = 1.1            # SSD 降低随机读成本
effective_io_concurrency = 200    # SSD 可以处理更多并发 I/O
default_statistics_target = 100   # 增加统计样本
```

### 查询优化

```sql
-- 启用并行查询
SET max_parallel_workers_per_gather = 4;

-- 定期更新统计信息
ANALYZE tenants;
ANALYZE inodes;
ANALYZE data_blocks;

-- 定期 VACUUM
VACUUM ANALYZE;
```

## 监控查询

### 租户统计

```sql
-- 租户使用情况
SELECT 
    t.tenant_name,
    COUNT(DISTINCT i.inode_id) as file_count,
    pg_size_pretty(SUM(i.size)) as total_size,
    pg_size_pretty(t.quota_bytes) as quota,
    ROUND(100.0 * SUM(i.size) / NULLIF(t.quota_bytes, 0), 2) as usage_percent
FROM tenants t
LEFT JOIN inodes i ON t.tenant_id = i.tenant_id
WHERE i.is_deleted = FALSE
GROUP BY t.tenant_id, t.tenant_name, t.quota_bytes;
```

### 文件统计

```sql
-- 按类型统计文件
SELECT 
    inode_type,
    COUNT(*) as count,
    pg_size_pretty(SUM(size)) as total_size,
    pg_size_pretty(AVG(size)::BIGINT) as avg_size
FROM inodes
WHERE tenant_id = $1 AND is_deleted = FALSE
GROUP BY inode_type;

-- 最大的文件
SELECT 
    i.inode_id,
    i.name,
    pg_size_pretty(i.size) as size,
    i.mtime
FROM inodes i
WHERE i.tenant_id = $1 
AND i.inode_type = 'file' 
AND i.is_deleted = FALSE
ORDER BY i.size DESC
LIMIT 10;
```

### 数据块统计

```sql
-- 数据块统计
SELECT 
    COUNT(*) as block_count,
    pg_size_pretty(SUM(block_size)) as total_size,
    pg_size_pretty(AVG(block_size)::BIGINT) as avg_block_size,
    COUNT(DISTINCT inode_id) as file_count,
    SUM(CASE WHEN is_compressed THEN 1 ELSE 0 END) as compressed_blocks,
    ROUND(100.0 * SUM(CASE WHEN is_compressed THEN block_size ELSE 0 END) / SUM(original_size), 2) as compression_ratio
FROM data_blocks
WHERE tenant_id = $1;

-- 内容去重统计
SELECT 
    COUNT(*) as total_blocks,
    COUNT(DISTINCT content_hash) as unique_blocks,
    ROUND(100.0 * (1 - COUNT(DISTINCT content_hash)::NUMERIC / COUNT(*)), 2) as dedup_rate_percent
FROM data_blocks
WHERE tenant_id = $1;
```

## 备份与恢复

### 逻辑备份

```bash
# 完整备份
pg_dump -Fc -d tarbox > tarbox_backup_$(date +%Y%m%d).dump

# 仅备份架构
pg_dump -s -d tarbox > tarbox_schema.sql

# 仅备份数据
pg_dump -a -d tarbox > tarbox_data.sql

# 恢复
pg_restore -d tarbox tarbox_backup_20260117.dump
```

### 物理备份（推荐）

使用 pgBackRest 或 WAL-G 进行连续归档和时间点恢复（PITR）：

```bash
# 使用 pgBackRest
pgbackrest --stanza=tarbox --type=full backup
pgbackrest --stanza=tarbox --type=incr backup

# 恢复到特定时间点
pgbackrest --stanza=tarbox --type=time "--target=2026-01-17 10:00:00" restore
```

## 维护任务

### 自动更新时间戳

```sql
-- 自动更新 updated_at 字段
CREATE OR REPLACE FUNCTION update_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_tenants_timestamp
BEFORE UPDATE ON tenants
FOR EACH ROW EXECUTE FUNCTION update_timestamp();
```

### 自动更新租户统计

```sql
-- 更新租户使用统计
CREATE OR REPLACE FUNCTION update_tenant_usage()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        UPDATE tenants
        SET used_inodes = used_inodes + 1,
            used_bytes = used_bytes + NEW.size
        WHERE tenant_id = NEW.tenant_id;
    ELSIF TG_OP = 'UPDATE' THEN
        UPDATE tenants
        SET used_bytes = used_bytes - OLD.size + NEW.size
        WHERE tenant_id = NEW.tenant_id;
    ELSIF TG_OP = 'DELETE' THEN
        UPDATE tenants
        SET used_inodes = used_inodes - 1,
            used_bytes = used_bytes - OLD.size
        WHERE tenant_id = OLD.tenant_id;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_inodes_tenant_usage
AFTER INSERT OR UPDATE OR DELETE ON inodes
FOR EACH ROW 
WHEN (OLD.is_deleted = FALSE OR NEW.is_deleted = FALSE)
EXECUTE FUNCTION update_tenant_usage();
```

## 数据一致性检查

```sql
-- 检查孤立的 inode（没有父节点且不是根）
SELECT i.tenant_id, i.inode_id, i.name
FROM inodes i
WHERE i.parent_id IS NOT NULL
AND NOT EXISTS (
    SELECT 1 FROM inodes p 
    WHERE p.tenant_id = i.tenant_id 
    AND p.inode_id = i.parent_id
);

-- 检查孤立的数据块（inode 已删除）
SELECT db.block_id, db.inode_id
FROM data_blocks db
WHERE NOT EXISTS (
    SELECT 1 FROM inodes i 
    WHERE i.tenant_id = db.tenant_id 
    AND i.inode_id = db.inode_id
    AND i.is_deleted = FALSE
);

-- 检查租户配额超限
SELECT t.tenant_name, t.used_bytes, t.quota_bytes
FROM tenants t
WHERE t.quota_bytes IS NOT NULL
AND t.used_bytes > t.quota_bytes;
```

## 迁移到高级功能

当需要启用高级功能时，执行以下步骤：

1. **备份数据库**：`pg_dump -Fc tarbox > backup.dump`
2. **执行高级 Schema**：运行 [spec/01-advanced-storage.md](01-advanced-storage.md) 中的建表语句
3. **数据迁移**：根据需要迁移现有数据到新表
4. **更新应用代码**：启用分层、审计等功能
5. **验证功能**：测试新功能是否正常工作

## 关联任务

- **Task 02**: 数据库层 MVP ✅ 已完成
- **Task 03**: 文件系统核心 MVP ✅ 已完成  
- **Task 04**: CLI 工具 MVP ✅ 已完成

## 相关规范

- [spec/01-advanced: 高级存储特性](01-advanced-storage.md) - 分层、文本优化、审计日志
- [spec/09: 多租户隔离](09-multi-tenancy.md) - 租户隔离机制详解
- [spec/00: 系统概览](00-overview.md) - 整体架构

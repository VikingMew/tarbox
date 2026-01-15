-- Initial database schema for Tarbox MVP
-- Creates tenants, inodes, and data_blocks tables

-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Tenants table
CREATE TABLE tenants (
    tenant_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_name VARCHAR(253) NOT NULL UNIQUE,
    root_inode_id BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_tenants_name ON tenants(tenant_name);

COMMENT ON TABLE tenants IS 'Tenant information for multi-tenancy support';
COMMENT ON COLUMN tenants.root_inode_id IS 'Root directory inode_id (always 1)';

-- Inodes table
CREATE TABLE inodes (
    inode_id BIGSERIAL,
    tenant_id UUID NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    parent_id BIGINT,
    name VARCHAR(255) NOT NULL,
    inode_type VARCHAR(10) NOT NULL CHECK (inode_type IN ('file', 'dir', 'symlink')),

    -- POSIX attributes
    mode INTEGER NOT NULL,
    uid INTEGER NOT NULL,
    gid INTEGER NOT NULL,
    size BIGINT NOT NULL DEFAULT 0,

    -- Timestamps
    atime TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    mtime TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    ctime TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,

    PRIMARY KEY (tenant_id, inode_id),
    FOREIGN KEY (tenant_id, parent_id) REFERENCES inodes(tenant_id, inode_id) ON DELETE CASCADE,
    UNIQUE(tenant_id, parent_id, name)
);

-- Indexes for inodes
CREATE INDEX idx_inodes_tenant_parent ON inodes(tenant_id, parent_id);
CREATE INDEX idx_inodes_tenant_parent_name ON inodes(tenant_id, parent_id, name);
CREATE INDEX idx_inodes_tenant_type ON inodes(tenant_id, inode_type);

COMMENT ON TABLE inodes IS 'File and directory metadata (inodes)';
COMMENT ON COLUMN inodes.mode IS 'POSIX permission bits (e.g., 0755)';
COMMENT ON COLUMN inodes.size IS 'File size in bytes';

-- Data blocks table
CREATE TABLE data_blocks (
    block_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    inode_id BIGINT NOT NULL,
    block_index INTEGER NOT NULL,
    data BYTEA NOT NULL,
    size INTEGER NOT NULL,
    content_hash VARCHAR(64) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY (tenant_id, inode_id) REFERENCES inodes(tenant_id, inode_id) ON DELETE CASCADE,
    UNIQUE(tenant_id, inode_id, block_index)
);

-- Indexes for data_blocks
CREATE INDEX idx_blocks_tenant_inode ON data_blocks(tenant_id, inode_id);
CREATE INDEX idx_blocks_tenant_inode_index ON data_blocks(tenant_id, inode_id, block_index);
CREATE INDEX idx_blocks_content_hash ON data_blocks(content_hash);

COMMENT ON TABLE data_blocks IS 'File data blocks storage';
COMMENT ON COLUMN data_blocks.block_index IS 'Block index in file (0, 1, 2, ...)';
COMMENT ON COLUMN data_blocks.content_hash IS 'BLAKE3 hash for deduplication';

-- Function to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger for tenants
CREATE TRIGGER update_tenants_updated_at
    BEFORE UPDATE ON tenants
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

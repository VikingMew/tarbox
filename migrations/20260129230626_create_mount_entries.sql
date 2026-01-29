-- Create mount_entries table for filesystem composition
-- This supports mounting from multiple sources: host, layer, published, working_layer

CREATE TABLE mount_entries (
    mount_entry_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE,

    -- Mount point name (for API reference, unique within tenant)
    name VARCHAR(255) NOT NULL,

    -- Virtual path (actual mount location)
    virtual_path TEXT NOT NULL,
    is_file BOOLEAN NOT NULL DEFAULT false,

    -- Source type: 'host', 'layer', 'working_layer', 'published'
    source_type VARCHAR(20) NOT NULL,

    -- Host source
    host_path TEXT,

    -- Layer source (direct reference)
    source_mount_id UUID REFERENCES mount_entries(mount_entry_id),
    source_layer_id UUID,  -- Will add foreign key in Task 21 after updating layers table
    source_subpath TEXT,

    -- Published source (reference by name)
    source_publish_name VARCHAR(255),

    -- WorkingLayer's current working layer ID (set in Task 21)
    current_layer_id UUID,

    -- Access mode: 'ro', 'rw', 'cow'
    mode VARCHAR(3) NOT NULL DEFAULT 'ro',

    enabled BOOLEAN NOT NULL DEFAULT true,
    metadata JSONB,

    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT valid_source CHECK (
        (source_type = 'host' AND host_path IS NOT NULL) OR
        (source_type = 'layer' AND source_mount_id IS NOT NULL) OR
        (source_type = 'published' AND source_publish_name IS NOT NULL) OR
        (source_type = 'working_layer')
    ),

    CONSTRAINT valid_mode CHECK (mode IN ('ro', 'rw', 'cow')),

    -- Same tenant cannot have duplicate names
    UNIQUE(tenant_id, name),
    -- Same tenant cannot have duplicate paths
    UNIQUE(tenant_id, virtual_path)
);

-- Indexes
CREATE INDEX idx_mount_entries_tenant ON mount_entries(tenant_id);
CREATE INDEX idx_mount_entries_name ON mount_entries(tenant_id, name);
CREATE INDEX idx_mount_entries_source ON mount_entries(source_mount_id);
CREATE INDEX idx_mount_entries_enabled ON mount_entries(tenant_id, enabled) WHERE enabled = true;

-- Comments
COMMENT ON TABLE mount_entries IS 'Mount entries for filesystem composition';
COMMENT ON COLUMN mount_entries.name IS 'Mount point name for API reference (e.g., "memory", "workspace")';
COMMENT ON COLUMN mount_entries.virtual_path IS 'Actual mount location in the filesystem (e.g., "/memory", "/workspace")';
COMMENT ON COLUMN mount_entries.is_file IS 'True if mounting a single file, false if mounting a directory';
COMMENT ON COLUMN mount_entries.source_type IS 'Type of mount source: host, layer, working_layer, or published';
COMMENT ON COLUMN mount_entries.mode IS 'Access mode: ro (read-only), rw (read-write), cow (copy-on-write)';

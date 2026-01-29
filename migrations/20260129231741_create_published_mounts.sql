-- Create published_mounts table for layer publishing
-- Supports publishing specific layer snapshots or working layers (real-time)

CREATE TABLE published_mounts (
    publish_id UUID PRIMARY KEY,
    mount_entry_id UUID NOT NULL REFERENCES mount_entries(mount_entry_id) ON DELETE CASCADE,
    tenant_id UUID NOT NULL REFERENCES tenants(tenant_id),

    -- Publish name (globally unique)
    publish_name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,

    -- Target type: 'layer' or 'working_layer'
    target_type VARCHAR(20) NOT NULL,
    layer_id UUID,  -- NULL if working_layer

    -- Access control
    scope VARCHAR(20) NOT NULL DEFAULT 'public',
    allowed_tenants UUID[],

    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT valid_target CHECK (
        (target_type = 'layer' AND layer_id IS NOT NULL) OR
        (target_type = 'working_layer' AND layer_id IS NULL)
    ),

    CONSTRAINT valid_scope CHECK (scope IN ('public', 'allow_list'))
);

-- Indexes
CREATE INDEX idx_published_mounts_name ON published_mounts(publish_name);
CREATE INDEX idx_published_mounts_tenant ON published_mounts(tenant_id);
CREATE INDEX idx_published_mounts_mount ON published_mounts(mount_entry_id);

-- Unique constraint: one publish per mount_entry
CREATE UNIQUE INDEX idx_published_mounts_unique_mount ON published_mounts(mount_entry_id);

-- Comments
COMMENT ON TABLE published_mounts IS 'Published mount configurations for cross-tenant sharing';
COMMENT ON COLUMN published_mounts.publish_name IS 'Globally unique name for this published mount';
COMMENT ON COLUMN published_mounts.target_type IS 'Type of publish target: layer (fixed snapshot) or working_layer (real-time)';
COMMENT ON COLUMN published_mounts.scope IS 'Access scope: public (all tenants) or allow_list (specific tenants)';
COMMENT ON COLUMN published_mounts.allowed_tenants IS 'List of tenant IDs allowed to access (only for allow_list scope)';

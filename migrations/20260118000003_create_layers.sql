-- Migration 003: Layer Management System
-- Creates tables for Docker-style layered filesystem with COW support

-- Create layers table
CREATE TABLE layers (
    layer_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    parent_layer_id UUID REFERENCES layers(layer_id) ON DELETE RESTRICT,
    layer_name VARCHAR(255) NOT NULL,
    description TEXT,
    file_count INTEGER NOT NULL DEFAULT 0,
    total_size BIGINT NOT NULL DEFAULT 0,
    status VARCHAR(20) NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'creating', 'deleting', 'archived')),
    is_readonly BOOLEAN NOT NULL DEFAULT false,
    tags JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_by VARCHAR(255) NOT NULL,

    UNIQUE(tenant_id, layer_name)
);

-- Create indexes on layers
CREATE INDEX idx_layers_tenant ON layers(tenant_id, created_at);
CREATE INDEX idx_layers_parent ON layers(parent_layer_id);
CREATE INDEX idx_layers_status ON layers(status) WHERE status = 'active';
CREATE INDEX idx_layers_tenant_status ON layers(tenant_id, status);

-- Create layer_entries table (tracks file changes per layer)
CREATE TABLE layer_entries (
    entry_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    layer_id UUID NOT NULL REFERENCES layers(layer_id) ON DELETE CASCADE,
    tenant_id UUID NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    inode_id BIGINT NOT NULL,
    path TEXT NOT NULL,
    change_type VARCHAR(10) NOT NULL CHECK (change_type IN ('add', 'modify', 'delete')),
    size_delta BIGINT,
    text_changes JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY (tenant_id, inode_id) REFERENCES inodes(tenant_id, inode_id) ON DELETE CASCADE,
    UNIQUE(layer_id, path)
);

-- Create indexes on layer_entries
CREATE INDEX idx_layer_entries_layer ON layer_entries(layer_id, change_type);
CREATE INDEX idx_layer_entries_inode ON layer_entries(tenant_id, inode_id);
CREATE INDEX idx_layer_entries_path ON layer_entries(path);
CREATE INDEX idx_layer_entries_tenant_layer ON layer_entries(tenant_id, layer_id);

-- Create tenant_current_layer table (tracks current active layer per tenant)
CREATE TABLE tenant_current_layer (
    tenant_id UUID PRIMARY KEY REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    current_layer_id UUID NOT NULL REFERENCES layers(layer_id) ON DELETE RESTRICT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create index on tenant_current_layer
CREATE INDEX idx_tenant_current_layer_layer ON tenant_current_layer(current_layer_id);

-- Trigger to auto-update layer statistics when layer_entries change
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
        SET file_count = GREATEST(file_count - 1, 0),
            total_size = GREATEST(total_size - COALESCE(OLD.size_delta, 0), 0)
        WHERE layer_id = OLD.layer_id;
    ELSIF TG_OP = 'UPDATE' THEN
        UPDATE layers
        SET total_size = total_size - COALESCE(OLD.size_delta, 0) + COALESCE(NEW.size_delta, 0)
        WHERE layer_id = NEW.layer_id;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_update_layer_stats
    AFTER INSERT OR UPDATE OR DELETE ON layer_entries
    FOR EACH ROW
    EXECUTE FUNCTION update_layer_stats();

-- Trigger to auto-update tenant_current_layer timestamp
CREATE OR REPLACE FUNCTION update_tenant_current_layer_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_update_tenant_current_layer_timestamp
    BEFORE UPDATE ON tenant_current_layer
    FOR EACH ROW
    EXECUTE FUNCTION update_tenant_current_layer_timestamp();

-- Function to get layer chain (from current layer to base)
CREATE OR REPLACE FUNCTION get_layer_chain(start_layer_id UUID)
RETURNS TABLE (
    layer_id UUID,
    parent_layer_id UUID,
    layer_name VARCHAR(255),
    depth INTEGER
) AS $$
BEGIN
    RETURN QUERY
    WITH RECURSIVE layer_chain AS (
        SELECT
            l.layer_id,
            l.parent_layer_id,
            l.layer_name,
            0 AS depth
        FROM layers l
        WHERE l.layer_id = start_layer_id

        UNION ALL

        SELECT
            l.layer_id,
            l.parent_layer_id,
            l.layer_name,
            lc.depth + 1
        FROM layers l
        INNER JOIN layer_chain lc ON l.layer_id = lc.parent_layer_id
    )
    SELECT * FROM layer_chain ORDER BY depth;
END;
$$ LANGUAGE plpgsql;

-- Function to create base layer for new tenant
CREATE OR REPLACE FUNCTION create_base_layer_for_tenant(
    p_tenant_id UUID,
    p_created_by VARCHAR(255) DEFAULT 'system'
)
RETURNS UUID AS $$
DECLARE
    v_layer_id UUID;
BEGIN
    -- Create base layer
    INSERT INTO layers (
        tenant_id,
        parent_layer_id,
        layer_name,
        description,
        status,
        is_readonly,
        created_by
    ) VALUES (
        p_tenant_id,
        NULL,
        'base',
        'Initial base layer',
        'active',
        false,
        p_created_by
    )
    RETURNING layer_id INTO v_layer_id;

    -- Set as current layer for tenant
    INSERT INTO tenant_current_layer (tenant_id, current_layer_id)
    VALUES (p_tenant_id, v_layer_id)
    ON CONFLICT (tenant_id) DO UPDATE
    SET current_layer_id = v_layer_id;

    RETURN v_layer_id;
END;
$$ LANGUAGE plpgsql;

-- Comments for documentation
COMMENT ON TABLE layers IS 'Layer metadata for Docker-style layered filesystem';
COMMENT ON COLUMN layers.layer_id IS 'Unique layer identifier';
COMMENT ON COLUMN layers.tenant_id IS 'Tenant who owns this layer';
COMMENT ON COLUMN layers.parent_layer_id IS 'Parent layer in the chain (NULL for base layer)';
COMMENT ON COLUMN layers.layer_name IS 'Human-readable layer name';
COMMENT ON COLUMN layers.description IS 'Layer description or commit message';
COMMENT ON COLUMN layers.file_count IS 'Number of file entries in this layer';
COMMENT ON COLUMN layers.total_size IS 'Total size delta of changes in this layer';
COMMENT ON COLUMN layers.status IS 'Layer status (active, creating, deleting, archived)';
COMMENT ON COLUMN layers.is_readonly IS 'Whether this layer is read-only';
COMMENT ON COLUMN layers.tags IS 'JSON tags for layer metadata';
COMMENT ON COLUMN layers.created_by IS 'User who created this layer';

COMMENT ON TABLE layer_entries IS 'File changes tracked per layer';
COMMENT ON COLUMN layer_entries.entry_id IS 'Unique entry identifier';
COMMENT ON COLUMN layer_entries.layer_id IS 'Layer this entry belongs to';
COMMENT ON COLUMN layer_entries.inode_id IS 'Inode that was modified';
COMMENT ON COLUMN layer_entries.path IS 'File path at the time of change';
COMMENT ON COLUMN layer_entries.change_type IS 'Type of change (add, modify, delete)';
COMMENT ON COLUMN layer_entries.size_delta IS 'Size change in bytes (can be negative)';
COMMENT ON COLUMN layer_entries.text_changes IS 'JSON details of text file changes';

COMMENT ON TABLE tenant_current_layer IS 'Tracks the current active layer for each tenant';
COMMENT ON COLUMN tenant_current_layer.current_layer_id IS 'The layer currently active for writes';

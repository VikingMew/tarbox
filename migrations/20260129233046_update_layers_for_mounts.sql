-- Update layers table for mount-level layer chains
-- Task 21: Mount-Level Layer Chains

-- Add mount_entry_id to layers table
ALTER TABLE layers ADD COLUMN mount_entry_id UUID REFERENCES mount_entries(mount_entry_id) ON DELETE CASCADE;

-- Add is_working flag to mark the current working layer
ALTER TABLE layers ADD COLUMN is_working BOOLEAN NOT NULL DEFAULT false;

-- Create unique constraint: each mount can have only one working layer
CREATE UNIQUE INDEX idx_layers_unique_working
    ON layers(mount_entry_id)
    WHERE is_working = true;

-- Create index for efficient mount-based queries
CREATE INDEX idx_layers_mount ON layers(mount_entry_id);

-- Add foreign key constraint for current_layer_id in mount_entries
ALTER TABLE mount_entries
    ADD CONSTRAINT fk_current_layer
    FOREIGN KEY (current_layer_id) REFERENCES layers(layer_id) ON DELETE SET NULL;

-- Comments
COMMENT ON COLUMN layers.mount_entry_id IS 'The mount entry this layer belongs to (NULL for legacy tenant-level layers)';
COMMENT ON COLUMN layers.is_working IS 'Whether this is the current working layer for the mount';
COMMENT ON INDEX idx_layers_unique_working IS 'Ensures each mount has only one working layer';

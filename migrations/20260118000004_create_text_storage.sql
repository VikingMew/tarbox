-- Migration 004: Text File Optimization System
-- Creates tables for line-level storage and content-addressed text blocks

-- Create text_blocks table (stores actual text content with deduplication)
CREATE TABLE text_blocks (
    block_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    content_hash VARCHAR(64) NOT NULL UNIQUE,
    content TEXT NOT NULL,
    line_count INTEGER NOT NULL,
    byte_size INTEGER NOT NULL,
    encoding VARCHAR(20) NOT NULL DEFAULT 'UTF-8',
    ref_count INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_accessed_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes on text_blocks
CREATE INDEX idx_text_blocks_hash ON text_blocks(content_hash);
CREATE INDEX idx_text_blocks_ref_count ON text_blocks(ref_count);
CREATE INDEX idx_text_blocks_last_accessed ON text_blocks(last_accessed_at);
CREATE INDEX idx_text_blocks_zero_refs ON text_blocks(ref_count) WHERE ref_count = 0;

-- Create text_file_metadata table (per-file metadata per layer)
CREATE TABLE text_file_metadata (
    tenant_id UUID NOT NULL REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    inode_id BIGINT NOT NULL,
    layer_id UUID NOT NULL REFERENCES layers(layer_id) ON DELETE CASCADE,
    total_lines INTEGER NOT NULL,
    encoding VARCHAR(20) NOT NULL DEFAULT 'UTF-8',
    line_ending VARCHAR(10) NOT NULL DEFAULT 'LF' CHECK (line_ending IN ('LF', 'CRLF', 'CR')),
    has_trailing_newline BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,

    PRIMARY KEY (tenant_id, inode_id, layer_id),
    FOREIGN KEY (tenant_id, inode_id) REFERENCES inodes(tenant_id, inode_id) ON DELETE CASCADE
);

-- Create indexes on text_file_metadata
CREATE INDEX idx_text_file_metadata_layer ON text_file_metadata(layer_id);
CREATE INDEX idx_text_file_metadata_inode ON text_file_metadata(tenant_id, inode_id);
CREATE INDEX idx_text_file_metadata_tenant_layer ON text_file_metadata(tenant_id, layer_id);

-- Create text_line_map table (maps line numbers to text blocks)
CREATE TABLE text_line_map (
    tenant_id UUID NOT NULL,
    inode_id BIGINT NOT NULL,
    layer_id UUID NOT NULL,
    line_number INTEGER NOT NULL,
    block_id UUID NOT NULL REFERENCES text_blocks(block_id) ON DELETE RESTRICT,
    block_line_offset INTEGER NOT NULL,

    PRIMARY KEY (tenant_id, inode_id, layer_id, line_number),
    FOREIGN KEY (tenant_id, inode_id, layer_id)
        REFERENCES text_file_metadata(tenant_id, inode_id, layer_id)
        ON DELETE CASCADE
);

-- Create indexes on text_line_map
CREATE INDEX idx_text_line_map_lookup ON text_line_map(tenant_id, inode_id, layer_id, line_number);
CREATE INDEX idx_text_line_map_block ON text_line_map(block_id);
CREATE INDEX idx_text_line_map_tenant_layer ON text_line_map(tenant_id, layer_id);

-- Trigger to update text_block last_accessed_at on read
CREATE OR REPLACE FUNCTION update_text_block_access_time()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE text_blocks
    SET last_accessed_at = CURRENT_TIMESTAMP
    WHERE block_id = NEW.block_id;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_update_text_block_access
    AFTER INSERT ON text_line_map
    FOR EACH ROW
    EXECUTE FUNCTION update_text_block_access_time();

-- Trigger to manage text_block reference counts
CREATE OR REPLACE FUNCTION manage_text_block_refcount()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        UPDATE text_blocks
        SET ref_count = ref_count + 1
        WHERE block_id = NEW.block_id;
        RETURN NEW;
    ELSIF TG_OP = 'DELETE' THEN
        UPDATE text_blocks
        SET ref_count = GREATEST(ref_count - 1, 0)
        WHERE block_id = OLD.block_id;
        RETURN OLD;
    ELSIF TG_OP = 'UPDATE' AND OLD.block_id != NEW.block_id THEN
        -- Decrement old block
        UPDATE text_blocks
        SET ref_count = GREATEST(ref_count - 1, 0)
        WHERE block_id = OLD.block_id;

        -- Increment new block
        UPDATE text_blocks
        SET ref_count = ref_count + 1
        WHERE block_id = NEW.block_id;

        RETURN NEW;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_manage_text_block_refcount
    AFTER INSERT OR UPDATE OR DELETE ON text_line_map
    FOR EACH ROW
    EXECUTE FUNCTION manage_text_block_refcount();

-- Function to find or create text block (content-addressed storage)
CREATE OR REPLACE FUNCTION find_or_create_text_block(
    p_content TEXT,
    p_encoding VARCHAR(20) DEFAULT 'UTF-8'
)
RETURNS UUID AS $$
DECLARE
    v_content_hash VARCHAR(64);
    v_block_id UUID;
    v_line_count INTEGER;
    v_byte_size INTEGER;
BEGIN
    -- Compute content hash (using MD5 as placeholder, should be BLAKE3 in application)
    v_content_hash := encode(digest(p_content, 'sha256'), 'hex');
    v_line_count := array_length(string_to_array(p_content, E'\n'), 1);
    v_byte_size := length(p_content);

    -- Try to find existing block
    SELECT block_id INTO v_block_id
    FROM text_blocks
    WHERE content_hash = v_content_hash;

    IF FOUND THEN
        -- Update access time
        UPDATE text_blocks
        SET last_accessed_at = CURRENT_TIMESTAMP
        WHERE block_id = v_block_id;

        RETURN v_block_id;
    ELSE
        -- Create new block
        INSERT INTO text_blocks (
            content_hash,
            content,
            line_count,
            byte_size,
            encoding,
            ref_count
        ) VALUES (
            v_content_hash,
            p_content,
            v_line_count,
            v_byte_size,
            p_encoding,
            0
        )
        RETURNING block_id INTO v_block_id;

        RETURN v_block_id;
    END IF;
END;
$$ LANGUAGE plpgsql;

-- Function to clean up unused text blocks (ref_count = 0)
CREATE OR REPLACE FUNCTION cleanup_unused_text_blocks(max_age_days INTEGER DEFAULT 7)
RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER;
BEGIN
    -- Delete blocks with zero references that haven't been accessed recently
    DELETE FROM text_blocks
    WHERE ref_count = 0
      AND last_accessed_at < CURRENT_TIMESTAMP - (max_age_days || ' days')::INTERVAL;

    GET DIAGNOSTICS deleted_count = ROW_COUNT;

    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

-- Function to get text file content by reconstructing from line mappings
CREATE OR REPLACE FUNCTION get_text_file_content(
    p_tenant_id UUID,
    p_inode_id BIGINT,
    p_layer_id UUID
)
RETURNS TEXT AS $$
DECLARE
    v_content TEXT := '';
    v_line_record RECORD;
    v_block_content TEXT;
    v_line_ending TEXT;
BEGIN
    -- Get line ending from metadata
    SELECT line_ending INTO v_line_ending
    FROM text_file_metadata
    WHERE tenant_id = p_tenant_id
      AND inode_id = p_inode_id
      AND layer_id = p_layer_id;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'Text file metadata not found';
    END IF;

    -- Convert line ending to actual characters
    v_line_ending := CASE v_line_ending
        WHEN 'LF' THEN E'\n'
        WHEN 'CRLF' THEN E'\r\n'
        WHEN 'CR' THEN E'\r'
    END;

    -- Reconstruct content from line mappings
    FOR v_line_record IN
        SELECT tlm.line_number, tb.content, tlm.block_line_offset
        FROM text_line_map tlm
        INNER JOIN text_blocks tb ON tlm.block_id = tb.block_id
        WHERE tlm.tenant_id = p_tenant_id
          AND tlm.inode_id = p_inode_id
          AND tlm.layer_id = p_layer_id
        ORDER BY tlm.line_number
    LOOP
        -- Extract specific line from block content
        v_block_content := split_part(v_line_record.content, E'\n', v_line_record.block_line_offset + 1);

        -- Append line with line ending
        IF v_content != '' THEN
            v_content := v_content || v_line_ending;
        END IF;
        v_content := v_content || v_block_content;
    END LOOP;

    RETURN v_content;
END;
$$ LANGUAGE plpgsql;

-- Comments for documentation
COMMENT ON TABLE text_blocks IS 'Content-addressed storage for text file lines with deduplication';
COMMENT ON COLUMN text_blocks.block_id IS 'Unique block identifier';
COMMENT ON COLUMN text_blocks.content_hash IS 'BLAKE3/SHA-256 hash of content for deduplication';
COMMENT ON COLUMN text_blocks.content IS 'Actual text content (single or multiple lines)';
COMMENT ON COLUMN text_blocks.line_count IS 'Number of lines in this block';
COMMENT ON COLUMN text_blocks.byte_size IS 'Size in bytes';
COMMENT ON COLUMN text_blocks.encoding IS 'Character encoding (UTF-8, ASCII, etc.)';
COMMENT ON COLUMN text_blocks.ref_count IS 'Number of references to this block';
COMMENT ON COLUMN text_blocks.last_accessed_at IS 'Last time this block was accessed';

COMMENT ON TABLE text_file_metadata IS 'Metadata for text files per layer';
COMMENT ON COLUMN text_file_metadata.total_lines IS 'Total number of lines in the file';
COMMENT ON COLUMN text_file_metadata.line_ending IS 'Line ending style (LF, CRLF, CR)';
COMMENT ON COLUMN text_file_metadata.has_trailing_newline IS 'Whether file ends with newline';

COMMENT ON TABLE text_line_map IS 'Maps line numbers to text blocks for file reconstruction';
COMMENT ON COLUMN text_line_map.line_number IS 'Logical line number (1-based)';
COMMENT ON COLUMN text_line_map.block_id IS 'Text block containing this line';
COMMENT ON COLUMN text_line_map.block_line_offset IS 'Line offset within the block (0-based)';

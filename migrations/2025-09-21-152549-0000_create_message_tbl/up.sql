-- create message table
CREATE TABLE IF NOT EXISTS message_tbl (
    id VARCHAR(255) PRIMARY KEY,
    channel_id VARCHAR(255) NOT NULL REFERENCES channel_tbl(id) ON DELETE CASCADE,
    -- when sender is deleted, set to NULL
    sender_id VARCHAR(255) NOT NULL REFERENCES user_tbl(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_message_created_at ON message_tbl(created_at DESC);
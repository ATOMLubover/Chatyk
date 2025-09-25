-- create channel table
CREATE TABLE IF NOT EXISTS channel_tbl (
    id VARCHAR(255) PRIMARY KEY,
    title TEXT NOT NULL,
    -- private: p2p chat between two users
    -- group: group chat with multiple users
    channel_type VARCHAR(63) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_channel_updated_at ON channel_tbl(updated_at DESC);

-- create channel members table
CREATE TABLE IF NOT EXISTS channel_member_tbl (
    channel_id VARCHAR(255) REFERENCES channel_tbl(id) ON DELETE CASCADE,
    user_id VARCHAR(255) REFERENCES user_tbl(id) ON DELETE CASCADE,
    joined_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    PRIMARY KEY (channel_id, user_id)
);
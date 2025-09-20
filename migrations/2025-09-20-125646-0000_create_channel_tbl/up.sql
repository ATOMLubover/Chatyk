-- define channel type
-- private: p2p chat between two users
-- group: group chat with multiple users
CREATE TYPE channel_type AS ENUM ('private', 'group');

-- create channel table
CREATE TABLE IF NOT EXISTS channel_tbl (
    id VARCHAR(255) PRIMARY KEY,
    chan_name TEXT,
    channel_type channel_type NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
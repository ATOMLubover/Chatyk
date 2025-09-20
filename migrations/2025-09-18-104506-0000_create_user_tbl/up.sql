-- create user table
CREATE TABLE IF NOT EXISTS user_tbl (
    id VARCHAR(255) PRIMARY KEY,

    username VARCHAR(50) UNIQUE NOT NULL,
    email VARCHAR(100) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX idx_user_username ON user_tbl(username);
CREATE UNIQUE INDEX idx_user_email ON user_tbl(email);
mod channel;
mod event;
mod message;

pub use channel::*;
pub use event::*;
pub use message::*;

use thiserror::Error;

use crate::cache::AsyncTypedCommands;
use crate::cache::CacheAsyncConn;

pub type ServiceResult<T> = Result<T, ServiceError>;

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("Close websocket normally")]
    CloseWebsocket,
    #[error("Axum error: {0}")]
    AxumError(#[from] axum::Error),
    #[error("Cache error: {0}")]
    CacheError(#[from] crate::cache::CacheError),
    #[error("Serialization error: {0}")]
    SerdeJsonError(#[from] serde_json::Error),
    #[error("Blocking join error: {0}")]
    BlockingJoinError(#[from] tokio::task::JoinError),
    #[error("Std IO error: {0}")]
    StdIoError(#[from] std::io::Error),
    #[error("Multipart resource upload error: {0}")]
    ResourceMultipartError(#[from] axum::extract::multipart::MultipartError),
    #[error("Database pool error: {0}")]
    DbPoolError(#[from] r2d2::Error),
    #[error("Database error: {0}")]
    DatabaseError(#[from] diesel::result::Error),
}

async fn lock_channel_message_cache(
    conn: &mut CacheAsyncConn,
    key: String,
    val: String,
    ttl: i64,
) -> ServiceResult<bool> {
    let lock = conn.set_nx(key.clone(), val).await?;

    // set an expiration time to avoid deadlock
    // FIXME: what if expire fails?
    conn.expire(key, ttl).await?;

    return Ok(lock);
}

async fn unlock_channel_message_cache(
    conn: &mut CacheAsyncConn,
    key: String,
    val: String,
) -> ServiceResult<()> {
    // use a Lua script to ensure atomicity
    let script = r#"
        if redis.call("get", KEYS[1]) == ARGV[1] then
            return redis.call("del", KEYS[1])
        else
            return 0
        end
    "#;

    // FIXME: what if invoke_async fails?
    redis::Script::new(script)
        .key(key)
        .arg(val)
        .invoke_async::<()>(conn)
        .await?;

    return Ok(());
}

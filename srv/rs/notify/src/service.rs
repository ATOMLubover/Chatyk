mod channel;
mod event;
mod message;

pub use channel::*;
pub use event::*;
pub use message::*;

use thiserror::Error;

use crate::MessageQueueClient;
use crate::cache;
use crate::cache::AsyncTypedCommands;
use crate::cache::CacheAsyncConn;
use crate::event::ServerEvent;

pub type ServiceResult<T> = Result<T, ServiceError>;

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("User is already in the channel")]
    UserAlreadyInChannel,
    #[error("User is not in the channel")]
    UserNotInChannel,
    #[error("Channel does not exist")]
    NonexistingChannel,
    #[error("Close websocket normally")]
    CloseWebsocket,
    #[error("Invalid message format")]
    InvalidMessage,
    #[error("Axum error: {0}")]
    AxumError(#[from] axum::Error),
    #[error("Redis error: {0}")]
    RedisError(#[from] redis::RedisError),
    #[error("Serialization error: {0}")]
    SerdeJsonError(#[from] serde_json::Error),
    #[error("Blocking join error: {0}")]
    BlockingJoinError(#[from] tokio::task::JoinError),
    #[error("Database pool error: {0}")]
    DatabasePoolError(#[from] r2d2::Error),
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
    cache::Script::new(script)
        .key(key)
        .arg(val)
        .invoke_async::<()>(conn)
        .await?;

    return Ok(());
}

async fn push_message_queue(
    message_queue: MessageQueueClient,
    event: ServerEvent,
) -> ServiceResult<()> {
    let payload = serde_json::to_string(&event)?;

    let mq_conn = &mut message_queue.get_async_conn().await?;

    mq_conn
        .xadd("chatyk:event_queue", "*", &[("payload", &payload)])
        .await?;

    return Ok(());
}

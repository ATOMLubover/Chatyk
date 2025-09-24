mod channel;
mod message;
mod resource;
mod user;

use anyhow::Result;
use redis::AsyncCommands;
use serde::Serialize;
use thiserror::Error;

pub use channel::*;
pub use message::*;
pub use resource::*;
pub use user::*;

use crate::cache::{CacheAsyncConn, CacheError};

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("Email of username already exists")]
    EmailOrUsernameConflict,

    #[error("User not found")]
    UserNotFound,

    #[error("Password mismatch")]
    PasswordMismatch,

    #[error("Unable to process password: {0}")]
    PasswordHashError(#[from] bcrypt::BcryptError),

    #[error("Invalid channel type")]
    InvalidChannelType,

    #[error("Invalid number of channel members, at least 2 members are required")]
    InvalidChannelMemberNumber,

    #[error("Failed to create channel")]
    ChannelCreationFailure,

    #[error("User is already in the channel")]
    UserAlreadyInChannel,

    #[error("User is not in the channel")]
    UserNotInChannel,

    #[error("Channel does not exist")]
    NonexistingChannel,

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

pub type ServiceResult<T> = Result<T, ServiceError>;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum PushEvent {
    MessageSpawned {
        message_id: String,
        channel_id: String,
        sender_id: String,
        content: String,
        created_at: String,
    },
    // client should refresh the channel member list
    // when receiving this two events
    UserJoinedChannel {
        channel_id: String,
    },
    UserLeftChannel {
        channel_id: String,
    },
}

async fn lock_channel_message_cache(
    conn: &mut CacheAsyncConn,
    key: String,
    val: String,
    ttl: i64,
) -> ServiceResult<bool> {
    let lock = conn
        .set_nx::<_, _, bool>(key.clone(), val)
        .await
        .map_err(|err| CacheError::from(err))?;

    // set an expiration time to avoid deadlock
    // FIXME: what if expire fails?
    conn.expire::<_, ()>(key, ttl)
        .await
        .map_err(|err| CacheError::from(err))?;

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
        .await
        .map_err(|err| CacheError::from(err))?;

    return Ok(());
}

mod channel;
mod message;
mod resource;
mod user;

use anyhow::Result;
use thiserror::Error;

pub use channel::*;
pub use message::*;
pub use resource::*;
pub use user::*;

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

    #[error("Blocking join error: {0}")]
    BlockingJoinError(#[from] tokio::task::JoinError),

    #[error("Std IO error: {0}")]
    StdIoError(#[from] std::io::Error),

    #[error("Resource upload error: {0}")]
    ResourceUploadError(String),

    #[error("Database pool error: {0}")]
    DbPoolError(#[from] r2d2::Error),

    #[error("Database error: {0}")]
    DatabaseError(#[from] diesel::result::Error),
}

pub type ServiceResult<T> = Result<T, ServiceError>;

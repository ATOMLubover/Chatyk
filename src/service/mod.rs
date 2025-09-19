mod user;

use thiserror::Error;

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

    #[error("Database pool error: {0}")]
    DbPoolError(#[from] r2d2::Error),

    #[error("Database error: {0}")]
    DatabaseError(#[from] diesel::result::Error),
}

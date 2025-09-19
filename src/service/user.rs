use anyhow::Result;
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error};

use crate::DbPool;
use crate::dto::{ReqRegisterUser, ReqUserLogin, RspUserInfo};
use crate::model::{NewUser, UserInfo};
use crate::service::ServiceError;
use crate::util;

pub async fn register_user(
    pool: &DbPool,
    request: ReqRegisterUser,
) -> Result<RspUserInfo, ServiceError> {
    use crate::schema::user_tbl::dsl::*;

    let hashed = bcrypt::hash(request.password, bcrypt::DEFAULT_COST)?;

    let user_id = util::generate_id().await;

    let new_user = NewUser {
        id: user_id,
        username: request.username,
        email: request.email,
        password_hash: hashed,
    };

    let conn = &mut pool.get()?;

    let user_info = match new_user
        .insert_into(user_tbl)
        .returning(UserInfo::as_returning())
        .get_result::<UserInfo>(conn)
    {
        Ok(user) => user,
        Err(err) => {
            if let Error::DatabaseError(DatabaseErrorKind::UniqueViolation, info) = err {
                tracing::trace!("Unique violation when register user: {:?}", info.message());
                return Err(ServiceError::EmailOrUsernameConflict);
            }

            return Err(ServiceError::DatabaseError(err));
        }
    };

    return Ok(RspUserInfo {
        id: user_info.id,
        username: user_info.username,
        email: user_info.email,
        created_at: user_info
            .created_at
            .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string()),
    });
}

pub async fn login_user(pool: &DbPool, request: ReqUserLogin) -> Result<RspUserInfo, ServiceError> {
    use crate::schema::user_tbl::dsl::*;

    let conn = &mut pool.get()?;

    let hashed_password = match user_tbl
        .filter(username.eq(&request.username))
        .select(password_hash)
        .first::<String>(conn)
    {
        Ok(user) => user,
        Err(err) => {
            if let Error::NotFound = err {
                tracing::trace!("User not found: {}", request.username);
                return Err(ServiceError::UserNotFound);
            }

            return Err(ServiceError::DatabaseError(err));
        }
    };

    if !bcrypt::verify(&request.password, &hashed_password)? {
        tracing::trace!("Invalid password for user: {}", request.username);
        return Err(ServiceError::PasswordMismatch);
    }

    let user_info = match user_tbl
        .filter(username.eq(&request.username))
        .select(UserInfo::as_select())
        .first::<UserInfo>(conn)
    {
        Ok(user) => user,
        Err(err) => {
            if let Error::NotFound = err {
                // log with warn level, this should rarely happen
                tracing::warn!(
                    "User not found after login unexpected: {}",
                    request.username
                );
                return Err(ServiceError::UserNotFound);
            }

            return Err(ServiceError::DatabaseError(err));
        }
    };

    return Ok(RspUserInfo {
        id: user_info.id,
        username: user_info.username,
        email: user_info.email,
        created_at: user_info
            .created_at
            .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string()),
    });
}

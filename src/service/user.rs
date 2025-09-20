use anyhow::Result;
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error};

use crate::DbPool;
use crate::dto::{ReqPatchUser, ReqRegisterUser, ReqUserLogin, RspUserInfo};
use crate::model::{NewUser, UserInfo};
use crate::service::{ServiceError, ServiceResult};
use crate::util;

pub async fn register_user(pool: &DbPool, request: ReqRegisterUser) -> ServiceResult<RspUserInfo> {
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
        created_at: user_info.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
    });
}

pub async fn login_user(pool: &DbPool, request: ReqUserLogin) -> ServiceResult<RspUserInfo> {
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
        created_at: user_info.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
    });
}

pub async fn patch_user(pool: &DbPool, user_id: &str, request: ReqPatchUser) -> ServiceResult<()> {
    use crate::schema::user_tbl::dsl::*;

    // a vitual tuple to hold the update values
    let mut update_tuple = (None, None, None);

    if let Some(new_username) = request.username {
        update_tuple.0 = Some(username.eq(new_username));
    }

    if let Some(new_email) = request.email {
        update_tuple.1 = Some(email.eq(new_email));
    }

    if let Some(new_password) = request.password {
        let hashed = bcrypt::hash(new_password, bcrypt::DEFAULT_COST)?;
        update_tuple.2 = Some(password_hash.eq(hashed));
    }

    let conn = &mut pool.get()?;

    if let Err(err) = diesel::update(user_tbl.filter(id.eq(user_id)))
        .set(update_tuple)
        .execute(conn)
    {
        if let Error::NotFound = err {
            tracing::trace!("User not found by id: {}", user_id);
            return Err(ServiceError::UserNotFound);
        }

        // possible unique violation
        if let Error::DatabaseError(DatabaseErrorKind::UniqueViolation, info) = err {
            tracing::trace!("Unique violation when patch user: {:?}", info.message());
            return Err(ServiceError::EmailOrUsernameConflict);
        }

        return Err(ServiceError::DatabaseError(err));
    }

    return Ok(());
}

pub async fn get_user_by_id(pool: &DbPool, user_id: &str) -> Result<RspUserInfo, ServiceError> {
    use crate::schema::user_tbl::dsl::*;

    let conn = &mut pool.get()?;

    let user_info = match user_tbl
        .filter(id.eq(user_id))
        .select(UserInfo::as_select())
        .first::<UserInfo>(conn)
    {
        Ok(user) => user,
        Err(err) => {
            if let Error::NotFound = err {
                tracing::trace!("User not found by id: {}", user_id);
                return Err(ServiceError::UserNotFound);
            }

            return Err(ServiceError::DatabaseError(err));
        }
    };

    return Ok(RspUserInfo {
        id: user_info.id,
        username: user_info.username,
        email: user_info.email,
        created_at: user_info.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
    });
}

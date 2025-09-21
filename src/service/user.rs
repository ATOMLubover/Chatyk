use anyhow::Result;
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error};
use tokio::task;

use crate::DbPool;
use crate::dto::{ReqPatchUser, ReqRegisterUser, ReqUserLogin, RspUserInfo};
use crate::model::{NewUser, UserInfo};
use crate::service::{ServiceError, ServiceResult};
use crate::util;

pub async fn register_user(pool: DbPool, req: ReqRegisterUser) -> ServiceResult<RspUserInfo> {
    use crate::schema::user_tbl::dsl::*;

    return task::spawn_blocking(move || {
        let conn = &mut pool.get()?;

        let hashed = bcrypt::hash(req.password, bcrypt::DEFAULT_COST)?;
        let user_id = util::generate_id();

        let new_user = NewUser {
            id: user_id,
            username: req.username,
            email: req.email,
            password_hash: hashed,
        };

        let user_info = match diesel::insert_into(user_tbl)
            .values(&new_user)
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

        Ok(RspUserInfo {
            id: user_info.id,
            username: user_info.username,
            email: user_info.email,
            created_at: user_info.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        })
    })
    .await?;
}

pub async fn login_user(pool: DbPool, req: ReqUserLogin) -> ServiceResult<RspUserInfo> {
    use crate::schema::user_tbl::dsl::*;

    return task::spawn_blocking(move || {
        let conn = &mut pool.get()?;

        let hashed_password = match user_tbl
            .filter(username.eq(&req.username))
            .select(password_hash)
            .first::<String>(conn)
        {
            Ok(user) => user,
            Err(err) => {
                if let Error::NotFound = err {
                    tracing::trace!("User not found: {}", req.username);
                    return Err(ServiceError::UserNotFound);
                }
                return Err(ServiceError::DatabaseError(err));
            }
        };

        if !bcrypt::verify(&req.password, &hashed_password)? {
            tracing::trace!("Invalid password for user: {}", req.username);
            return Err(ServiceError::PasswordMismatch);
        }

        let user_info = match user_tbl
            .filter(username.eq(&req.username))
            .select(UserInfo::as_select())
            .first::<UserInfo>(conn)
        {
            Ok(user) => user,
            Err(err) => {
                if let Error::NotFound = err {
                    tracing::warn!("User not found after login unexpected: {}", req.username);
                    return Err(ServiceError::UserNotFound);
                }
                return Err(ServiceError::DatabaseError(err));
            }
        };

        Ok(RspUserInfo {
            id: user_info.id,
            username: user_info.username,
            email: user_info.email,
            created_at: user_info.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        })
    })
    .await?;
}

pub async fn patch_user(pool: DbPool, req: ReqPatchUser) -> ServiceResult<()> {
    use crate::schema::user_tbl::dsl::*;

    return task::spawn_blocking(move || {
        let conn = &mut pool.get()?;
        let mut update_tuple = (None, None, None);

        if let Some(new_username) = req.username {
            update_tuple.0 = Some(username.eq(new_username));
        }

        if let Some(new_email) = req.email {
            update_tuple.1 = Some(email.eq(new_email));
        }

        if let Some(new_password) = req.password {
            let hashed = bcrypt::hash(new_password, bcrypt::DEFAULT_COST)?;
            update_tuple.2 = Some(password_hash.eq(hashed));
        }

        if let Err(err) = diesel::update(user_tbl.filter(id.eq(&req.user_id)))
            .set(update_tuple)
            .execute(conn)
        {
            if let Error::NotFound = err {
                tracing::trace!("User not found by id: {}", req.user_id);
                return Err(ServiceError::UserNotFound);
            }
            if let Error::DatabaseError(DatabaseErrorKind::UniqueViolation, info) = err {
                tracing::trace!("Unique violation when patch user: {:?}", info.message());
                return Err(ServiceError::EmailOrUsernameConflict);
            }
            return Err(ServiceError::DatabaseError(err));
        }

        return Ok(());
    })
    .await?;
}

pub async fn get_user_by_id(pool: DbPool, user_id: String) -> Result<RspUserInfo, ServiceError> {
    use crate::schema::user_tbl::dsl::*;

    return task::spawn_blocking(move || {
        let conn = &mut pool.get()?;

        let user_info = match user_tbl
            .filter(id.eq(user_id.clone()))
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

        Ok(RspUserInfo {
            id: user_info.id,
            username: user_info.username,
            email: user_info.email,
            created_at: user_info.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        })
    })
    .await?;
}

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use tokio::task;

use crate::DbPool;
use crate::dto::{
    ReqAddUserToChannel, ReqCreateChannel, ReqRemoveUserFromChannel, RspChannelInfo,
    RspChannelMember,
};
use crate::model::{ChannelInfo, NewChannel, NewChannelMember, UserInfo};
use crate::service::{ServiceError, ServiceResult};
use crate::util;

pub async fn create_channel(pool: DbPool, req: ReqCreateChannel) -> ServiceResult<()> {
    use crate::schema::channel_member_tbl;
    use crate::schema::channel_tbl;

    if req.member_ids.len() < 2 {
        return Err(ServiceError::InvalidChannelMemberNumber);
    }

    return task::spawn_blocking(move || {
        let conn = &mut pool.get()?;

        // use transaction in case adding members to channel not existing
        return conn.transaction(|conn| {
            // create a new channel first
            let channel_id = util::generate_id();

            let channel_name = match req.channel_name {
                Some(name) => name,
                _ => format!("Unnamed group channel # {}", &channel_id[..8]),
            };

            let new_channel = NewChannel {
                id: channel_id,
                chan_name: channel_name,
                // FIXME: validate channel type, as now we only do not use ENUM
                channel_type: match req.channel_type.as_str() {
                    "private" => "private".to_string(),
                    "public" => "public".to_string(),
                    _ => return Err(ServiceError::InvalidChannelType),
                },
                created_at: Utc::now(),
            };

            let new_channel_id = diesel::insert_into(channel_tbl::table)
                .values(&new_channel)
                .returning(channel_tbl::id)
                .get_result::<String>(conn)?;

            // then add members to the channel
            let members = req
                .member_ids
                .iter()
                .map(|member_id| NewChannelMember {
                    channel_id: new_channel_id.clone(),
                    user_id: member_id.to_string(),
                    joined_at: Utc::now(),
                })
                .collect::<Vec<_>>();

            return match diesel::insert_into(channel_member_tbl::table)
                .values(&members)
                .execute(conn)?
            {
                rows if rows == members.len() => Ok(()),
                _ => Err(ServiceError::ChannelCreationFailure),
            };
        });
    })
    .await?;
}

pub async fn list_user_channels(
    pool: DbPool,
    user_id: String,
    offset: i64,
    limit: i64,
) -> ServiceResult<Vec<RspChannelInfo>> {
    use crate::schema::channel_member_tbl;
    use crate::schema::channel_tbl;

    let user_id = user_id.to_string();

    return task::spawn_blocking(move || {
        let conn = &mut pool.get()?;

        let results = channel_tbl::table
            // filter channels by user_id first in order to reduce the number of rows to join
            .filter(channel_member_tbl::user_id.eq(user_id))
            .inner_join(
                channel_member_tbl::table.on(channel_tbl::id.eq(channel_member_tbl::channel_id)),
            )
            .select(ChannelInfo::as_select())
            .offset(offset)
            .limit(limit)
            .load::<ChannelInfo>(conn)?;

        return Ok(results
            .into_iter()
            .map(|c| RspChannelInfo {
                id: c.id,
                chan_name: c.chan_name,
                channel_type: c.channel_type,
                created_at: c.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                updated_at: c.updated_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            })
            .collect::<Vec<_>>());
    })
    .await?;
}

pub async fn list_channel_members(
    pool: DbPool,
    channel_id: String,
) -> ServiceResult<Vec<RspChannelMember>> {
    use crate::schema::channel_member_tbl;
    use crate::schema::user_tbl;

    return task::spawn_blocking(move || {
        let conn = &mut pool.get()?;

        let results = channel_member_tbl::table
            .filter(channel_member_tbl::channel_id.eq(&channel_id))
            .inner_join(user_tbl::table.on(channel_member_tbl::user_id.eq(user_tbl::id)))
            .select((UserInfo::as_select(), channel_member_tbl::joined_at))
            .load::<(UserInfo, DateTime<Utc>)>(conn)?;

        return Ok(results
            .into_iter()
            .map(|tuple| RspChannelMember {
                user_id: tuple.0.id,
                username: tuple.0.username,
                user_email: tuple.0.email,
                channel_id: channel_id.to_string(),
                joined_at: tuple.1.format("%Y-%m-%d %H:%M:%S").to_string(),
            })
            .collect::<Vec<_>>());
    })
    .await?;
}

pub async fn add_user_to_channel(pool: DbPool, req: ReqAddUserToChannel) -> ServiceResult<()> {
    use crate::schema::channel_member_tbl::dsl::*;
    use diesel::result::DatabaseErrorKind;
    use diesel::result::Error::DatabaseError;

    return task::spawn_blocking(move || {
        let conn = &mut pool.get()?;

        let new_member = NewChannelMember {
            channel_id: req.channel_id,
            user_id: req.user_id,
            joined_at: Utc::now(),
        };

        match diesel::insert_into(channel_member_tbl)
            .values(&new_member)
            .execute(conn)
        {
            Ok(_) => Ok(()),
            Err(err) => match err {
                // FIXME: user foreign key may be violated as well, but that rarely happens
                DatabaseError(DatabaseErrorKind::ForeignKeyViolation, _) => {
                    Err(ServiceError::NonexistingChannel)
                }
                DatabaseError(DatabaseErrorKind::UniqueViolation, _) => {
                    Err(ServiceError::UserAlreadyInChannel)
                }
                _ => Err(ServiceError::DatabaseError(err)),
            },
        }
    })
    .await?;
}

/// `remove_user_from_channel` removes a user from a channel
/// If the user is not in the channel, returns `ServiceError::UserNotInChannel`,
/// but the function is idempotent
pub async fn remove_user_from_channel(
    pool: DbPool,
    req: ReqRemoveUserFromChannel,
) -> ServiceResult<()> {
    use crate::schema::channel_member_tbl::dsl::*;

    return task::spawn_blocking(move || {
        let conn = &mut pool.get()?;

        return match diesel::delete(
            channel_member_tbl
                .filter(channel_id.eq(&req.channel_id))
                .filter(user_id.eq(&req.user_id)),
        )
        .execute(conn)?
        {
            rows if rows == 0 => Err(ServiceError::UserNotInChannel),
            _ => Ok(()),
        };
    })
    .await?;
}

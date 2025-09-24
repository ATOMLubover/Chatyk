use std::sync::Arc;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use diesel::prelude::*;
use redis::{AsyncCommands, Commands};
use tokio::{sync::broadcast::Sender, task};

use crate::{
    CacheCli, DbPool,
    cache::CacheError,
    dto::{
        ReqAddUserToChannel, ReqCreateChannel, ReqRemoveUserFromChannel, RspChannelInfo,
        RspChannelMember,
    },
    model::{ChannelInfo, NewChannel, NewChannelMember, UserInfo},
    service::{PushEvent, ServiceError, ServiceResult},
    util,
};

pub async fn create_channel(
    pool: DbPool,
    cache: CacheCli,
    online_users: Arc<DashMap<String, Sender<PushEvent>>>,
    req: ReqCreateChannel,
) -> ServiceResult<()> {
    use crate::schema::channel_member_tbl;
    use crate::schema::channel_tbl;

    if req.member_ids.len() < 2 {
        return Err(ServiceError::InvalidChannelMemberNumber);
    }

    let pool_clone = pool.clone();
    let member_ids_clone = req.member_ids.clone();

    let new_channel_id = task::spawn_blocking(move || {
        let conn = &mut pool_clone.get()?;

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
                title: channel_name,
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
                rows if rows == members.len() => Ok(new_channel_id),
                _ => Err(ServiceError::ChannelCreationFailure),
            };
        });
    })
    .await??;

    // list the members of the newly created channel,
    // which populates the cache implicitly
    list_channel_members(pool, cache, new_channel_id.clone()).await?;

    // notify other members in the channel to refresh their member list
    tokio::spawn(async move {
        for member_id in member_ids_clone.into_iter() {
            // broadcast to related user
            if let Some(tx) = online_users.get(&member_id) {
                if let Err(err) = tx.send(PushEvent::UserJoinedChannel {
                    channel_id: new_channel_id.clone(),
                }) {
                    tracing::error!("Failed to send push event to user {}: {}", member_id, err);
                }
            }
        }
    });

    return Ok(());
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
                chan_name: c.title,
                channel_type: c.channel_type,
                created_at: c.created_at.to_rfc3339().to_string(),
                updated_at: c.updated_at.to_rfc3339().to_string(),
            })
            .collect::<Vec<_>>());
    })
    .await?;
}

pub async fn list_channel_members(
    pool: DbPool,
    cache: CacheCli,
    channel_id: String,
) -> ServiceResult<Vec<RspChannelMember>> {
    use crate::schema::channel_member_tbl;
    use crate::schema::user_tbl;

    // try getting from cache first
    let channel_key = format!("channel:{}:members", &channel_id);

    let cache_conn = &mut cache.get_async_conn().await?;

    let members = cache_conn
        .get::<_, Option<String>>(channel_key.clone())
        .await
        .map_err(|err| CacheError::from(err))?;

    if let Some(members) = members {
        // cache hit, deserialize and return
        let rsp_members = serde_json::from_str::<Vec<RspChannelMember>>(&members)?;

        return Ok(rsp_members);
    }

    // cache miss, query from database and populate the cache
    return task::spawn_blocking(move || {
        let conn = &mut pool.get()?;

        let results = channel_member_tbl::table
            .filter(channel_member_tbl::channel_id.eq(&channel_id))
            .inner_join(user_tbl::table.on(channel_member_tbl::user_id.eq(user_tbl::id)))
            .select((UserInfo::as_select(), channel_member_tbl::joined_at))
            .load::<(UserInfo, DateTime<Utc>)>(conn)?;

        let members = results
            .into_iter()
            .map(|tuple| RspChannelMember {
                user_id: tuple.0.id,
                username: tuple.0.username,
                user_email: tuple.0.email,
                channel_id: channel_id.to_string(),
                joined_at: tuple.1.to_rfc3339().to_string(),
            })
            .collect::<Vec<_>>();

        // serialize and populate the cache
        let serialized = serde_json::to_string(&members)?;

        let cache_conn = &mut cache.get_sync_conn()?;

        // ignore error here
        let _ = cache_conn
            .set_ex::<_, _, ()>(channel_key, serialized, 24 * 60 * 60)
            .map_err(|err| {
                tracing::error!(
                    "Failed to set channel members cache: {}",
                    CacheError::from(err)
                );
            });

        return Ok(members);
    })
    .await?;
}

pub async fn add_user_to_channel(
    pool: DbPool,
    cache: CacheCli,
    online_users: Arc<DashMap<String, Sender<PushEvent>>>,
    req: ReqAddUserToChannel,
) -> ServiceResult<()> {
    use crate::schema::channel_member_tbl::dsl::*;
    use diesel::result::DatabaseErrorKind;
    use diesel::result::Error::DatabaseError;

    let channel_id_clone = req.channel_id.clone();
    let user_id_clone = req.user_id.clone();

    task::spawn_blocking(move || {
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
    .await??;

    // invalidate the channel member cache
    let cache_key = format!("channel:{}:members", &channel_id_clone);

    let cache_conn = &mut cache.get_async_conn().await?;

    cache_conn
        .del::<_, ()>(cache_key)
        .await
        .map_err(|err| CacheError::from(err))?;

    // notify other members in the channel to refresh their member list
    tokio::spawn(async move {
        // broadcast to related user
        // FIXME: the inviter user should refresh by himself after get OK response
        if let Some(tx) = online_users.get(&user_id_clone) {
            if let Err(err) = tx.send(PushEvent::UserJoinedChannel {
                channel_id: channel_id_clone,
            }) {
                tracing::error!(
                    "Failed to send push event to user {}: {}",
                    user_id_clone,
                    err
                );
            }
        }
    });

    return Ok(());
}

/// `remove_user_from_channel` removes a user from a channel
/// If the user is not in the channel, returns `ServiceError::UserNotInChannel`,
/// but the function is idempotent
pub async fn remove_user_from_channel(
    pool: DbPool,
    cache: CacheCli,
    online_users: Arc<DashMap<String, Sender<PushEvent>>>,
    req: ReqRemoveUserFromChannel,
) -> ServiceResult<()> {
    use crate::schema::channel_member_tbl::dsl::*;

    let channel_id_clone = req.channel_id.clone();
    let user_id_clone = req.user_id.clone();

    // TODO: delete the channel if there is no member in it anymore
    task::spawn_blocking(move || {
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
    .await??;

    // invalidate the channel member cache
    let cache_key = format!("channel:{}:members", &channel_id_clone);

    let cache_conn = &mut cache.get_async_conn().await?;

    cache_conn
        .del::<_, ()>(cache_key)
        .await
        .map_err(|err| CacheError::from(err))?;

    // notify other members in the channel to refresh their member list
    tokio::spawn(async move {
        // broadcast to related user
        if let Some(tx) = online_users.get(&user_id_clone) {
            if let Err(err) = tx.send(PushEvent::UserLeftChannel {
                channel_id: channel_id_clone,
            }) {
                tracing::error!(
                    "Failed to send push event to user {}: {}",
                    user_id_clone,
                    err
                );
            }
        }
    });

    return Ok(());
}

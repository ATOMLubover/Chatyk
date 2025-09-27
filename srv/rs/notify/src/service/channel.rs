use chrono::{DateTime, Utc};
use futures_util::{StreamExt, stream};
use shared::model::{NewChannelMember, UserInfo};

use crate::cache::{AsyncTypedCommands, TypedCommands};
use crate::dto::{ReqUserJoinChannel, ReqUserLeaveChannel, RspChannelMember};
use crate::event::ServerEvent;
use crate::handler::OnlineUsersMap;
use crate::service::{self, ServiceError, ServiceResult};
use crate::{CacheClient, DatabasePool, MessageQueueClient};

pub async fn list_channel_members(
    pool: DatabasePool,
    cache: CacheClient,
    channel_id: String,
) -> ServiceResult<Vec<RspChannelMember>> {
    use diesel::prelude::*;
    use shared::schema::channel_member_tbl;
    use shared::schema::user_tbl;

    // try getting from cache first
    let channel_key = format!("channel:{}:members", &channel_id);

    let cache_conn = &mut cache.get_async_conn().await?;

    let members = cache_conn.mget(channel_key.clone()).await?;

    if members.len() > 0 {
        // cache hit, deserialize and return
        let rsp_members: Vec<RspChannelMember> = members
            .into_iter()
            .filter_map(|item| item)
            .filter_map(|item| serde_json::from_str::<Vec<RspChannelMember>>(&item).ok())
            .flatten()
            .collect();

        return Ok(rsp_members);
    }

    // cache miss, query from database and populate the cache
    return tokio::task::spawn_blocking(move || {
        let conn = &mut pool.get()?;

        let results = channel_member_tbl::table
            .filter(channel_member_tbl::channel_id.eq(&channel_id))
            .inner_join(user_tbl::table.on(channel_member_tbl::user_id.eq(user_tbl::id)))
            .select((UserInfo::as_select(), channel_member_tbl::joined_at))
            .load::<(UserInfo, DateTime<Utc>)>(conn)?;

        let members: Vec<RspChannelMember> = results
            .into_iter()
            .map(|tuple| RspChannelMember {
                user_id: tuple.0.id,
                username: tuple.0.username,
                channel_id: channel_id.to_string(),
                joined_at: tuple.1.to_rfc3339().to_string(),
            })
            .collect();

        // serialize and populate the cache
        let payload = serde_json::to_string(&members)?;

        let cache_conn = &mut cache.get_sync_conn()?;

        // FIXME: if failed, retry twice
        // it is not promised that the cache will be consistent with database
        for i in 0..2 {
            if let Ok(_) = cache_conn.set_ex(channel_key.clone(), &payload, 24 * 60 * 60) {
                break;
            }

            tracing::error!(
                "Failed to set channel members cache, retrying {} time(s)...",
                i + 1
            );
        }

        return Ok(members);
    })
    .await?;
}

pub async fn add_user_to_channel(
    database: DatabasePool,
    cache: CacheClient,
    message_queue: MessageQueueClient,
    online_users: OnlineUsersMap,
    req: ReqUserJoinChannel,
) -> ServiceResult<()> {
    use diesel::prelude::*;
    use diesel::result::DatabaseErrorKind;
    use diesel::result::Error::DatabaseError;
    use shared::schema::channel_member_tbl::dsl::*;

    let channel_id_clone = req.channel_id.clone();
    let user_id_clone = req.user_id.clone();
    let database_clone = database.clone();

    tokio::task::spawn_blocking(move || {
        let conn = &mut database_clone.get()?;

        let new_member = NewChannelMember {
            channel_id: channel_id_clone,
            user_id: user_id_clone,
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
    let cache_key = format!("channel:{}:members", &req.channel_id);

    let cache_conn = &mut cache.get_async_conn().await?;

    cache_conn.del(cache_key).await?;

    // notify other members in the channel to refresh their member list
    tokio::spawn(async move {
        // broadcast to related user
        // FIXME: the inviter user should refresh by himself after get OK response
        let channel_members =
            match service::list_channel_members(database, cache, req.channel_id.clone()).await {
                Err(err) => {
                    tracing::error!("Failed to list channel members in create_message: {}", err);
                    return;
                }
                Ok(members) => members,
            };

        let event = ServerEvent::UserLeaveChannel {
            user_id: req.user_id,
            channel_id: req.channel_id,
        };

        // if the user is connected to this server instance, send the event directly
        let preserved_members = stream::iter(channel_members)
            .filter_map(|member| {
                let online_users = online_users.clone();
                let event = event.clone();

                return async move {
                    if let Some(tx) = online_users.get(&member.user_id) {
                        match tx.send(event).await {
                            Ok(_) => {
                                return None;
                            }
                            Err(err) => {
                                tracing::error!(
                                    "Failed to push event to user {}: {}",
                                    &member.user_id,
                                    err
                                );
                                return None;
                            }
                        }
                    }

                    return Some(member);
                };
            })
            .collect::<Vec<_>>()
            .await;

        // else, we send the event to message queue for other server instances to pick up
        if preserved_members.len() == 0 {
            return;
        }

        if let Err(err) = super::push_message_queue(message_queue.clone(), event).await {
            tracing::error!("Failed to push event to MQ: {}", err);
        }
    });

    return Ok(());
}

/// `remove_user_from_channel` removes a user from a channel
/// If the user is not in the channel, returns `ServiceError::UserNotInChannel`,
/// but the function is idempotent
pub async fn remove_user_from_channel(
    database: DatabasePool,
    cache: CacheClient,
    message_queue: MessageQueueClient,
    online_users: OnlineUsersMap,
    req: ReqUserLeaveChannel,
) -> ServiceResult<()> {
    use diesel::prelude::*;
    use shared::schema::channel_member_tbl::dsl::*;

    let channel_id_clone = req.channel_id.clone();
    let user_id_clone = req.user_id.clone();
    let database_clone = database.clone();

    // TODO: delete the channel if there is no member in it anymore
    tokio::task::spawn_blocking(move || {
        let conn = &mut database_clone.get()?;

        return match diesel::delete(
            channel_member_tbl
                .filter(channel_id.eq(&channel_id_clone.clone()))
                .filter(user_id.eq(&user_id_clone.clone())),
        )
        .execute(conn)?
        {
            rows if rows == 0 => Err(ServiceError::UserNotInChannel),
            _ => Ok(()),
        };
    })
    .await??;

    // invalidate the channel member cache
    let cache_key = format!("channel:{}:members", &req.channel_id);

    let cache_conn = &mut cache.get_async_conn().await?;

    cache_conn.del(cache_key).await?;

    // notify other members in the channel to refresh their member list
    tokio::spawn(async move {
        let channel_members =
            match service::list_channel_members(database, cache, req.channel_id.clone()).await {
                Err(err) => {
                    tracing::error!("Failed to list channel members in create_message: {}", err);
                    return;
                }
                Ok(members) => members,
            };

        let event = ServerEvent::UserLeaveChannel {
            user_id: req.user_id,
            channel_id: req.channel_id,
        };

        // if the user is connected to this server instance, send the event directly
        let preserved_members = stream::iter(channel_members)
            .filter_map(|member| {
                let online_users = online_users.clone();
                let event = event.clone();

                return async move {
                    if let Some(tx) = online_users.get(&member.user_id) {
                        match tx.send(event).await {
                            Ok(_) => {
                                return None;
                            }
                            Err(err) => {
                                tracing::error!(
                                    "Failed to push event to user {}: {}",
                                    &member.user_id,
                                    err
                                );
                                return None;
                            }
                        }
                    }

                    return Some(member);
                };
            })
            .collect::<Vec<_>>()
            .await;

        // else, we send the event to message queue for other server instances to pick up
        if preserved_members.len() == 0 {
            return;
        }

        if let Err(err) = super::push_message_queue(message_queue.clone(), event).await {
            tracing::error!("Failed to push event to MQ: {}", err);
        }
    });

    return Ok(());
}

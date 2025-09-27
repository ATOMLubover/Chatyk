use std::collections::HashSet;

use chrono::Utc;
use shared::model::NewMessage;
use shared::util;

use crate::cache::{AsyncTypedCommands, TypedCommands};
use crate::dto::{ReqSendMessage, RspMessage};
use crate::event::ServerEvent;
use crate::handler::OnlineUsersMap;
use crate::service::{self, ServiceError, ServiceResult, channel};
use crate::{CacheClient, DatabasePool};

pub async fn create_message(
    pool: DatabasePool,
    cache: CacheClient,
    online_users: OnlineUsersMap,
    req: ReqSendMessage,
) -> ServiceResult<()> {
    use diesel::prelude::*;
    use shared::schema::message_tbl::dsl::*;

    let pool_clone = pool.clone();
    let cache_clone = cache.clone();

    let msg_id = shared::util::generate_id();

    // TODO: validate the resources in the content
    let new_message = NewMessage {
        id: msg_id,
        channel_id: req.channel_id.clone(),
        sender_id: req.sender_id.clone(),
        content: req.content,
        created_at: Utc::now(),
    };

    let new_message_clone = new_message.clone();

    // insert the new message into database first
    tokio::task::spawn_blocking(move || {
        let conn = &mut pool.get()?;

        // we need a strong consistency with cache here, so we use a transaction
        return conn.transaction::<_, ServiceError, _>(|conn| {
            diesel::insert_into(message_tbl)
                .values(&new_message.clone())
                .execute(conn)?;

            let msg_val = serde_json::to_string(&ServerEvent::SpawnMessage {
                channel_id: new_message.channel_id.clone(),
                sender_id: new_message.sender_id,
                content: new_message.content,
                created_at: new_message.created_at.clone(),
            })?;

            let channel_key = format!("channel:{}:recent_messages", new_message.channel_id);

            let cache_conn = &mut cache.get_sync_conn()?;

            cache_conn.lpush(channel_key, msg_val)?;

            return Ok(());
        });
    })
    .await??;

    // notify online users in the channel in background
    tokio::spawn(async move {
        let mut channel_members =
            match channel::list_channel_members(pool_clone, cache_clone.clone(), req.channel_id)
                .await
            {
                Err(err) => {
                    tracing::error!("Failed to list channel members in create_message: {}", err);
                    return;
                }
                Ok(members) => members,
            };

        let ev = ServerEvent::SpawnMessage {
            sender_id: new_message_clone.sender_id,
            channel_id: new_message_clone.channel_id,
            content: new_message_clone.content,
            created_at: new_message_clone.created_at,
        };

        // if the user is connected to this server instance, send the event directly
        channel_members.retain(|member| {
            if let Some(tx) = online_users.get(&member.user_id) {
                if let Err(err) = tx.send(ev.clone()) {
                    // FIXME: we do not retry as now, just log the error
                    tracing::error!("Failed to push event to user {}: {}", &member.user_id, err);
                }
                return true;
            }
            // as long as the user is not connected to this server instance, we consider it offline
            return false;
        });

        // else, we send the event to message queue for other server instances to pick up
        if channel_members.len() == 0 {
            return;
        }

        let payload = match serde_json::to_string(&ev) {
            Err(err) => {
                tracing::error!("Failed to serialize event when pushing MQ: {}", err);
                return;
            }
            Ok(payload) => payload,
        };

        let cache_conn = &mut match cache_clone.get_async_conn().await {
            Err(err) => {
                tracing::error!(
                    "Failed to get async cache connection when pushing MQ: {}",
                    err
                );
                return;
            }
            Ok(conn) => conn,
        };

        // FIXME: this is a crucial operation, we need to retry a few times if failed
        match cache_conn
            .xadd("event_queue", "*", &[("payload", &payload)])
            .await
        {
            Err(err) => {
                tracing::error!("Failed to push event to message queue: {}", err);
            }
            Ok(_) => {
                tracing::debug!("Pushed event to message queue successfully");
            }
        };
    });

    return Ok(());
}

pub async fn list_channel_messages(
    pool: DatabasePool,
    channel_id: String,
    offset: i64,
    limit: i64,
) -> ServiceResult<Vec<RspMessage>> {
    use diesel::prelude::*;
    use shared::model::MessageInfo;
    use shared::schema::message_tbl;

    return tokio::task::spawn_blocking(move || {
        let conn = &mut pool.get()?;

        let msgs = message_tbl::table
            .filter(message_tbl::channel_id.eq(&channel_id))
            .order(message_tbl::created_at.desc())
            .offset(offset)
            .limit(limit)
            .load::<MessageInfo>(conn)?;

        let rsp_msgs = msgs
            .into_iter()
            .map(|msg| RspMessage {
                message_id: msg.id,
                channel_id: msg.channel_id,
                sender_id: msg.sender_id,
                content: msg.content,
                created_at: msg.created_at,
            })
            .collect::<Vec<_>>();

        return Ok(rsp_msgs);
    })
    .await?;
}

/// a shortcut to list recent messages across all channels
/// this will be useful for cache acceleration
pub async fn list_channel_recent_messages(
    pool: DatabasePool,
    cache: CacheClient,
    channel_id: String,
    limit: i64,
) -> ServiceResult<Vec<RspMessage>> {
    let conn = &mut cache.get_async_conn().await?;

    let channel_key = format!("channel:{}:recent_messages", channel_id);

    let cache_msgs = conn.lrange(channel_key, 0, (limit - 1) as isize).await?;

    // cache hit
    if cache_msgs.len() > 0 {
        tracing::trace!("Cache hit for recent messages in channel {}", channel_id);

        // if we have enough messages in cache, return directly
        if cache_msgs.len() as i64 >= limit {
            return Ok(cache_msgs
                .into_iter()
                .map(|s| {
                    serde_json::from_str(&s).unwrap_or_else(|err| {
                        // it is not likely to happen, just log it
                        tracing::error!(
                            "Failed to deserialize message from cache: {}, error: {}",
                            s,
                            err
                        );

                        RspMessage {
                            message_id: "".to_string(),
                            channel_id: "".to_string(),
                            sender_id: "".to_string(),
                            content: "".to_string(),
                            created_at: Utc::now(),
                        }
                    })
                })
                // skip the corrupted ones
                .filter(|msg| !msg.message_id.is_empty())
                .collect());
        }

        // otherwise, we will fetch from database later
        let mut msgs = list_channel_messages(
            pool.clone(),
            channel_id.clone(),
            // fetch more to avoid new messages arriving during the process
            std::cmp::max(cache_msgs.len() as i64 - 10, 0),
            // as well, fetch a little more than needed
            limit - cache_msgs.len() as i64 + 10,
        )
        .await?;

        // merge the cache and database results, remove duplicates
        cache_msgs
            .into_iter()
            .for_each(|s| match serde_json::from_str(&s) {
                Ok(msg) => msgs.push(msg),
                Err(err) => {
                    tracing::error!(
                        "Failed to deserialize message from cache: {}, error: {}",
                        s,
                        err
                    )
                }
            });

        let mut seen = HashSet::new();

        msgs.retain(|msg| seen.insert(msg.message_id.clone()));

        return Ok(msgs);
    }

    // cache miss, load from database with a lock
    tracing::trace!("Cache miss for recent messages in channel {}", channel_id);

    let lock_key = format!("lock:channel:{}:recent_messages", channel_id);
    let uuid_val = util::generate_id();

    if service::lock_channel_message_cache(conn, lock_key.clone(), uuid_val.clone(), 5).await?
        == false
    {
        // someone else is updating the cache, wait a moment and try to get from cache again
        // we will try for 10 times, each time wait for 100ms
        for i in 0..10 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            let msgs = conn
                .lrange(
                    format!("channel:{}:recent_messages", channel_id),
                    0,
                    (limit - 1) as isize,
                )
                .await?;

            if !msgs.is_empty() {
                return Ok(msgs
                    .into_iter()
                    .map(|s| {
                        serde_json::from_str(&s).unwrap_or_else(|err| {
                            // it is not likely to happen, just log it
                            tracing::error!(
                                "Failed to deserialize message from cache: {}, error: {}",
                                s,
                                err
                            );

                            RspMessage {
                                message_id: "".to_string(),
                                channel_id: "".to_string(),
                                sender_id: "".to_string(),
                                content: "Corrupted message".to_string(),
                                created_at: Utc::now(),
                            }
                        })
                    })
                    .collect());
            }

            tracing::trace!(
                "Retrying to get recent messages from cache for channel {}, attempt {}",
                channel_id,
                i + 1
            );
        }

        // still not found, fall back to database
        // FIXME: this may cause thundering herd problem, need a better solution
        tracing::trace!(
            "Failed to get recent messages from cache for channel {} after retries, falling back to database",
            channel_id
        );

        let msgs = list_channel_messages(pool.clone(), channel_id.clone(), 0, limit).await?;

        return Ok(msgs);
    }

    // successfully acquired the lock, load from database and update the cache
    let msgs = list_channel_messages(pool.clone(), channel_id.clone(), 0, limit).await?;

    if !msgs.is_empty() {
        // clear the old cache first
        conn.del(format!("channel:{}:recent_messages", channel_id))
            .await?;

        let serialized_msgs = msgs
            .iter()
            .map(|msg| {
                serde_json::to_string(msg).unwrap_or_else(|err| {
                    // it is not likely to happen, just log it
                    tracing::error!(
                        "Failed to serialize message for cache: {:?}, error: {}",
                        msg,
                        err
                    );

                    return "".to_string();
                })
            })
            // skip the corrupted ones
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();

        // rebuild the cache and set an expiration time of 24 hour
        conn.rpush(
            format!("channel:{}:recent_messages", channel_id),
            serialized_msgs,
        )
        .await?;

        conn.expire(
            format!("channel:{}:recent_messages", channel_id),
            24 * 60 * 60,
        )
        .await?;
    }

    // release the lock
    service::unlock_channel_message_cache(conn, lock_key, uuid_val).await?;

    return Ok(msgs);
}

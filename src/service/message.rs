use std::{collections::HashSet, sync::Arc};

use chrono::Utc;
use dashmap::DashMap;
use diesel::prelude::*;
use redis::{AsyncCommands, Commands};
use tokio::{sync::broadcast::Sender, task};

use crate::{
    CacheCli, DbPool,
    cache::CacheError,
    dto::{ReqSendMessage, RspMessage},
    model::{MessageInfo, NewMessage},
    service::{self, PushEvent, ServiceError, ServiceResult, channel},
    util,
};

pub async fn create_message(
    pool: DbPool,
    cache: CacheCli,
    online_users: Arc<DashMap<String, Sender<PushEvent>>>,
    req: ReqSendMessage,
) -> ServiceResult<()> {
    use crate::schema::message_tbl::dsl::*;

    let pool_clone = pool.clone();
    let cache_clone = cache.clone();

    let msg_id = util::generate_id();

    // TODO: validate the resources in the content
    let new_message = NewMessage {
        id: msg_id,
        channel_id: req.channel_id.clone(),
        sender_id: req.sender_id,
        content: req.content,
        created_at: Utc::now(),
    };

    let new_message_clone = new_message.clone();

    // insert the new message into database first
    task::spawn_blocking(move || {
        let conn = &mut pool.get()?;

        // we need a strong consistency with cache here, so we use a transaction
        return conn.transaction::<_, ServiceError, _>(|conn| {
            diesel::insert_into(message_tbl)
                .values(&new_message.clone())
                .execute(conn)?;

            let msg_val = serde_json::to_string(&RspMessage {
                id: new_message.id,
                channel_id: new_message.channel_id.clone(),
                sender_id: new_message.sender_id,
                content: new_message.content,
                created_at: new_message.created_at.to_rfc3339().to_string(),
            })?;

            let channel_key = format!("channel:{}:recent_messages", new_message.channel_id);

            let cache_conn = &mut cache.get_sync_conn()?;

            cache_conn
                .lpush::<_, _, ()>(channel_key, msg_val)
                .map_err(|err| CacheError::from(err))?;

            return Ok(());
        });
    })
    .await??;

    // notify online users in the channel in background
    tokio::spawn(async move {
        let channel_members =
            match channel::list_channel_members(pool_clone, cache_clone, req.channel_id).await {
                Ok(members) => members,
                Err(err) => {
                    tracing::error!("Failed to list channel members in create_message: {}", err);
                    return;
                }
            };

        for member in channel_members.into_iter() {
            if let Some(tx) = online_users.get(&member.user_id) {
                if let Err(err) = tx.send(PushEvent::MessageSpawned {
                    message_id: new_message_clone.id.clone(),
                    channel_id: new_message_clone.channel_id.clone(),
                    sender_id: new_message_clone.sender_id.clone(),
                    content: new_message_clone.content.clone(),
                    created_at: new_message_clone.created_at.to_rfc3339().to_string(),
                }) {
                    tracing::error!(
                        "Failed to send PushEvent to user {}: {}",
                        &member.user_id,
                        err
                    );
                }
            }
        }
    });

    return Ok(());
}

pub async fn list_channel_messages(
    pool: DbPool,
    channel_id: String,
    offset: i64,
    limit: i64,
) -> ServiceResult<Vec<RspMessage>> {
    use crate::schema::message_tbl;

    return task::spawn_blocking(move || {
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
                id: msg.id,
                channel_id: msg.channel_id,
                sender_id: msg.sender_id,
                content: msg.content,
                created_at: msg.created_at.to_rfc3339().to_string(),
            })
            .collect::<Vec<_>>();

        return Ok(rsp_msgs);
    })
    .await?;
}

/// a shortcut to list recent messages across all channels
/// this will be useful for cache acceleration
pub async fn list_channel_recent_messages(
    pool: DbPool,
    cache: CacheCli,
    channel_id: String,
    limit: i64,
) -> ServiceResult<Vec<RspMessage>> {
    let conn = &mut cache.get_async_conn().await?;

    let channel_key = format!("channel:{}:recent_messages", channel_id);

    let cache_msgs = conn
        .lrange::<_, Vec<String>>(channel_key, 0, (limit - 1) as isize)
        .await
        .map_err(|err| CacheError::from(err))?;

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
                            id: "".to_string(),
                            channel_id: "".to_string(),
                            sender_id: "".to_string(),
                            content: "".to_string(),
                            created_at: "".to_string(),
                        }
                    })
                })
                // skip the corrupted ones
                .filter(|msg| !msg.id.is_empty())
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

        msgs.retain(|msg| seen.insert(msg.id.clone()));

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
                .lrange::<_, Vec<String>>(
                    format!("channel:{}:recent_messages", channel_id),
                    0,
                    (limit - 1) as isize,
                )
                .await
                .map_err(|err| CacheError::from(err))?;

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
                                id: "".to_string(),
                                channel_id: "".to_string(),
                                sender_id: "".to_string(),
                                content: "Corrupted message".to_string(),
                                created_at: "".to_string(),
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
        conn.del::<_, ()>(format!("channel:{}:recent_messages", channel_id))
            .await
            .map_err(|err| CacheError::from(err))?;

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
        conn.rpush::<_, _, ()>(
            format!("channel:{}:recent_messages", channel_id),
            serialized_msgs,
        )
        .await
        .map_err(|err| CacheError::from(err))?;

        conn.expire::<_, ()>(
            format!("channel:{}:recent_messages", channel_id),
            24 * 60 * 60,
        )
        .await
        .map_err(|err| CacheError::from(err))?;
    }

    // release the lock
    service::unlock_channel_message_cache(conn, lock_key, uuid_val).await?;

    return Ok(msgs);
}

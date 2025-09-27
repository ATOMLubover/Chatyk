use chrono::{DateTime, Utc};
use shared::model::UserInfo;

use crate::cache::{AsyncTypedCommands, TypedCommands};
use crate::dto::RspChannelMember;
use crate::service::ServiceResult;
use crate::{CacheClient, DatabasePool};

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

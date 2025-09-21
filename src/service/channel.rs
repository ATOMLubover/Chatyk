use chrono::Utc;
use diesel::prelude::*;
use tokio::task;

use crate::dto::{ReqAddUserToChannel, ReqCreateChannel, ReqRemoveUserFromChannel, RspChannelInfo};
use crate::model::{ChannelInfo, NewChannel, NewChannelMember, user};
use crate::service::{ServiceError, ServiceResult};
use crate::{DbPool, util};

pub async fn create_channel(pool: DbPool, req: ReqCreateChannel) -> ServiceResult<()> {
    use crate::schema::channel_member_tbl;
    use crate::schema::channel_tbl;

    if req.members.len() < 2 {
        return Err(ServiceError::InvalidChannelMemberNumber);
    }

    return task::spawn_blocking(move || {
        let conn = &mut pool.get()?;

        // use transaction in case adding members to channel not existing
        return conn.transaction(|conn| {
            // create a new channel first
            let channel_id = util::generate_id();

            let channel_name = match req.chan_name {
                Some(name) => name,
                _ if req.members.len() == 2 => {
                    // a default channel name is composed of the two members' names
                    format!(
                        "{}, {}'s channel",
                        req.members[0].member_name.clone(),
                        req.members[1].member_name.clone()
                    )
                }
                _ => format!("Unnamed group channel # {}", &channel_id[..8]),
            };

            let new_channel = NewChannel {
                id: channel_id,
                chan_name: channel_name,
                // FIXME: validate channel type, as now we only do not use ENUM
                channel_type: match req.channel_type.as_str() {
                    "private" => "private".to_string(),
                    "public" => "public".to_string(),
                    ty => return Err(ServiceError::InvalidChannelType(ty.to_string())),
                },
                created_at: Utc::now(),
            };

            let result = diesel::insert_into(channel_tbl::table)
                .values(&new_channel)
                .returning(channel_tbl::id)
                .get_result::<String>(conn)?;

            // then add members to the channel
            let members = req
                .members
                .iter()
                .map(|m| NewChannelMember {
                    channel_id: result.clone(),
                    user_id: m.member_id.clone(),
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
    user_id: &str,
    offset: i32,
    limit: i32,
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
            .offset(offset as i64)
            .limit(limit as i64)
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

pub async fn add_user_to_channel(pool: &DbPool, req: ReqAddUserToChannel) -> ServiceResult<()> {
    todo!()
}

pub async fn remove_user_from_channel(
    pool: &DbPool,
    req: ReqRemoveUserFromChannel,
) -> ServiceResult<()> {
    todo!()
}

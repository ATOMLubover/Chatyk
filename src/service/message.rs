use chrono::Utc;
use diesel::prelude::*;
use tokio::task;

use crate::{
    DbPool,
    dto::{ReqSendMessage, RspMessage},
    model::{MessageInfo, NewMessage},
    service::{ServiceError, ServiceResult},
    util,
};

pub async fn create_message(pool: DbPool, req: ReqSendMessage) -> ServiceResult<()> {
    use crate::schema::message_tbl::dsl::*;

    return task::spawn_blocking(move || {
        let msg_id = util::generate_id();

        // TODO: validate the resources in the content
        let new_message = NewMessage {
            id: msg_id,
            channel_id: req.channel_id,
            sender_id: req.sender_id,
            content: req.content,
            created_at: Utc::now(),
        };

        let conn = &mut pool.get()?;

        return match diesel::insert_into(message_tbl)
            .values(&new_message)
            .execute(conn)
        {
            Ok(_) => Ok(()),
            Err(err) => Err(ServiceError::DatabaseError(err)),
        };
    })
    .await?;
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
                created_at: msg.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            })
            .collect::<Vec<_>>();

        return Ok(rsp_msgs);
    })
    .await?;
}

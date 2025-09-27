use axum::extract::ws::{Message, WebSocket};
use futures_util::SinkExt;
use futures_util::stream::SplitSink;

use crate::dto::{ClientMessage, ReqSendMessage};
use crate::event::ServerEvent;
use crate::handler::OnlineUsersMap;
use crate::service::{self, ServiceError, ServiceResult};
use crate::{CacheClient, DatabasePool};

pub async fn handle_notified_event(
    event: ServerEvent,
    sender: &mut SplitSink<WebSocket, Message>,
) -> ServiceResult<()> {
    match event {
        ServerEvent::SpawnMessage {
            sender_id,
            channel_id,
            content,
            created_at,
        } => {
            let msg = serde_json::to_string(&ServerEvent::SpawnMessage {
                sender_id,
                channel_id,
                content,
                created_at,
            })?;

            sender.send(Message::Text(msg.into())).await?;
        }
        ServerEvent::MessageList {
            channel_id,
            messages,
        } => {
            let msg = serde_json::to_string(&ServerEvent::MessageList {
                channel_id,
                messages,
            })?;

            sender.send(Message::Text(msg.into())).await?;
        }
        ServerEvent::UserJoinChannel {
            user_id: joined_user_id,
            channel_id,
            joined_at,
        } => {
            let msg = serde_json::to_string(&ServerEvent::UserJoinChannel {
                user_id: joined_user_id,
                channel_id,
                joined_at,
            })?;

            sender.send(Message::Text(msg.into())).await?;
        }
        ServerEvent::UserLeaveChannel {
            user_id: left_user_id,
            channel_id,
            left_at,
        } => {
            let msg = serde_json::to_string(&ServerEvent::UserLeaveChannel {
                user_id: left_user_id,
                channel_id,
                left_at,
            })?;

            sender.send(Message::Text(msg.into())).await?;
        }
    }

    return Ok(());
}

// FIXME: becoz we can only know the user_id after parsing the message,
// then if we want to verify user, we have to pass user_id down into service functions
pub async fn handle_recv_message(
    user_id: String,
    msg: Message,
    online_users: OnlineUsersMap,
    database: DatabasePool,
    cache: CacheClient,
) -> ServiceResult<()> {
    match msg {
        Message::Text(message) => {
            tracing::debug!("Received text message from client: {}", message);

            let cli_message: ClientMessage = serde_json::from_str(&message)?;

            handle_client_message(user_id, cli_message, online_users, database, cache).await?;
        }
        Message::Binary(bin) => {
            tracing::debug!("Received binary message from client: {:?}", bin);
        }
        Message::Close(close_frame) => {
            tracing::debug!("Received close from client: {:?}", close_frame);
            return Err(ServiceError::CloseWebsocket);
        }
        Message::Ping(ping) => {
            tracing::debug!("Received ping from client: {:?}", ping);
        }
        Message::Pong(pong) => {
            tracing::debug!("Received pong from client: {:?}", pong);
        }
    }

    return Ok(());
}

async fn handle_client_message(
    user_id: String,
    msg: ClientMessage,
    online_users: OnlineUsersMap,
    database: DatabasePool,
    cache: CacheClient,
) -> ServiceResult<()> {
    match msg {
        ClientMessage::ReqSendMessage {
            sender_id,
            channel_id,
            content,
        } => {
            service::create_message(
                database,
                cache,
                online_users,
                ReqSendMessage {
                    sender_id,
                    channel_id,
                    content,
                },
            )
            .await?
        }
        ClientMessage::ReqListChannelMessages {
            channel_id,
            offset,
            limit,
        } => {
            let messages = match offset {
                0 => {
                    service::list_channel_recent_messages(
                        database.clone(),
                        cache.clone(),
                        channel_id.clone(),
                        limit as i64,
                    )
                    .await?
                }
                _ => {
                    service::list_channel_messages(
                        database.clone(),
                        channel_id.clone(),
                        offset as i64,
                        limit as i64,
                    )
                    .await?
                }
            };

            // send messages to rx to send it back to client
            online_users.entry(user_id).and_modify(|tx| {
                let _ = tx.send(ServerEvent::MessageList {
                    channel_id: channel_id.clone(),
                    messages: messages.clone(),
                });
            });

            tracing::debug!(
                "Listed {} messages for channel_id {}",
                messages.len(),
                &channel_id
            );
        }
    }

    Ok(())
}

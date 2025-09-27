use axum::extract::ws::{Message, WebSocket};
use futures_util::SinkExt;
use futures_util::stream::SplitSink;

use crate::cache::streams::StreamReadReply;
use crate::dto::{ClientMessage, ReqSendMessage, ReqUserJoinChannel};
use crate::event::ServerEvent;
use crate::handler::OnlineUsersMap;
use crate::service::{self, ServiceError, ServiceResult};
use crate::{CacheClient, DatabasePool, MessageQueueClient};

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
        } => {
            let msg = serde_json::to_string(&ServerEvent::UserLeaveChannel {
                user_id: left_user_id,
                channel_id,
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
    message_queue: MessageQueueClient,
) -> ServiceResult<()> {
    match msg {
        Message::Text(message) => {
            tracing::debug!("Received text message from client: {}", message);

            let cli_message: ClientMessage = serde_json::from_str(&message)?;

            handle_client_message(
                user_id,
                cli_message,
                online_users,
                database,
                cache,
                message_queue,
            )
            .await?;
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
    message_queue: MessageQueueClient,
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
                message_queue,
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
        ClientMessage::ReqJoinChannel {
            user_id,
            channel_id,
        } => {
            service::add_user_to_channel(
                database,
                cache,
                message_queue,
                online_users,
                ReqUserJoinChannel {
                    user_id,
                    channel_id,
                },
            )
            .await?
        }
        ClientMessage::ReqLeaveChannel {
            user_id,
            channel_id,
        } => {
            service::remove_user_from_channel(
                database,
                cache,
                message_queue,
                online_users,
                crate::dto::ReqUserLeaveChannel {
                    user_id,
                    channel_id,
                },
            )
            .await?
        }
    }

    return Ok(());
}

pub async fn handle_pulled_reply(
    database: DatabasePool,
    cache: CacheClient,
    online_users: OnlineUsersMap,
    reply: StreamReadReply,
    last_id: &mut String,
) -> ServiceResult<()> {
    // though we will only have one stream, but we still need to iterate through it
    let stream = match reply.keys.first() {
        None => return Err(ServiceError::InvalidMessage),
        Some(s) => s,
    };

    for entry in &stream.ids {
        let payload: String = entry.get("payload").ok_or(ServiceError::InvalidMessage)?;

        let event: ServerEvent = serde_json::from_str(&payload)?;

        handle_pulled_event(database.clone(), cache.clone(), online_users.clone(), event).await?;

        // record the last ID we have processed
        *last_id = entry.id.clone();
    }

    return Ok(());
}

async fn handle_pulled_event(
    database: DatabasePool,
    cache: CacheClient,
    online_users: OnlineUsersMap,
    event: ServerEvent,
) -> ServiceResult<()> {
    match event {
        ServerEvent::SpawnMessage {
            sender_id,
            channel_id,
            content,
            created_at,
        } => {
            // FIXME: is there a better way to do this? like batch get all members at once
            let members =
                service::list_channel_members(database.clone(), cache.clone(), channel_id.clone())
                    .await?;

            for member in members {
                // send to the user if online
                if let Some(tx) = online_users.get(&member.user_id) {
                    if let Err(err) = tx
                        .send(ServerEvent::SpawnMessage {
                            sender_id: sender_id.clone(),
                            channel_id: channel_id.clone(),
                            content: content.clone(),
                            created_at: created_at.clone(),
                        })
                        .await
                    {
                        tracing::error!(
                            "Failed to send event to user_id {}: {:?}",
                            &member.user_id,
                            err
                        );
                    }
                }
            }
        }
        ServerEvent::UserJoinChannel {
            user_id,
            channel_id,
            joined_at,
        } => {
            let members =
                service::list_channel_members(database.clone(), cache.clone(), channel_id.clone())
                    .await?;

            for member in members {
                // send to the user if online
                if let Some(tx) = online_users.get(&member.user_id) {
                    if let Err(err) = tx
                        .send(ServerEvent::UserJoinChannel {
                            user_id: user_id.clone(),
                            channel_id: channel_id.clone(),
                            joined_at: joined_at.clone(),
                        })
                        .await
                    {
                        tracing::error!(
                            "Failed to send event to user_id {}: {:?}",
                            &member.user_id,
                            err
                        );
                    }
                }
            }
        }
        ServerEvent::UserLeaveChannel {
            user_id,
            channel_id,
        } => {
            let members =
                service::list_channel_members(database.clone(), cache.clone(), channel_id.clone())
                    .await?;

            for member in members {
                // send to the user if online
                if let Some(tx) = online_users.get(&member.user_id) {
                    if let Err(err) = tx
                        .send(ServerEvent::UserLeaveChannel {
                            user_id: user_id.clone(),
                            channel_id: channel_id.clone(),
                        })
                        .await
                    {
                        tracing::error!(
                            "Failed to send event to user_id {}: {:?}",
                            &member.user_id,
                            err
                        );
                    }
                }
            }
        }
        _ => {
            tracing::warn!("Unhandled event type: {:?}", event);
        }
    }

    return Ok(());
}

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
pub struct ReqSendMessage {
    pub sender_id: String,
    pub channel_id: String,
    pub content: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RspChannelMember {
    pub user_id: String,
    pub username: String,
    pub channel_id: String,
    pub joined_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RspMessage {
    pub message_id: String,
    pub sender_id: String,
    pub channel_id: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum ClientMessage {
    ReqSendMessage {
        sender_id: String,
        channel_id: String,
        content: String,
    },
    ReqListChannelMessages {
        channel_id: String,
        offset: usize,
        limit: usize,
    },
}

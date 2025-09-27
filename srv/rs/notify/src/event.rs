use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::dto::RspMessage;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum ServerEvent {
    SpawnMessage {
        sender_id: String,
        channel_id: String,
        content: String,
        created_at: DateTime<Utc>,
    },
    MessageList {
        channel_id: String,
        messages: Vec<RspMessage>,
    },
    UserJoinChannel {
        user_id: String,
        channel_id: String,
        joined_at: DateTime<Utc>,
    },
    UserLeaveChannel {
        user_id: String,
        channel_id: String,
        left_at: DateTime<Utc>,
    },
}

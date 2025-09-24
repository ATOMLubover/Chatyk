use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct RspUserInfo {
    pub id: String,
    pub username: String,
    pub email: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RspChannelInfo {
    pub id: String,
    pub chan_name: String,
    pub channel_type: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RspChannelMember {
    pub user_id: String,
    pub username: String,
    pub user_email: String,
    pub channel_id: String,
    pub joined_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RspMessage {
    pub id: String,
    pub channel_id: String,
    pub sender_id: String,
    pub content: String,
    pub created_at: String,
}

// TODO: at current stage, we do not care about content type
#[derive(Debug, Clone, Serialize)]
pub struct RspResourceInfo {
    pub id: String,
    pub url: String,
    pub uploaded_at: String,
}

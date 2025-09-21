use serde::Serialize;

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
    pub chan_name: Option<String>,
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

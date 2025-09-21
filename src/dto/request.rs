use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ReqRegisterUser {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReqUserLogin {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReqPatchUser {
    pub user_id: String,
    pub username: Option<String>,
    pub email: Option<String>,
    pub password: Option<String>,
}

/// `ReqCreateChannel` is used to create a new channel.
/// since a channel should be composed of at least two members,
/// member_ids, which is a vec, is a required field.
#[derive(Debug, Clone, Deserialize)]
pub struct ReqCreateChannel {
    pub member_ids: Vec<String>,
    pub channel_name: Option<String>,
    pub channel_type: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetChannelListParams {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReqAddUserToChannel {
    pub user_id: String,
    pub channel_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReqRemoveUserFromChannel {
    pub user_id: String,
    pub channel_id: String,
}

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

#[derive(Debug, Clone, Deserialize)]
pub struct ReqChannelMember {
    pub member_id: String,
    pub member_name: String,
}

/// `ReqCreateChannel` is used to create a new channel.
/// since a channel should be composed of at least two members,
/// member_ids, which is a vec, is a required field.
#[derive(Debug, Clone, Deserialize)]
pub struct ReqCreateChannel {
    pub members: Vec<ReqChannelMember>,
    pub chan_name: Option<String>,
    pub channel_type: String,
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

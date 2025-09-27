use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct ReqRegisterUser {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReqUserLogin {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RspToken {
    pub token_type: String,
    pub token: String,
}

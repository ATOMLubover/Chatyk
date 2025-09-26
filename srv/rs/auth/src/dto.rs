use serde::Deserialize;

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

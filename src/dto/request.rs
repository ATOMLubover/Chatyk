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
    pub username: Option<String>,
    pub email: Option<String>,
    pub password: Option<String>,
}

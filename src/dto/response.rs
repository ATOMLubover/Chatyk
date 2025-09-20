use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct RspUserInfo {
    pub id: String,
    pub username: String,
    pub email: String,
    pub created_at: String,
}

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct RspUserInfo {
    pub id: String,
    pub username: String,
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

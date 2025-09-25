mod request;
mod response;

use serde::{Deserialize, Serialize};

pub use request::*;
pub use response::*;

#[derive(Debug, Clone, Serialize)]
pub struct ErrorResponse {
    pub code: u16,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PaginatedParams {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

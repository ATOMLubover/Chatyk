pub mod request;
pub mod response;

use serde::{Deserialize, Serialize};

pub use request::*;
pub use response::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub code: u16,
    pub message: String,
}

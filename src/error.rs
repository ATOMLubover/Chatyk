use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::dto::ErrorResponse;

#[derive(Debug, Serialize, Deserialize, Error)]
pub enum AppError {
    #[error("Unknown error: {0}")]
    UnknownError(String),
}

impl Into<StatusCode> for AppError {
    fn into(self) -> StatusCode {
        match self {
            AppError::UnknownError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        tracing::trace!("AppError into response: {:?}", self);

        let (status, msg) = match self {
            AppError::UnknownError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "An unknown error occurred.".to_string(),
            ),
        };

        let body = axum::Json(ErrorResponse {
            code: status.as_u16(),
            message: msg,
        });

        return (status, body).into_response();
    }
}

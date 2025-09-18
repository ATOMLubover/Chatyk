mod user;

use axum::{Router, response::IntoResponse, routing};

use crate::error::AppError;

pub async fn health_check() -> impl IntoResponse {
    return "OK".to_string();
}

pub async fn test_app_error() -> Result<String, AppError> {
    return Err(AppError::UnknownError("This is a test error".to_string()));
}

pub fn get_router() -> Router {
    let router = Router::new()
        .route("/health", routing::get(health_check))
        .route("/test_error", routing::get(test_app_error));

    return router;
}

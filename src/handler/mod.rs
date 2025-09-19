mod middleware;
mod user;

use anyhow::{Error, Result};
use axum::Json;
use axum::Router;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::http::header::InvalidHeaderValue;
use axum::response::{IntoResponse, Response};
use axum::routing;
use cookie::time::Duration;
use thiserror::Error;

use crate::dto::ErrorResponse;

use crate::DbPool;
use crate::config::AppConfig;
use crate::service::ServiceError;

#[derive(Debug, Clone)]
struct AppState {
    config: AppConfig,
    db_pool: DbPool,
    jwt_encoding_key: String,
    #[allow(dead_code)]
    jwt_decoding_key: String,
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Service error: {0}")]
    ServiceError(StatusCode, String),

    #[error("Authentication token genration failure")]
    TokenGenerationFailure,

    #[error("Failed to parse cookie")]
    CookieParseError,

    #[allow(dead_code)]
    #[error("Unknown error: {0}")]
    UnknownError(String),
}

impl Into<StatusCode> for AppError {
    fn into(self) -> StatusCode {
        match self {
            AppError::ServiceError(status, _) => status,
            AppError::TokenGenerationFailure => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::CookieParseError => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::UnknownError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        tracing::trace!("AppError into response: {:?}", self);

        let (status, msg) = match self {
            AppError::ServiceError(status, msg) => (status, msg),
            AppError::TokenGenerationFailure => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to generate authentication token.".to_string(),
            ),
            AppError::CookieParseError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to parse cookie.".to_string(),
            ),
            AppError::UnknownError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "An unknown error occurred.".to_string(),
            ),
        };

        let body = Json(ErrorResponse {
            code: status.as_u16(),
            message: msg,
        });

        return (status, body).into_response();
    }
}

impl From<ServiceError> for AppError {
    fn from(err: ServiceError) -> Self {
        match err {
            ServiceError::EmailOrUsernameConflict => {
                tracing::trace!("Email or username conflict error");
                AppError::ServiceError(
                    StatusCode::CONFLICT,
                    "Email or username already exists".to_string(),
                )
            }
            ServiceError::UserNotFound => {
                tracing::trace!("User not found error");
                AppError::ServiceError(StatusCode::NOT_FOUND, "User not found".to_string())
            }
            ServiceError::PasswordMismatch => {
                tracing::trace!("Password mismatch error");
                AppError::ServiceError(StatusCode::UNAUTHORIZED, "Password mismatch".to_string())
            }
            ServiceError::PasswordHashError(err) => {
                tracing::error!("Password hash error: {:?}", err);
                AppError::ServiceError(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Unprocessable password".to_string(),
                )
            }

            // database / db pool error will only be handled here
            // so log them as error level
            ServiceError::DbPoolError(err) => {
                tracing::error!("Database pool error: {:?}", err);
                AppError::ServiceError(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Database pool error".to_string(),
                )
            }
            ServiceError::DatabaseError(err) => {
                tracing::error!("Database error: {:?}", err);
                AppError::ServiceError(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Database error".to_string(),
                )
            }
        }
    }
}

pub fn append_cookie(
    response: &mut Response,
    key: &str,
    value: &str,
    path: &str,
    exp: Duration,
) -> Result<(), InvalidHeaderValue> {
    use cookie::Cookie;

    let cookie = match HeaderValue::from_str(
        &Cookie::build((key, value))
            .path(path)
            .http_only(true)
            .max_age(exp)
            .to_string(),
    ) {
        Ok(c) => c,
        Err(err) => return Err(err),
    };

    response.headers_mut().append("Set-Cookie", cookie);

    return Ok(());
}

pub async fn health_check() -> impl IntoResponse {
    return "OK".to_string();
}

pub fn get_router(config: AppConfig, db_pool: DbPool) -> Result<Router, Error> {
    dotenvy::dotenv().map_err(|err| anyhow::anyhow!("Failed to load .env file: {err}."))?;

    let jwt_encoding_key = std::env::var(&config.jwt_encoding_key_env)
        .map_err(|err| anyhow::anyhow!("Failed to get JWT encoding key from env: {err}."))?;
    let jwt_decoding_key = std::env::var(&config.jwt_decoding_key_env)
        .map_err(|err| anyhow::anyhow!("Failed to get JWT decoding key from env: {err}."))?;

    let auth_router = Router::new()
        .route("/register", routing::post(user::register_user))
        .route("/login", routing::post(user::login_user));

    let api_router = Router::new().layer(axum::middleware::from_fn_with_state(
        jwt_decoding_key.clone(),
        middleware::auth_middleware,
    ));

    let router = Router::new()
        .route("/health_check", routing::get(health_check))
        .nest("/auth", auth_router)
        .nest("/api", api_router)
        .with_state(AppState {
            config,
            db_pool,
            jwt_decoding_key,
            jwt_encoding_key,
        });

    return Ok(router);
}

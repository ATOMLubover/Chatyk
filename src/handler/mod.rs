mod auth;
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
                AppError::ServiceError(StatusCode::CONFLICT, err.to_string())
            }
            ServiceError::UserNotFound => {
                AppError::ServiceError(StatusCode::NOT_FOUND, err.to_string())
            }
            ServiceError::PasswordMismatch => {
                AppError::ServiceError(StatusCode::UNAUTHORIZED, err.to_string())
            }
            ServiceError::PasswordHashError(_) => {
                AppError::ServiceError(StatusCode::BAD_REQUEST, err.to_string())
            }
            ServiceError::InvalidChannelType(msg) => {
                AppError::ServiceError(StatusCode::BAD_REQUEST, msg)
            }
            ServiceError::InvalidChannelMemberNumber => {
                AppError::ServiceError(StatusCode::BAD_REQUEST, err.to_string())
            }
            ServiceError::ChannelCreationFailure => {
                AppError::ServiceError(StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
            }
            ServiceError::BlockingJoinError(_) => {
                AppError::ServiceError(StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
            }
            ServiceError::DbPoolError(_) => {
                AppError::ServiceError(StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
            }
            ServiceError::DatabaseError(_) => {
                AppError::ServiceError(StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
            }
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;

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

pub fn get_router(config: AppConfig, db_pool: DbPool) -> Result<Router, Error> {
    dotenvy::dotenv().map_err(|err| anyhow::anyhow!("Failed to load .env file: {err}."))?;

    let jwt_encoding_key = std::env::var(&config.jwt_encoding_key_env)
        .map_err(|err| anyhow::anyhow!("Failed to get JWT encoding key from env: {err}."))?;
    let jwt_decoding_key = std::env::var(&config.jwt_decoding_key_env)
        .map_err(|err| anyhow::anyhow!("Failed to get JWT decoding key from env: {err}."))?;

    // router for authentication (no auth required)
    let auth_router = Router::new()
        .route("/register", routing::post(auth::register_user))
        .route("/login", routing::post(auth::login_user));

    // router for APIs (auth middleware added)
    let api_router = Router::new()
        .route(
            "/users/{:user_id}",
            routing::get(user::get_user_info_by_id).patch(user::patch_user_with_id),
        )
        .layer(axum::middleware::from_fn_with_state(
            jwt_decoding_key.clone(),
            middleware::auth_middleware,
        ));

    // router for entire application
    let app_router = Router::new()
        .route(
            "/health_check",
            routing::get(async || "Hello from server!\n".to_string()),
        )
        .nest("/auth", auth_router)
        .nest("/api", api_router)
        .with_state(AppState {
            config,
            db_pool,
            jwt_decoding_key,
            jwt_encoding_key,
        });

    return Ok(app_router);
}

mod auth;
mod channel;
mod message;
mod middleware;
mod resource;
mod user;

use anyhow::{Error, Result};
use axum::{
    Json, Router,
    http::header::InvalidHeaderValue,
    http::{HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing,
};
use cookie::time::Duration;
use thiserror::Error;

use crate::{DbPool, config::AppConfig, dto::ErrorResponse, service::ServiceError};

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct AppState {
    config: AppConfig,
    db_pool: DbPool,
    jwt_encoding_key: String,
    jwt_decoding_key: String,
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Service error: {0}")]
    ServiceError(StatusCode, String),

    #[error("Authentication token genration failure")]
    TokenGenerationFailure,

    #[error("Failed to parse cookie")]
    CookieAppendError,

    #[error("Failed to parse multipart form data: {0}")]
    MultiPartParseError(#[from] axum::extract::multipart::MultipartError),

    #[allow(dead_code)]
    #[error("Unknown error: {0}")]
    UnknownError(String),
}

impl Into<StatusCode> for AppError {
    fn into(self) -> StatusCode {
        match self {
            AppError::ServiceError(status, _) => status,
            AppError::TokenGenerationFailure => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::CookieAppendError => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::MultiPartParseError(_) => StatusCode::BAD_REQUEST,
            AppError::UnknownError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        tracing::trace!("AppError into response: {:?}", self);

        let (status, msg) = match self {
            AppError::ServiceError(status, msg) => match status {
                StatusCode::INTERNAL_SERVER_ERROR => {
                    // we do not want to expose internal error details to clients
                    (status, "Internal server error occuerrd.".to_string())
                }
                _ => (status, msg),
            },
            AppError::TokenGenerationFailure => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to generate authentication token.".to_string(),
            ),
            AppError::CookieAppendError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to parse cookie.".to_string(),
            ),
            AppError::MultiPartParseError(_) => (
                StatusCode::BAD_REQUEST,
                "Failed to parse multipart form data: {}".to_string(),
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
        // TODO: delete err.to_string() calls in 500
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
            ServiceError::InvalidChannelType => {
                AppError::ServiceError(StatusCode::BAD_REQUEST, err.to_string())
            }
            ServiceError::InvalidChannelMemberNumber => {
                AppError::ServiceError(StatusCode::BAD_REQUEST, err.to_string())
            }
            ServiceError::ChannelCreationFailure => {
                AppError::ServiceError(StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
            }
            ServiceError::NonexistingChannel => {
                AppError::ServiceError(StatusCode::NOT_FOUND, err.to_string())
            }
            ServiceError::UserAlreadyInChannel => {
                AppError::ServiceError(StatusCode::CONFLICT, err.to_string())
            }
            ServiceError::UserNotInChannel => {
                AppError::ServiceError(StatusCode::BAD_REQUEST, err.to_string())
            }
            ServiceError::BlockingJoinError(_) => {
                AppError::ServiceError(StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
            }
            ServiceError::StdIoError(_) => {
                AppError::ServiceError(StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
            }
            ServiceError::ResourceUploadError(_) => {
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

    // router for users
    let user_router = Router::new().route(
        "/{:user_id}",
        routing::get(user::get_user_info_by_id).patch(user::patch_user_with_id),
    );

    // router for channels
    let channel_router = Router::new()
        .route(
            "/",
            routing::get(channel::get_channel_list).post(channel::create_channel),
        )
        .route(
            "/{:channel_id}/members",
            routing::get(channel::get_channel_member_list)
                .post(channel::join_channel)
                .delete(channel::quit_channel),
        )
        .route(
            "/{:channel_id}/messages",
            routing::get(message::get_message_list).post(message::send_message),
        );

    // router for resources
    let resource_router = Router::new().route("/upload", routing::post(resource::upload_resource));

    // router for APIs (auth middleware added)
    let api_router = Router::new()
        .nest("/users", user_router)
        .nest("/channels", channel_router)
        .nest("/resources", resource_router)
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

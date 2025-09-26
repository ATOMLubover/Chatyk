mod auth;

use std::{ops::Deref, sync::Arc};

use axum::Json;
use axum::Router;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use serde::{Deserialize, Serialize};
use shared::util::JwtCodec;
use thiserror::Error;

use crate::AppConfig;
use crate::DatabasePool;

type AppResult<T> = Result<T, AppError>;

#[derive(Clone, Debug)]
struct AppState {
    inner: Arc<AppStateInner>,
}

#[derive(Clone, Debug)]
struct AppStateInner {
    pub jwt_codec: JwtCodec,
    pub database_pool: Arc<DatabasePool>,
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Username exsiting")]
    UsernameConflict,
    #[error("Username or password incorrect")]
    UsernameOrPasswordIncorrect,

    #[error("Database error: {0}")]
    DatabaseError(#[from] diesel::result::Error),
    #[error("Database pool error: {0}")]
    DatabasePoolError(#[from] r2d2::Error),
    #[error("Bcrypt error: {0}")]
    BcrypeError(#[from] bcrypt::BcryptError),
    #[error("Tokio join error: {0}")]
    TokioJoinError(#[from] tokio::task::JoinError),
    #[error("JWT codec error: {0}")]
    JwtCodecError(#[from] jsonwebtoken::errors::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TokenSubject {
    pub user_id: String,
}

impl Deref for AppState {
    type Target = AppStateInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl AppState {
    pub fn new(jwt_codec: JwtCodec, database_pool: DatabasePool) -> Self {
        AppState {
            inner: Arc::new(AppStateInner {
                jwt_codec,
                database_pool: Arc::new(database_pool),
            }),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::UsernameConflict => {
                tracing::debug!("AppError: Username conflict");
                (StatusCode::CONFLICT, self.to_string())
            }
            AppError::UsernameOrPasswordIncorrect => {
                tracing::debug!("AppError: User or password incorrect");
                (StatusCode::UNAUTHORIZED, self.to_string())
            }
            AppError::DatabaseError(err) => {
                tracing::error!("AppError: Database error: {:?}", err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Unexpected server error"),
                )
            }
            AppError::DatabasePoolError(err) => {
                tracing::error!("AppError: Database pool error: {:?}", err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Unexpected server error"),
                )
            }
            AppError::BcrypeError(err) => {
                tracing::error!("AppError: Bcrypt error: {:?}", err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Unexpected server error"),
                )
            }
            AppError::TokioJoinError(err) => {
                tracing::error!("AppError: Tokio join error: {:?}", err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Unexpected server error"),
                )
            }
            AppError::JwtCodecError(err) => {
                tracing::error!("AppError: JWT codec error: {:?}", err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Unexpected server error"),
                )
            }
        };

        let body = Json(serde_json::json!({
            "code": status.as_u16(),
            "error": error_message,
        }));

        return (status, body).into_response();
    }
}

pub fn get_router(config: &AppConfig, database_pool: DatabasePool) -> anyhow::Result<Router> {
    let jwt_encoding_key = std::env::var(&config.jwt_enckey_env)
        .map_err(|err| anyhow::anyhow!("JWT encoding key env not set: {}", err))?;
    let jwt_decoding_key = std::env::var(&config.jwt_deckey_env)
        .map_err(|err| anyhow::anyhow!("JWT decoding key env not set: {}", err))?;

    tracing::debug!("JWT encoding and decoding keys loaded from env");

    let jwt_codec = JwtCodec::new(
        jwt_encoding_key.as_bytes(),
        jwt_decoding_key.as_bytes(),
        24 * 60 * 60, // TODO: now it is 24 hours, make it configurable
    );

    let app_state = AppState::new(jwt_codec, database_pool);

    let app_router = Router::new()
        .route("/health", axum::routing::get(|| async { "OK" }))
        .nest("/auth", auth::get_router())
        .with_state(app_state);

    return Ok(app_router);
}

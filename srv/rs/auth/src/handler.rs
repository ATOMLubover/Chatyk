use std::ops::Deref;
use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing;

use diesel::result::DatabaseErrorKind;
use diesel::result::Error;
use serde::{Deserialize, Serialize};
use shared::model::NewUser;
use shared::util;
use shared::util::JwtCodec;
use thiserror::Error;

use crate::AppConfig;
use crate::DatabasePool;
use crate::dto::ReqRegisterUser;
use crate::dto::ReqUserLogin;
use crate::dto::RspToken;

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
        24 * 24 * 60 * 60, // TODO: now it is 24 days, make it configurable
    );

    let auth_router: Router<AppState> = Router::new()
        .route("/register", routing::post(register_user))
        .route("/login", routing::post(login_user));

    let app_state = AppState::new(jwt_codec, database_pool);

    let app_router = Router::new()
        .route("/health", axum::routing::get(|| async { "OK" }))
        .nest("/auth", auth_router)
        .with_state(app_state);

    return Ok(app_router);
}

fn generate_token(subject: TokenSubject, codec: &JwtCodec) -> AppResult<String> {
    let token = codec.encode(subject)?;
    return Ok(token);
}

async fn register_user(
    State(state): State<AppState>,
    Json(payload): Json<ReqRegisterUser>,
) -> AppResult<impl IntoResponse> {
    use diesel::prelude::*;
    use shared::schema::user_tbl::dsl::*;

    tracing::trace!("Register payload: {:?}", payload);

    let database_pool = Arc::clone(&state.database_pool);

    let token = tokio::task::spawn_blocking(move || {
        let conn = &mut database_pool.get()?;

        let hashed_password = bcrypt::hash(payload.password, bcrypt::DEFAULT_COST)?;
        let user_id = util::generate_id();

        // we need to ensure that token is generated before inserting user into database
        let token = generate_token(
            TokenSubject {
                user_id: user_id.clone(),
            },
            &state.inner.jwt_codec,
        )?;

        let new_user = NewUser {
            id: user_id.clone(),
            username: payload.username,
            password_hash: hashed_password,
        };

        match diesel::insert_into(user_tbl)
            .values(&new_user)
            .execute(conn)
        {
            Err(err) => {
                if let Error::DatabaseError(DatabaseErrorKind::UniqueViolation, info) = err {
                    tracing::debug!("Unique violation when register user: {:?}", info.message());
                    return Err(AppError::UsernameConflict);
                }
                return Err(AppError::DatabaseError(err));
            }
            Ok(_) => return Ok(token),
        };
    })
    .await??;

    return Ok(Json(RspToken {
        token_type: "Bearer".to_string(),
        token,
    }));
}

async fn login_user(
    State(state): State<AppState>,
    Json(payload): Json<ReqUserLogin>,
) -> AppResult<impl IntoResponse> {
    tracing::trace!("Login payload: {:?}", payload);

    let database_pool = Arc::clone(&state.database_pool);

    let token = tokio::task::spawn_blocking(move || {
        use diesel::prelude::*;
        use shared::schema::user_tbl::dsl::*;

        let conn = &mut database_pool.get()?;

        let (user_id, hashed_password): (String, String) = user_tbl
            .filter(username.eq(&payload.username))
            .select((id, password_hash))
            .first(conn)?;

        if !bcrypt::verify(&payload.password, &hashed_password)? {
            return Err(AppError::UsernameOrPasswordIncorrect);
        }

        let token = generate_token(
            TokenSubject {
                user_id: user_id.clone(),
            },
            &state.inner.jwt_codec,
        )?;

        return Ok(token);
    })
    .await??;

    return Ok(Json(RspToken {
        token_type: "Bearer".to_string(),
        token,
    }));
}

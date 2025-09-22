use std::sync::Arc;

use axum::{
    Json,
    body::Body,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use cookie::time::Duration;
use jsonwebtoken::EncodingKey;

use crate::handler::middleware::UserToken;
use crate::{
    dto::{ReqRegisterUser, ReqUserLogin},
    handler::{AppError, AppResult, AppState},
    service,
    util::encode_jwt,
};

fn append_token_cookie(
    response: &mut Response<Body>,
    token: UserToken,
    encoding_key: &EncodingKey,
    expiration_time: Duration,
) -> AppResult<()> {
    let token = match encode_jwt(token, &encoding_key).map_err(|err| {
        tracing::error!("Failed to encode JWT: {:?}", err);
        AppError::TokenGenerationFailure
    }) {
        Ok(t) => t,
        Err(app_err) => return Err(app_err),
    };

    super::append_cookie(
        response,
        "Set-Cookie",
        &format!("Bearer {}", token),
        "/",
        expiration_time,
    )
    .map_err(|err| {
        tracing::error!("Failed to append cookie: {:?}", err);
        return AppError::CookieAppendError;
    })?;

    return Ok(());
}

pub async fn register_user(
    State(state): State<AppState>,
    Json(payload): Json<ReqRegisterUser>,
) -> AppResult<impl IntoResponse> {
    tracing::trace!("Register payload: {:?}", payload);

    let result = service::register_user(Arc::clone(&state.db_pool), payload).await?;

    let user_id = result.id.clone();

    let mut response = (StatusCode::CREATED, Json(result)).into_response();

    // add auth header with JWT
    let response = append_token_cookie(
        &mut response,
        UserToken {
            user_id: user_id.clone(),
            exp: chrono::Utc::now().timestamp() + state.config.jwt_expiration_hours * 3600,
        },
        &EncodingKey::from_secret(state.jwt_encoding_key.as_bytes()),
        Duration::hours(state.config.jwt_expiration_hours),
    )
    .map_err(|err| {
        tracing::error!("Failed to append token cookie: {:?}", err);
        return AppError::from(err);
    })?;

    return Ok(response);
}

pub async fn login_user(
    State(state): State<AppState>,
    Json(payload): Json<ReqUserLogin>,
) -> AppResult<impl IntoResponse> {
    tracing::trace!("Login payload: {:?}", payload);

    let result = service::login_user(Arc::clone(&state.db_pool), payload).await?;

    let user_id = result.id.clone();

    let mut response = (StatusCode::OK, Json(result)).into_response();

    // add auth header with JWT
    let response = append_token_cookie(
        &mut response,
        UserToken {
            user_id: user_id.clone(),
            exp: chrono::Utc::now().timestamp() + state.config.jwt_expiration_hours * 3600,
        },
        &EncodingKey::from_secret(state.jwt_encoding_key.as_bytes()),
        Duration::hours(state.config.jwt_expiration_hours),
    )
    .map_err(|err| {
        tracing::error!("Failed to append token cookie: {:?}", err);
        return AppError::from(err);
    })?;

    return Ok(response);
}

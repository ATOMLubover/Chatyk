use axum::Json;
use axum::body::Body;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use cookie::time::Duration;
use jsonwebtoken::EncodingKey;

use crate::dto::request::{ReqRegisterUser, ReqUserLogin};
use crate::handler::AppResult;
use crate::handler::middleware::UserToken;
use crate::handler::{AppError, AppState};
use crate::service::{self};
use crate::util::encode_jwt;

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

    if let Err(err) = super::append_cookie(
        response,
        "Set-Cookie",
        &format!("Bearer {}", token),
        "/",
        expiration_time,
    ) {
        tracing::error!("Failed to append auth cookie: {:?}", err);
        return Err(AppError::CookieParseError);
    }

    return Ok(());
}

pub async fn register_user(
    State(state): State<AppState>,
    Json(payload): Json<ReqRegisterUser>,
) -> impl IntoResponse {
    tracing::trace!("Register payload: {:?}", payload);

    let result = match service::register_user(&state.db_pool, payload).await {
        Ok(user) => user,
        Err(err) => return AppError::from(err).into_response(),
    };

    let user_id = result.id.clone();

    let mut response = (StatusCode::CREATED, Json(result)).into_response();

    // add auth header with JWT
    if let Err(err) = append_token_cookie(
        &mut response,
        UserToken {
            user_id: user_id.clone(),
            exp: chrono::Utc::now().timestamp() + state.config.jwt_expiration_hours * 3600,
        },
        &EncodingKey::from_secret(state.jwt_encoding_key.as_bytes()),
        Duration::hours(state.config.jwt_expiration_hours),
    ) {
        tracing::error!("Failed to append token cookie: {:?}", err);
        return AppError::from(err).into_response();
    }

    return response;
}

pub async fn login_user(
    State(state): State<AppState>,
    Json(payload): Json<ReqUserLogin>,
) -> impl IntoResponse {
    tracing::trace!("Login payload: {:?}", payload);

    let result = match service::login_user(&state.db_pool, payload).await {
        Ok(user) => user,
        Err(err) => return AppError::from(err).into_response(),
    };

    let user_id = result.id.clone();

    let mut response = (StatusCode::OK, Json(result)).into_response();

    // add auth header with JWT
    if let Err(err) = append_token_cookie(
        &mut response,
        UserToken {
            user_id: user_id.clone(),
            exp: chrono::Utc::now().timestamp() + state.config.jwt_expiration_hours * 3600,
        },
        &EncodingKey::from_secret(state.jwt_encoding_key.as_bytes()),
        Duration::hours(state.config.jwt_expiration_hours),
    ) {
        tracing::error!("Failed to append token cookie: {:?}", err);
        return AppError::from(err).into_response();
    }

    return response;
}

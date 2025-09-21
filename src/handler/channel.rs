use std::sync::Arc;

use axum::Extension;
use axum::Json;
use axum::extract::Path;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;

use crate::dto::ReqAddUserToChannel;
use crate::dto::ReqRemoveUserFromChannel;
use crate::dto::{GetChannelListParams, ReqCreateChannel};
use crate::handler::middleware::UserToken;
use crate::handler::{AppError, AppState};
use crate::service::{self};

pub async fn create_channel(
    State(state): State<AppState>,
    Json(payload): Json<ReqCreateChannel>,
) -> impl IntoResponse {
    tracing::trace!("Create channel payload: {:?}", payload);

    return match service::create_channel(Arc::clone(&state.db_pool), payload).await {
        Ok(_) => (StatusCode::CREATED, Json("Channel created successfully")).into_response(),
        Err(err) => return AppError::from(err).into_response(),
    };
}

pub async fn get_channel_list(
    State(state): State<AppState>,
    Query(params): Query<GetChannelListParams>,
    Extension(user_token): Extension<UserToken>,
) -> impl IntoResponse {
    tracing::trace!("Get channel list for user_id: {}", user_token.user_id);

    return match service::list_user_channels(
        Arc::clone(&state.db_pool),
        user_token.user_id,
        params.offset.unwrap_or(0),
        params.limit.unwrap_or(20),
    )
    .await
    {
        Ok(channels) => (StatusCode::OK, Json(channels)).into_response(),
        Err(err) => return AppError::from(err).into_response(),
    };
}

pub async fn get_channel_member_list(
    State(state): State<AppState>,
    Path(channel_id): Path<String>,
) -> impl IntoResponse {
    tracing::trace!("Get channel member list for channel_id: {}", channel_id);

    return match service::list_channel_members(Arc::clone(&state.db_pool), channel_id).await {
        Ok(members) => (StatusCode::OK, Json(members)).into_response(),
        Err(err) => return AppError::from(err).into_response(),
    };
}

pub async fn join_channel(
    State(state): State<AppState>,
    Extension(user_token): Extension<UserToken>,
    Json(payload): Json<ReqAddUserToChannel>,
) -> impl IntoResponse {
    tracing::trace!("Join channel payload: {:?}", payload);

    if payload.user_id != user_token.user_id {
        // a user can only join a channel with his/her own user_id
        return StatusCode::FORBIDDEN.into_response();
    }

    return match service::add_user_to_channel(Arc::clone(&state.db_pool), payload).await {
        Ok(_) => (StatusCode::NO_CONTENT).into_response(),
        Err(err) => return AppError::from(err).into_response(),
    };
}

pub async fn quit_channel(
    State(state): State<AppState>,
    Extension(user_token): Extension<UserToken>,
    Json(payload): Json<ReqRemoveUserFromChannel>,
) -> impl IntoResponse {
    tracing::trace!("Quit channel payload: {:?}", payload);

    if payload.user_id != user_token.user_id {
        // a user can only quit a channel he/she is in
        return StatusCode::FORBIDDEN.into_response();
    }

    return match service::remove_user_from_channel(Arc::clone(&state.db_pool), payload).await {
        Ok(_) => (StatusCode::NO_CONTENT).into_response(),
        Err(err) => return AppError::from(err).into_response(),
    };
}

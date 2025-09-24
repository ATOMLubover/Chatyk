use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};

use crate::{
    dto::{PaginatedParams, ReqAddUserToChannel, ReqCreateChannel, ReqRemoveUserFromChannel},
    handler::middleware::UserToken,
    handler::{AppResult, AppState},
    service,
};

pub async fn create_channel(
    State(state): State<AppState>,
    Json(payload): Json<ReqCreateChannel>,
) -> AppResult<impl IntoResponse> {
    tracing::trace!("Create channel payload: {:?}", payload);

    service::create_channel(
        Arc::clone(&state.db_pool),
        Arc::clone(&state.cache_cli),
        Arc::clone(&state.online_users),
        payload,
    )
    .await?;

    return Ok(StatusCode::CREATED);
}

pub async fn get_channel_list(
    State(state): State<AppState>,
    Query(params): Query<PaginatedParams>,
    Extension(user_token): Extension<UserToken>,
) -> AppResult<impl IntoResponse> {
    tracing::trace!("Get channel list for user_id: {}", user_token.user_id);

    let list = service::list_user_channels(
        Arc::clone(&state.db_pool),
        user_token.user_id,
        params.offset.unwrap_or(0),
        params.limit.unwrap_or(20),
    )
    .await?;

    return Ok((StatusCode::OK, Json(list)));
}

pub async fn get_channel_member_list(
    State(state): State<AppState>,
    Path(channel_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    tracing::trace!("Get channel member list for channel_id: {}", channel_id);

    let list = service::list_channel_members(
        Arc::clone(&state.db_pool),
        Arc::clone(&state.cache_cli),
        channel_id,
    )
    .await?;

    return Ok((StatusCode::OK, Json(list)));
}

pub async fn join_channel(
    State(state): State<AppState>,
    Extension(user_token): Extension<UserToken>,
    Json(payload): Json<ReqAddUserToChannel>,
) -> AppResult<impl IntoResponse> {
    tracing::trace!("Join channel payload: {:?}", payload);

    if payload.user_id != user_token.user_id {
        // a user can only join a channel with his/her own user_id
        return Ok(StatusCode::FORBIDDEN);
    }

    service::add_user_to_channel(
        Arc::clone(&state.db_pool),
        Arc::clone(&state.cache_cli),
        Arc::clone(&state.online_users),
        payload,
    )
    .await?;

    return Ok(StatusCode::NO_CONTENT);
}

pub async fn quit_channel(
    State(state): State<AppState>,
    Extension(user_token): Extension<UserToken>,
    Json(payload): Json<ReqRemoveUserFromChannel>,
) -> AppResult<impl IntoResponse> {
    tracing::trace!("Quit channel payload: {:?}", payload);

    if payload.user_id != user_token.user_id {
        // a user can only quit a channel he/she is in
        return Ok(StatusCode::FORBIDDEN);
    }

    service::remove_user_from_channel(
        Arc::clone(&state.db_pool),
        Arc::clone(&state.cache_cli),
        Arc::clone(&state.online_users),
        payload,
    )
    .await?;

    return Ok(StatusCode::NO_CONTENT);
}

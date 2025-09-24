use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};

use crate::{
    dto::{PaginatedParams, ReqSendMessage},
    handler::{AppResult, AppState, middleware::UserToken},
    service,
};

pub async fn send_message(
    State(state): State<AppState>,
    Extension(user_token): Extension<UserToken>,
    Json(payload): Json<ReqSendMessage>,
) -> AppResult<impl IntoResponse> {
    tracing::trace!("Send message payload: {:?}", payload);

    if payload.sender_id != user_token.user_id {
        // a user can only send a message with his/her own user_id
        return Ok(StatusCode::FORBIDDEN);
    }

    service::create_message(
        Arc::clone(&state.db_pool),
        Arc::clone(&state.cache_pool),
        payload,
    )
    .await?;

    return Ok(StatusCode::CREATED);
}

pub async fn get_message_list(
    State(state): State<AppState>,
    Query(params): Query<PaginatedParams>,
    Path(channel_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    tracing::trace!("Get message list for channel id: {}", channel_id);

    // FIXME: a user can only get messages from channels he/she is a member of
    // espacially for private channels
    let messages = service::list_channel_messages(
        Arc::clone(&state.db_pool),
        channel_id,
        params.offset.unwrap_or(0),
        params.limit.unwrap_or(20),
    )
    .await?;

    return Ok((StatusCode::OK, Json(messages)));
}

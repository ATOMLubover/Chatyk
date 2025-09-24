use axum::{
    Extension,
    extract::{State, ws::WebSocket},
    http::StatusCode,
    response::IntoResponse,
};

use crate::{
    handler::middleware::UserToken,
    handler::{AppResult, AppState},
};

pub async fn notify_events(
    State(state): State<AppState>,
    Extension(user_token): Extension<UserToken>,
    ws: WebSocket,
) -> AppResult<impl IntoResponse> {
    tracing::trace!(
        "WebSocket connection established for user_id: {}",
        user_token.user_id
    );

    // service::handle_notify_ws(
    //     Arc::clone(&state.online_users),
    //     user_token.user_id.clone(),
    //     ws,
    // )
    // .await?;

    tracing::trace!(
        "WebSocket connection closed for user_id: {}",
        user_token.user_id
    );

    return Ok(StatusCode::OK);
}

use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};

use crate::{
    dto::ReqPatchUser,
    handler::middleware::UserToken,
    handler::{AppError, AppResult, AppState},
    service,
};

pub async fn get_user_info_by_id(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    tracing::trace!("Get user info for user_id: {}", user_id);

    let result = service::get_user_by_id(Arc::clone(&state.db_pool), user_id).await?;

    return Ok((StatusCode::OK, Json(result)));
}

/// `patch_user_with_id` allows a user to update their own information
/// so the the user_id stored in JWT will be used to identify the user to be updated,
/// instead of the user_id in the path parameter.
pub async fn patch_user_with_id(
    State(state): State<AppState>,
    Extension(user_token): Extension<UserToken>,
    Json(payload): Json<ReqPatchUser>,
) -> AppResult<impl IntoResponse> {
    tracing::trace!("Patch user payload: {:?}", payload);

    if user_token.user_id != payload.user_id {
        // user can only update their own information
        return Err(AppError::ServiceError(
            StatusCode::FORBIDDEN,
            "Cannot update other user's information".to_string(),
        ));
    }

    service::patch_user(Arc::clone(&state.db_pool), payload).await?;

    return Ok(StatusCode::NO_CONTENT);
}

use std::sync::Arc;

use axum::Extension;
use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;

use crate::dto::ReqPatchUser;
use crate::handler::middleware::UserToken;
use crate::handler::{AppError, AppState};
use crate::service::{self};

pub async fn get_user_info_by_id(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> impl IntoResponse {
    tracing::trace!("Get user info for user_id: {}", user_id);

    let result = match service::get_user_by_id(Arc::clone(&state.db_pool), user_id).await {
        Ok(user) => user,
        Err(err) => return AppError::from(err).into_response(),
    };

    return (StatusCode::OK, Json(result)).into_response();
}

/// `patch_user_with_id` allows a user to update their own information
/// so the the user_id stored in JWT will be used to identify the user to be updated,
/// instead of the user_id in the path parameter.
pub async fn patch_user_with_id(
    State(state): State<AppState>,
    Extension(user_token): Extension<UserToken>,
    Json(payload): Json<ReqPatchUser>,
) -> impl IntoResponse {
    tracing::trace!("Patch user payload: {:?}", payload);

    if user_token.user_id != payload.user_id {
        // user can only update their own information
        return AppError::ServiceError(
            StatusCode::FORBIDDEN,
            "Cannot update other user's information".to_string(),
        )
        .into_response();
    }

    match service::patch_user(Arc::clone(&state.db_pool), payload).await {
        Ok(_) => return StatusCode::NO_CONTENT.into_response(),
        Err(err) => return AppError::from(err).into_response(),
    };
}

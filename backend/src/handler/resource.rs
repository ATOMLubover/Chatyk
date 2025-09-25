use std::path::PathBuf;

use axum::{
    Json,
    extract::{Multipart, State},
    http::StatusCode,
    response::IntoResponse,
};

use crate::{
    dto::RspResourceInfo,
    handler::{AppError, AppResult, AppState},
    service,
};

pub async fn upload_resource(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> AppResult<impl IntoResponse> {
    let mut resources = Vec::new();

    while let Some(mut field) = multipart.next_field().await? {
        // skip fields without a name, or with an unexpected name
        match field.name() {
            Some(name) if name == "resources" => (),
            _ => continue,
        };
        // skip fields without a content type, or with an unexpected content type
        match field.content_type() {
            Some(ty) if ty == "application/octet-stream" => (),
            _ => continue,
        };
        // skip fields without a file name, or with an empty file name
        let file_name = match field.file_name() {
            Some(filename) if !filename.is_empty() => filename.to_string(),
            _ => continue,
        };

        let result = match service::save_resource(
            &mut field,
            file_name.to_string(),
            state.config.resource_base_url.clone(),
            state.config.upload_dir.clone(),
        )
        .await
        {
            Ok(info) => info,
            Err(err) => {
                tracing::error!("Failed to save resource: {:?}", err);

                // delete already uploaded resources in case of error
                if !resources.is_empty() {
                    if let Err(err) = service::delete_resources(
                        PathBuf::from(&state.config.upload_dir),
                        resources
                            .into_iter()
                            .map(|r: RspResourceInfo| r.id.clone())
                            .collect(),
                    )
                    .await
                    {
                        // FIXME: just log the error for now
                        tracing::error!("Failed to delete resources after upload error: {:?}", err);
                    }
                }

                return Err(AppError::from(err));
            }
        };

        resources.push(result);
    }

    return Ok((StatusCode::OK, Json(resources)));
}

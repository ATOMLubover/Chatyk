use std::path::{Path, PathBuf};

use axum::extract::multipart::Field;
use chrono::Utc;
use tokio::{fs::File, io::AsyncWriteExt as _};

use crate::{dto::RspResourceInfo, service::ServiceResult};

pub async fn save_resource(
    field: &mut Field<'_>,
    file_name: String,
    resource_base_url: String,
    upload_dir: String,
) -> ServiceResult<RspResourceInfo> {
    // preserve the file extension if any
    let ext = Path::new(&file_name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    let resource_id = crate::util::generate_id();

    let upload_path = PathBuf::from(&upload_dir).join(format!("{}.{}", &resource_id, ext));

    if let Some(p) = upload_path.parent() {
        // create dir if not exists
        std::fs::create_dir_all(p)?;
    };

    let mut file = File::create(&upload_path).await?;

    while let Some(chunk) = field.chunk().await? {
        file.write_all(&chunk).await?;
    }

    return Ok(RspResourceInfo {
        id: resource_id.clone(),
        url: format!("{}/{}", resource_base_url, &resource_id),
        uploaded_at: Utc::now().to_rfc3339().to_string(),
    });
}

/// `delete_resources` deletes resources with given IDs from the upload directory
/// this is typically useful when delete resources when a message is deleted
/// or some error occurs during resource upload
pub async fn delete_resources(upload_dir: PathBuf, resource_ids: Vec<String>) -> ServiceResult<()> {
    for resource_id in resource_ids.into_iter() {
        let path = upload_dir.join(&resource_id);
        if path.exists() {
            tokio::fs::remove_file(path).await?;
        }
    }

    return Ok(());
}

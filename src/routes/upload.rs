use axum::{
    Json,
    extract::{Multipart, State},
    http::StatusCode,
};
use serde::Serialize;

use crate::{app::AppState, storage::Storage, upload::save_multipart_file};

#[derive(Debug, Clone, Serialize)]
pub struct UploadResponse {
    pub job_id: String,
    pub filename: String,
    pub size_bytes: u64,
}

pub async fn upload_handler(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<UploadResponse>), (StatusCode, Json<serde_json::Value>)> {
    let mut filename = String::new();
    let mut target_format = None;
    let mut found_file = false;
    let mut size_bytes = 0;
    let storage = Storage::new(&state.settings.data_dir).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "storage unavailable" })),
        )
    })?;
    let temp_path = storage
        .temp
        .join(format!("upload-{}", uuid::Uuid::new_v4()));

    while let Some(field) = multipart.next_field().await.map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "invalid multipart payload" })),
        )
    })? {
        let name = field.name().unwrap_or("file").to_string();
        let file_name = field.file_name().map(str::to_string).unwrap_or_default();

        if name == "file" && !file_name.is_empty() {
            filename = file_name;
            found_file = true;
            let uploaded =
                save_multipart_file(field, &temp_path, &filename, state.settings.max_upload_size)
                    .await
                    .map_err(|err| {
                        let _ = std::fs::remove_file(&temp_path);
                        (
                            if err == "file too large" {
                                StatusCode::PAYLOAD_TOO_LARGE
                            } else {
                                StatusCode::BAD_REQUEST
                            },
                            Json(serde_json::json!({ "error": err })),
                        )
                    })?;
            size_bytes = uploaded.size_bytes;
        } else if name == "to" {
            target_format = Some(field.text().await.map_err(|_| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "error": "invalid target format" })),
                )
            })?);
        }
    }

    if !found_file || filename.is_empty() {
        let _ = std::fs::remove_file(&temp_path);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "no file uploaded" })),
        ));
    }

    let safe_name = Storage::ensure_safe_filename(&filename).map_err(|_| {
        let _ = std::fs::remove_file(&temp_path);
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "unsafe filename" })),
        )
    })?;

    let source_format = std::path::Path::new(&safe_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| {
            let _ = std::fs::remove_file(&temp_path);
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "file extension is required" })),
            )
        })?;
    let target_format = target_format
        .unwrap_or_else(|| "jpg".to_string())
        .trim()
        .to_ascii_lowercase();

    if target_format.is_empty() || target_format.contains('.') || target_format.contains('/') {
        let _ = std::fs::remove_file(&temp_path);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "invalid target format" })),
        ));
    }

    if !crate::worker::is_supported_conversion(&source_format, &target_format) {
        let _ = std::fs::remove_file(&temp_path);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("unsupported conversion: {source_format} -> {target_format}")
            })),
        ));
    }

    let job = state
        .jobs
        .create_job(&safe_name, &source_format, &target_format);
    let job_dir = storage.create_job_dir(&job.id).map_err(|_| {
        let _ = std::fs::remove_file(&temp_path);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "failed to create job directory" })),
        )
    })?;

    let saved_path = job_dir.join(&safe_name);
    std::fs::rename(&temp_path, &saved_path).map_err(|err| {
        let _ = std::fs::remove_file(&temp_path);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("failed to store upload: {err}") })),
        )
    })?;

    if state.queue.enqueue(job.id.clone()).await.is_err() {
        state.jobs.set_error(&job.id, "failed to enqueue job");
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "failed to enqueue job" })),
        ));
    }

    Ok((
        StatusCode::OK,
        Json(UploadResponse {
            job_id: job.id,
            filename: safe_name,
            size_bytes,
        }),
    ))
}

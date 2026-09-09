use axum::{
    Json,
    extract::{Multipart, State},
    http::StatusCode,
};
use serde::Serialize;

use crate::{app::AppState, storage::Storage, upload::save_uploaded_file};

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
    let mut file_bytes = Vec::new();
    let mut filename = String::new();
    let mut target_format = None;
    let mut found_file = false;

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
            let data = field.bytes().await.map_err(|_| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "error": "unable to read uploaded file" })),
                )
            })?;
            file_bytes.extend_from_slice(&data);
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
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "no file uploaded" })),
        ));
    }

    let safe_name = Storage::ensure_safe_filename(&filename).map_err(|_| {
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
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "invalid target format" })),
        ));
    }

    if !crate::worker::is_supported_conversion(&source_format, &target_format) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("unsupported conversion: {source_format} -> {target_format}")
            })),
        ));
    }

    if file_bytes.len() as u64 > state.settings.max_upload_size {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(serde_json::json!({ "error": "file too large" })),
        ));
    }

    let storage = Storage::new(&state.settings.data_dir).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "storage unavailable" })),
        )
    })?;

    let job = state
        .jobs
        .create_job(&safe_name, &source_format, &target_format);
    let job_dir = storage.create_job_dir(&job.id).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "failed to create job directory" })),
        )
    })?;

    let saved_path = job_dir.join(&safe_name);
    let uploaded = save_uploaded_file(
        &file_bytes,
        &saved_path,
        &safe_name,
        state.settings.max_upload_size,
    )
    .map_err(|err| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": err })),
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
            filename: uploaded.safe_name,
            size_bytes: uploaded.size_bytes,
        }),
    ))
}

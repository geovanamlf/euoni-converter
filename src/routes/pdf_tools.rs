use crate::{
    app::AppState,
    pdf_tools::{PdfManifest, PdfOperation},
    storage::Storage,
    upload::{save_multipart_file, validate_uploaded_format},
};
use axum::{
    Json,
    extract::{Multipart, State},
    http::StatusCode,
};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PdfOperationResponse {
    pub job_id: String,
    pub token: String,
    pub files: usize,
    pub size_bytes: u64,
}

pub async fn pdf_operation_handler(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<PdfOperationResponse>), (StatusCode, Json<serde_json::Value>)> {
    let storage = Storage::new(&state.settings.data_dir)
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "storage unavailable"))?;
    let temp_dir = storage
        .temp
        .join(format!("pdf-operation-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_dir)
        .await
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "storage unavailable"))?;
    let mut operation: Option<PdfOperation> = None;
    let mut names = Vec::new();
    let mut watermark_image = None;
    let mut size_bytes = 0u64;
    let result = async {
        while let Some(field) = multipart
            .next_field()
            .await
            .map_err(|_| error(StatusCode::BAD_REQUEST, "invalid multipart payload"))?
        {
            let name = field.name().unwrap_or_default().to_string();
            if name == "operation" {
                if operation.is_some() {
                    return Err(error(
                        StatusCode::BAD_REQUEST,
                        "operation must be provided once",
                    ));
                }
                let raw = field
                    .text()
                    .await
                    .map_err(|_| error(StatusCode::BAD_REQUEST, "invalid operation"))?;
                operation = Some(
                    serde_json::from_str(&raw)
                        .map_err(|_| error(StatusCode::BAD_REQUEST, "invalid PDF operation"))?,
                );
                continue;
            }
            if name == "watermark_image" {
                if watermark_image.is_some() {
                    return Err(error(
                        StatusCode::BAD_REQUEST,
                        "watermark image must be provided once",
                    ));
                }
                let original = field.file_name().map(str::to_string).unwrap_or_default();
                let safe = Storage::ensure_safe_filename(&original)
                    .map_err(|_| error(StatusCode::BAD_REQUEST, "unsafe watermark filename"))?;
                if !safe.to_ascii_lowercase().ends_with(".png") {
                    return Err(error(
                        StatusCode::BAD_REQUEST,
                        "watermark image must be PNG",
                    ));
                }
                let path = temp_dir.join(&safe);
                let uploaded =
                    save_multipart_file(field, &path, &safe, state.settings.max_upload_size)
                        .await
                        .map_err(|message| error(StatusCode::BAD_REQUEST, &message))?;
                size_bytes = size_bytes
                    .checked_add(uploaded.size_bytes)
                    .ok_or_else(|| error(StatusCode::PAYLOAD_TOO_LARGE, "upload too large"))?;
                if size_bytes > state.settings.max_upload_size {
                    return Err(error(
                        StatusCode::PAYLOAD_TOO_LARGE,
                        "total upload is too large",
                    ));
                }
                validate_uploaded_format(&path, "png")
                    .map_err(|message| error(StatusCode::BAD_REQUEST, &message))?;
                watermark_image = Some(safe);
                continue;
            }
            if name != "file" && name != "files" {
                return Err(error(StatusCode::BAD_REQUEST, "unexpected multipart field"));
            }
            if names.len() >= 32 {
                return Err(error(
                    StatusCode::BAD_REQUEST,
                    "a PDF operation accepts at most 32 files",
                ));
            }
            let original = field.file_name().map(str::to_string).unwrap_or_default();
            let safe = Storage::ensure_safe_filename(&original)
                .map_err(|_| error(StatusCode::BAD_REQUEST, "unsafe filename"))?;
            if !safe.to_ascii_lowercase().ends_with(".pdf") {
                return Err(error(
                    StatusCode::BAD_REQUEST,
                    "PDF filenames must end in .pdf",
                ));
            }
            let stored_name = unique_filename(&safe, &names);
            let path = temp_dir.join(&stored_name);
            let uploaded =
                save_multipart_file(field, &path, &stored_name, state.settings.max_upload_size)
                    .await
                    .map_err(|message| {
                        error(
                            if message == "file too large" {
                                StatusCode::PAYLOAD_TOO_LARGE
                            } else {
                                StatusCode::BAD_REQUEST
                            },
                            &message,
                        )
                    })?;
            size_bytes = size_bytes
                .checked_add(uploaded.size_bytes)
                .ok_or_else(|| error(StatusCode::PAYLOAD_TOO_LARGE, "upload too large"))?;
            if size_bytes > state.settings.max_upload_size {
                return Err(error(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "total upload is too large",
                ));
            }
            validate_uploaded_format(&path, "pdf")
                .map_err(|message| error(StatusCode::BAD_REQUEST, &message))?;
            names.push(stored_name);
        }

        fn unique_filename(filename: &str, existing: &[String]) -> String {
            if !existing.iter().any(|entry| entry == filename) {
                return filename.to_string();
            }
            let path = std::path::Path::new(filename);
            let stem = path
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("file");
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("pdf");
            for index in 2..=32 {
                let candidate = format!("{stem}-{index}.{extension}");
                if !existing.iter().any(|entry| entry == &candidate) {
                    return candidate;
                }
            }
            filename.to_string()
        }

        let operation =
            operation.ok_or_else(|| error(StatusCode::BAD_REQUEST, "operation is required"))?;
        validate_operation(&operation, names.len(), watermark_image.is_some())
            .map_err(|message| error(StatusCode::BAD_REQUEST, &message))?;
        let manifest = PdfManifest {
            operation,
            files: names.clone(),
            watermark_image: watermark_image.clone(),
        };
        let manifest_bytes = serde_json::to_vec(&manifest).map_err(|_| {
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to encode operation",
            )
        })?;
        tokio::fs::write(temp_dir.join("operation.json"), manifest_bytes)
            .await
            .map_err(|_| {
                error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to store operation",
                )
            })?;

        let first = names
            .first()
            .cloned()
            .unwrap_or_else(|| "source.pdf".to_string());
        let (job, token) = state
            .jobs
            .create_job_with_token(&first, "pdf", "pdf-operation")
            .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "failed to create job"))?;
        let job_dir = match storage.create_job_dir(&job.id) {
            Ok(path) => path,
            Err(_) => {
                state.jobs.delete(&job.id);
                return Err(error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to create job directory",
                ));
            }
        };
        for name in &names {
            if tokio::fs::rename(temp_dir.join(name), job_dir.join(name))
                .await
                .is_err()
            {
                let _ = tokio::fs::remove_dir_all(&job_dir).await;
                state.jobs.delete(&job.id);
                return Err(error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to store PDF",
                ));
            }
            if let Some(name) = &watermark_image
                && tokio::fs::rename(temp_dir.join(name), job_dir.join(name))
                    .await
                    .is_err()
            {
                let _ = tokio::fs::remove_dir_all(&job_dir).await;
                state.jobs.delete(&job.id);
                return Err(error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to store watermark image",
                ));
            }
        }
        if tokio::fs::rename(
            temp_dir.join("operation.json"),
            job_dir.join("operation.json"),
        )
        .await
        .is_err()
        {
            let _ = tokio::fs::remove_dir_all(&job_dir).await;
            state.jobs.delete(&job.id);
            return Err(error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to store operation",
            ));
        }
        if state.queue.enqueue(job.id.clone()).await.is_err() {
            let _ = tokio::fs::remove_dir_all(&job_dir).await;
            state.jobs.delete(&job.id);
            return Err(error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to enqueue job",
            ));
        }
        Ok((
            StatusCode::OK,
            Json(PdfOperationResponse {
                job_id: job.id,
                token,
                files: names.len(),
                size_bytes,
            }),
        ))
    }
    .await;
    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    result
}

fn validate_operation(
    operation: &PdfOperation,
    files: usize,
    has_watermark_image: bool,
) -> Result<(), String> {
    if has_watermark_image && !matches!(operation, PdfOperation::Watermark { .. }) {
        return Err("watermark image is only valid for watermark operations".to_string());
    }
    match operation {
        PdfOperation::Merge if files < 2 => {
            Err("merge requires at least two PDF files".to_string())
        }
        PdfOperation::Merge => Ok(()),
        PdfOperation::Split { .. }
        | PdfOperation::Reorder { .. }
        | PdfOperation::Rotate { .. }
        | PdfOperation::Compress { .. }
        | PdfOperation::Watermark { .. }
            if files != 1 =>
        {
            Err("this operation requires exactly one PDF file".to_string())
        }
        PdfOperation::Split { .. }
        | PdfOperation::Reorder { .. }
        | PdfOperation::Rotate { .. }
        | PdfOperation::Compress { .. } => Ok(()),
        PdfOperation::Watermark { text, image, .. } => {
            if text.is_some() == image.is_some() || image.is_some() != has_watermark_image {
                Err("watermark requires exactly one text or PNG image".to_string())
            } else {
                Ok(())
            }
        }
    }
}

fn error(status: StatusCode, message: &str) -> (StatusCode, Json<serde_json::Value>) {
    (status, Json(serde_json::json!({ "error": message })))
}

use axum::{
    extract::{Path, State},
    http::{StatusCode, header},
    response::Response,
};

use crate::app::AppState;

pub async fn download_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, (StatusCode, String)> {
    let Some(job) = state.jobs.get(&id) else {
        return Err((StatusCode::NOT_FOUND, "job not found".to_string()));
    };

    let output_name = match job.output_name {
        Some(ref output) => output.clone(),
        None => return Err((StatusCode::NOT_FOUND, "job has no output yet".to_string())),
    };

    let path = std::path::Path::new(&state.settings.data_dir)
        .join("jobs")
        .join(&id)
        .join(output_name);

    if !path.exists() {
        return Err((StatusCode::NOT_FOUND, "output file not found".to_string()));
    }

    let file = std::fs::read(&path).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "unable to read output file".to_string(),
        )
    })?;

    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("output");
    let content_type = match path.extension().and_then(|extension| extension.to_str()) {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("mp4") => "video/mp4",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    };

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .body(axum::body::Body::from(file))
        .unwrap())
}

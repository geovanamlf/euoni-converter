use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};

use crate::storage::Storage;
use crate::{app::AppState, jobs::Job};

#[derive(Debug, Clone, Deserialize)]
pub struct CreateJobRequest {
    pub input_name: String,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateJobResponse {
    pub job: Job,
    pub token: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct JobResponse {
    pub job: Job,
}

#[derive(Debug, Clone, Serialize)]
pub struct JobStatusResponse {
    pub id: String,
    pub status: String,
    pub progress: u8,
    pub error: Option<String>,
}

pub async fn create_job_handler(
    State(state): State<AppState>,
    Json(request): Json<CreateJobRequest>,
) -> Result<(StatusCode, Json<CreateJobResponse>), (StatusCode, Json<serde_json::Value>)> {
    let input_name = Storage::ensure_safe_filename(&request.input_name).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "unsafe input filename" })),
        )
    })?;
    let from = request.from.trim().to_ascii_lowercase();
    let to = request.to.trim().to_ascii_lowercase();
    if !crate::worker::is_supported_conversion(&from, &to)
        || std::path::Path::new(&input_name)
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase() != from)
            .unwrap_or(true)
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "unsupported conversion" })),
        ));
    }
    let store = state.jobs.clone();
    let (job, token) = store
        .create_job_with_token(&input_name, &from, &to)
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "failed to create job" })),
            )
        })?;

    if state.queue.enqueue(job.id.clone()).await.is_err() {
        state.jobs.set_error(&job.id, "failed to enqueue job");
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "failed to enqueue job" })),
        ));
    }

    Ok((StatusCode::CREATED, Json(CreateJobResponse { job, token })))
}

pub async fn get_job_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<JobResponse>, (StatusCode, Json<serde_json::Value>)> {
    match authorized_job(&state, &id, &headers) {
        Ok(job) => Ok(Json(JobResponse { job })),
        Err(error) => Err(error),
    }
}

pub async fn get_job_status_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<JobStatusResponse>, (StatusCode, Json<serde_json::Value>)> {
    match authorized_job(&state, &id, &headers) {
        Ok(job) => Ok(Json(JobStatusResponse {
            id: job.id.clone(),
            status: format!("{:?}", job.status).to_ascii_lowercase(),
            progress: job.progress,
            error: job.error.clone(),
        })),
        Err(error) => Err(error),
    }
}

fn authorized_job(
    state: &AppState,
    id: &str,
    headers: &HeaderMap,
) -> Result<Job, (StatusCode, Json<serde_json::Value>)> {
    let token = headers
        .get("x-euoni-job-token")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if !state.jobs.authorize(id, token) {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "job not found" })),
        ));
    }
    state.jobs.get(id).ok_or((
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({ "error": "job not found" })),
    ))
}

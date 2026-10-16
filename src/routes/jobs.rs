use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use serde::Serialize;

use crate::{app::AppState, jobs::Job};

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

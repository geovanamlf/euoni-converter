use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

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
    let store = state.jobs.clone();
    let job = store.create_job(&request.input_name, &request.from, &request.to);

    if state.queue.enqueue(job.id.clone()).await.is_err() {
        state.jobs.set_error(&job.id, "failed to enqueue job");
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "failed to enqueue job" })),
        ));
    }

    Ok((StatusCode::CREATED, Json(CreateJobResponse { job })))
}

pub async fn get_job_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<JobResponse>, (StatusCode, Json<serde_json::Value>)> {
    match state.jobs.get(&id) {
        Some(job) => Ok(Json(JobResponse { job })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "job not found" })),
        )),
    }
}

pub async fn get_job_status_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<JobStatusResponse>, (StatusCode, Json<serde_json::Value>)> {
    match state.jobs.get(&id) {
        Some(job) => Ok(Json(JobStatusResponse {
            id: job.id.clone(),
            status: format!("{:?}", job.status).to_ascii_lowercase(),
            progress: job.progress,
            error: job.error.clone(),
        })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "job not found" })),
        )),
    }
}

pub async fn list_jobs_handler(State(state): State<AppState>) -> impl IntoResponse {
    let jobs = state.jobs.list();
    (StatusCode::OK, Json(serde_json::json!({ "jobs": jobs })))
}

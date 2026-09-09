use axum::{Json, extract::State, http::StatusCode};

use crate::app::AppState;

#[derive(serde::Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub ffmpeg: bool,
    pub workers: WorkersSummary,
}

#[derive(serde::Serialize)]
pub struct WorkersSummary {
    pub active: usize,
    pub max: usize,
}

pub async fn health_handler(State(_state): State<AppState>) -> (StatusCode, Json<HealthResponse>) {
    let response = HealthResponse {
        status: "ok",
        ffmpeg: which::which("ffmpeg").is_ok(),
        workers: WorkersSummary {
            active: 0,
            max: _state.settings.max_workers,
        },
    };

    (StatusCode::OK, Json(response))
}

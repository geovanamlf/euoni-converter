use axum::{Json, extract::State};

use crate::app::AppState;

#[derive(serde::Serialize)]
pub struct SystemResponse {
    pub host: String,
    pub port: u16,
    pub max_upload_size: u64,
    pub max_workers: usize,
    pub data_dir: String,
    pub job_retention_hours: u64,
}

pub async fn system_handler(State(state): State<AppState>) -> Json<SystemResponse> {
    Json(SystemResponse {
        host: state.settings.host.clone(),
        port: state.settings.port,
        max_upload_size: state.settings.max_upload_size,
        max_workers: state.settings.max_workers,
        data_dir: state.settings.data_dir.clone(),
        job_retention_hours: state.settings.job_retention_hours,
    })
}

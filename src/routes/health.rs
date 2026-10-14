use axum::{Json, extract::State, http::StatusCode};

use crate::app::AppState;

#[derive(serde::Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
}

pub async fn health_handler(State(_state): State<AppState>) -> (StatusCode, Json<HealthResponse>) {
    (StatusCode::OK, Json(HealthResponse { status: "ok" }))
}

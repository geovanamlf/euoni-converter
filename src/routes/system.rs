use axum::{Json, extract::State};

use crate::app::AppState;

#[derive(serde::Serialize)]
pub struct SystemResponse {
    pub status: &'static str,
}

pub async fn system_handler(State(state): State<AppState>) -> Json<SystemResponse> {
    let _ = state;
    Json(SystemResponse { status: "ok" })
}

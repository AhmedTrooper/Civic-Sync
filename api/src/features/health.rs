use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::Serialize;

use crate::state::AppState;

#[derive(Serialize)]
pub struct LiveResponse {
    pub status: &'static str,
    pub service: &'static str,
}

pub async fn live() -> impl IntoResponse {
    Json(LiveResponse {
        status: "ok",
        service: "civic-sync-api",
    })
}

pub async fn ready(State(state): State<AppState>) -> impl IntoResponse {
    match state.database.as_ref() {
        Some(pool) => match sqlx::query("SELECT 1").execute(pool).await {
            Ok(_) => (
                StatusCode::OK,
                Json(serde_json::json!({"status": "ready", "database": "up"})),
            )
                .into_response(),
            Err(err) => {
                tracing::warn!(error = %err, "database ping failed");
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(serde_json::json!({"status": "not_ready", "database": "down"})),
                )
                    .into_response()
            }
        },
        None => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "ready", "database": "in_memory"})),
        )
            .into_response(),
    }
}

//! HTTP 引导探针；业务请求只由正式 RPC adapter 处理。
use crate::{AppState, error::ApiError};
use axum::{Json, Router, extract::State, routing::get};
use kanban_protocol::HealthResponse;

pub(crate) fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Result<Json<HealthResponse>, ApiError> {
    crate::application::health::get_health(state)
        .await
        .map(Json)
}

use crate::{error::ApiError, state::AppState};
use axum::{Json, Router, extract::State, routing::get};
use kanban_protocol::HealthResponse;

pub(crate) async fn health(
    State(state): State<AppState>,
) -> Result<Json<HealthResponse>, ApiError> {
    crate::application::health::get_health(state)
        .await
        .map(Json)
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        crate::http::operations::registered_path(kanban_protocol::HttpMethod::Get, "/health"),
        get(health),
    )
}

#[cfg(test)]
mod tests {}

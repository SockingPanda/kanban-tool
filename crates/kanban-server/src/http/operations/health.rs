use crate::{error::ApiError, state::AppState};
use axum::{Json, Router, extract::State, routing::get};
use kanban_protocol::{HealthReport, HealthResponse};
use std::time::UNIX_EPOCH;

pub(crate) async fn health(
    State(state): State<AppState>,
) -> Result<Json<HealthResponse>, ApiError> {
    state.application().health().await?;
    let metadata = tokio::fs::metadata(state.db_path())
        .await
        .map_err(|error| kanban_core::KanbanError::Storage(error.to_string()))?;
    let modified_ms = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    Ok(Json(HealthResponse::new(HealthReport {
        ok: true,
        db: "turso".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        db_path: state.db_path().display().to_string(),
        db_fingerprint: format!("turso:{}:{modified_ms}", metadata.len()),
    })))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/health", get(health))
}

#[cfg(test)]
mod tests {}

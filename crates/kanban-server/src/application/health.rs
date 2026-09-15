//! 原生客户端与启动探测共用的健康报告。

use std::time::UNIX_EPOCH;

use kanban_protocol::{HealthReport, HealthResponse};

use crate::{AppState, error::ApiError};

pub(crate) async fn get_health(state: AppState) -> Result<HealthResponse, ApiError> {
    state.application().health().await?;
    let metadata = tokio::fs::metadata(state.db_path())
        .await
        .map_err(|error| kanban_service::KanbanError::Storage(error.to_string()))?;
    let modified_ms = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    Ok(HealthResponse::new(HealthReport {
        ok: true,
        db: "turso".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        db_path: state.db_path().display().to_string(),
        db_fingerprint: format!("turso:{}:{modified_ms}", metadata.len()),
    }))
}

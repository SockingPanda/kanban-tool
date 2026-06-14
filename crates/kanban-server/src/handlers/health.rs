use std::time::UNIX_EPOCH;

use axum::{Json, extract::State};
use serde_json::json;

use crate::{
    dto::Envelope,
    error::{ApiError, invalid_input},
    state::AppState,
};

pub(crate) async fn health(
    State(state): State<AppState>,
) -> Result<Json<Envelope<serde_json::Value>>, ApiError> {
    let metadata = std::fs::metadata(state.db_path()).map_err(|error| {
        invalid_input(format!(
            "database file is unreadable: {} ({error})",
            state.db_path().display()
        ))
    })?;
    let modified_ms = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let report = kanban_sqlite::doctor_database(state.db_path())?;
    if !report.ok {
        return Err(invalid_input(format!(
            "database failed health check: {}",
            state.db_path().display()
        )));
    }
    Ok(Json(Envelope {
        data: json!({
            "ok": true,
            "db": "ok",
            "version": env!("CARGO_PKG_VERSION"),
            "db_path": state.db_path().display().to_string(),
            "db_fingerprint": format!("sqlite:{}:{modified_ms}", metadata.len()),
        }),
        meta: None,
    }))
}

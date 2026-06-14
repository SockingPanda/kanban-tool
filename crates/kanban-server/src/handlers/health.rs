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
    if !state.db_path().is_file() {
        return Err(invalid_input(format!(
            "database file is missing: {}",
            state.db_path().display()
        )));
    }
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
    let _conn = kanban_sqlite::connect_file(state.db_path())?;
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

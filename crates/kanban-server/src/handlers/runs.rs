use std::fs;

use axum::{
    Json,
    extract::{Path, State},
};
use kanban_core::KanbanError;

use crate::dto::{Envelope, RunDto, RunLogDto};
use crate::error::ApiError;
use crate::state::AppState;

pub(crate) async fn list_runs(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<Envelope<Vec<RunDto>>>, ApiError> {
    let task = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &task_id)?;
    Ok(Json(Envelope {
        data: kanban_sqlite::api::list_runs(state.db_path(), &task.board_id, Some(&task_id))?
            .into_iter()
            .map(RunDto::from)
            .collect(),
        meta: None,
    }))
}

pub(crate) async fn get_run(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
) -> Result<Json<Envelope<RunDto>>, ApiError> {
    Ok(Json(Envelope {
        data: RunDto::from(kanban_sqlite::api::get_run_by_id_global(
            state.db_path(),
            &run_id,
        )?),
        meta: None,
    }))
}

pub(crate) async fn get_run_log(
    State(state): State<AppState>,
    Path(run_id): Path<String>,
) -> Result<Json<Envelope<RunLogDto>>, ApiError> {
    const MAX_RUN_LOG_BYTES: usize = 256 * 1024;
    let run = kanban_sqlite::api::get_run_by_id_global(state.db_path(), &run_id)?;
    let log_path = run
        .log_path
        .as_deref()
        .ok_or_else(|| KanbanError::NotFound(format!("run log {run_id}")))?;
    let path = kanban_sqlite::api::resolve_run_log_path(state.db_path(), &run_id, log_path)?;
    let bytes = fs::read(path).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => KanbanError::NotFound(format!("run log {run_id}")),
        _ => KanbanError::Storage(error.to_string()),
    })?;
    let truncated = bytes.len() > MAX_RUN_LOG_BYTES;
    let start = if truncated {
        bytes.len() - MAX_RUN_LOG_BYTES
    } else {
        0
    };
    let content = String::from_utf8_lossy(&bytes[start..]).into_owned();
    Ok(Json(Envelope {
        data: RunLogDto {
            run_id,
            content,
            truncated,
        },
        meta: None,
    }))
}

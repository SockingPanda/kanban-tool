use kanban_contract::{
    ApiRun, ApiRunLog, ApiRunStatus, GetRunLogPath, GetRunLogResponse, GetRunPath, GetRunResponse,
    ListRunsPath, ListRunsResponse,
};
use std::fs;

use axum::{
    Json,
    extract::{Path, State},
};
use kanban_core::{KanbanError, Result as KanbanResult};

use crate::error::ApiError;
use crate::state::AppState;

pub(crate) fn api_run(run: kanban_sqlite::api::RunRecord) -> KanbanResult<ApiRun> {
    let metadata = serde_json::from_str(&run.metadata_json).map_err(|error| {
        KanbanError::Storage(format!("run {} has invalid metadata_json: {error}", run.id))
    })?;
    Ok(ApiRun {
        id: run.id,
        task_id: run.task_id,
        status: ApiRunStatus::try_from(run.status.as_str()).map_err(KanbanError::Storage)?,
        worker_profile: run.worker_profile,
        worker_pid: run.worker_pid,
        claim_owner: run.claim_owner,
        started_at: run.started_at,
        finished_at: run.finished_at,
        exit_code: run.exit_code,
        summary: run.summary,
        error: run.error,
        has_log: run.log_path.is_some(),
        metadata,
    })
}

pub(crate) async fn list_runs(
    State(state): State<AppState>,
    Path(path): Path<ListRunsPath>,
) -> Result<Json<ListRunsResponse>, ApiError> {
    let task = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &path.task_id)?;
    let data = kanban_sqlite::api::list_runs(state.db_path(), &task.board_id, Some(&path.task_id))?
        .into_iter()
        .map(api_run)
        .collect::<KanbanResult<Vec<_>>>()?;
    Ok(Json(ListRunsResponse { data }))
}

pub(crate) async fn get_run(
    State(state): State<AppState>,
    Path(path): Path<GetRunPath>,
) -> Result<Json<GetRunResponse>, ApiError> {
    let data = api_run(kanban_sqlite::api::get_run_by_id_global(
        state.db_path(),
        &path.run_id,
    )?)?;
    Ok(Json(GetRunResponse { data }))
}

pub(crate) async fn get_run_log(
    State(state): State<AppState>,
    Path(path): Path<GetRunLogPath>,
) -> Result<Json<GetRunLogResponse>, ApiError> {
    const MAX_RUN_LOG_BYTES: usize = 256 * 1024;
    let run = kanban_sqlite::api::get_run_by_id_global(state.db_path(), &path.run_id)?;
    let log_path = run
        .log_path
        .as_deref()
        .ok_or_else(|| KanbanError::NotFound(format!("run log {}", path.run_id)))?;
    let log_file =
        kanban_sqlite::api::resolve_run_log_path(state.db_path(), &path.run_id, log_path)?;
    let bytes = fs::read(log_file).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => KanbanError::NotFound(format!("run log {}", path.run_id)),
        _ => KanbanError::Storage(error.to_string()),
    })?;
    let truncated = bytes.len() > MAX_RUN_LOG_BYTES;
    let start = if truncated {
        bytes.len() - MAX_RUN_LOG_BYTES
    } else {
        0
    };
    let content = String::from_utf8_lossy(&bytes[start..]).into_owned();
    Ok(Json(GetRunLogResponse {
        data: ApiRunLog {
            run_id: path.run_id,
            content,
            truncated,
        },
    }))
}

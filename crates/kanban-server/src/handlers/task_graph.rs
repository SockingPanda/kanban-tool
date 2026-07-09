use axum::{
    Json,
    extract::{Path, Query, State, rejection::QueryRejection},
};
use serde::Deserialize;

use crate::dto::{BoardTaskMapDto, Envelope, TaskNeighborhoodDto};
use crate::error::{ApiError, extractor_error};
use crate::state::AppState;

fn default_depth() -> usize {
    1
}

fn default_limit_nodes() -> usize {
    250
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub(crate) struct TaskNeighborhoodQuery {
    #[serde(default = "default_depth")]
    depth: usize,
    #[serde(default = "default_limit_nodes")]
    limit_nodes: usize,
    #[serde(default)]
    include_archived_context: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BoardTaskMapQuery {
    #[serde(default = "default_true")]
    active_only: bool,
    #[serde(default = "default_depth")]
    context_depth: usize,
    #[serde(default = "default_limit_nodes")]
    limit_nodes: usize,
    #[serde(default = "default_true")]
    include_done_context: bool,
    #[serde(default)]
    include_archived_context: bool,
    #[serde(default)]
    hide_isolated: bool,
}

pub(crate) async fn task_neighborhood(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    query: Result<Query<TaskNeighborhoodQuery>, QueryRejection>,
) -> Result<Json<Envelope<TaskNeighborhoodDto>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    let graph = kanban_sqlite::api::task_neighborhood(
        state.db_path(),
        &task_id,
        kanban_sqlite::api::TaskNeighborhoodOptions {
            depth: query.depth,
            limit_nodes: query.limit_nodes,
            include_archived_context: query.include_archived_context,
        },
    )?;
    Ok(Json(Envelope {
        data: TaskNeighborhoodDto::from(graph),
        meta: None,
    }))
}

pub(crate) async fn board_task_map(
    State(state): State<AppState>,
    Path(board): Path<String>,
    query: Result<Query<BoardTaskMapQuery>, QueryRejection>,
) -> Result<Json<Envelope<BoardTaskMapDto>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    let graph = kanban_sqlite::api::board_task_map(
        state.db_path(),
        &board,
        kanban_sqlite::api::BoardTaskMapOptions {
            active_only: query.active_only,
            context_depth: query.context_depth,
            limit_nodes: query.limit_nodes,
            include_done_context: query.include_done_context,
            include_archived_context: query.include_archived_context,
            hide_isolated: query.hide_isolated,
        },
    )?;
    Ok(Json(Envelope {
        data: BoardTaskMapDto::from(graph),
        meta: None,
    }))
}

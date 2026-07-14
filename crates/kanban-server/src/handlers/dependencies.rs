use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
};
use kanban_contract::{
    AddDependencyPath, AddDependencyRequest, AddDependencyResponse, ListDependenciesPath,
    ListDependenciesResponse, RemoveDependencyPath, RemoveDependencyResponse,
};

use crate::error::{ApiError, extractor_error};
use crate::state::AppState;

use super::shared::{actor, dependencies_dto};

pub(crate) async fn add_dependency(
    State(state): State<AppState>,
    Path(path): Path<AddDependencyPath>,
    headers: HeaderMap,
    body: Result<Json<AddDependencyRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<AddDependencyResponse>), ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let child = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &path.task_id)?;
    let outcome = kanban_sqlite::api::add_dependency_with_outcome(
        state.db_path(),
        &child.board_id,
        &actor,
        &body.parent_task_id,
        &path.task_id,
    )?;
    let status = match outcome {
        kanban_sqlite::api::AddDependencyOutcome::Added => StatusCode::CREATED,
        kanban_sqlite::api::AddDependencyOutcome::AlreadyExists => StatusCode::OK,
    };
    Ok((
        status,
        Json(AddDependencyResponse {
            data: dependencies_dto(&state, &path.task_id)?,
        }),
    ))
}

pub(crate) async fn remove_dependency(
    State(state): State<AppState>,
    Path(path): Path<RemoveDependencyPath>,
    headers: HeaderMap,
) -> Result<Json<RemoveDependencyResponse>, ApiError> {
    let actor = actor(None, &headers, &state);
    let child = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &path.child_task_id)?;
    kanban_sqlite::api::remove_dependency(
        state.db_path(),
        &child.board_id,
        &actor,
        &path.parent_task_id,
        &path.child_task_id,
    )?;
    Ok(Json(RemoveDependencyResponse {
        data: dependencies_dto(&state, &path.child_task_id)?,
    }))
}

pub(crate) async fn list_dependencies(
    State(state): State<AppState>,
    Path(path): Path<ListDependenciesPath>,
) -> Result<Json<ListDependenciesResponse>, ApiError> {
    Ok(Json(ListDependenciesResponse {
        data: dependencies_dto(&state, &path.task_id)?,
    }))
}

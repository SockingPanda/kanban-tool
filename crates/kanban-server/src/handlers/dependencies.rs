use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
};

use crate::dto::{DependenciesDto, Envelope};
use crate::error::{ApiError, extractor_error};
use crate::state::AppState;

use super::shared::{AddDependencyBody, actor, dependencies_dto};

pub(crate) async fn add_dependency(
    State(state): State<AppState>,
    Path(child_task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<AddDependencyBody>, JsonRejection>,
) -> Result<(StatusCode, Json<Envelope<DependenciesDto>>), ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let child = kanban_sqlite::get_task_by_id_global(state.db_path(), &child_task_id)?;
    let outcome = kanban_sqlite::add_dependency_with_outcome(
        state.db_path(),
        &child.board_id,
        &actor,
        &body.parent_task_id,
        &child_task_id,
    )?;
    let status = match outcome {
        kanban_sqlite::AddDependencyOutcome::Added => StatusCode::CREATED,
        kanban_sqlite::AddDependencyOutcome::AlreadyExists => StatusCode::OK,
    };
    Ok((
        status,
        Json(Envelope {
            data: dependencies_dto(&state, &child_task_id)?,
            meta: None,
        }),
    ))
}

pub(crate) async fn remove_dependency(
    State(state): State<AppState>,
    Path((child_task_id, parent_task_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<Envelope<DependenciesDto>>, ApiError> {
    let actor = actor(None, &headers, &state);
    let child = kanban_sqlite::get_task_by_id_global(state.db_path(), &child_task_id)?;
    kanban_sqlite::remove_dependency(
        state.db_path(),
        &child.board_id,
        &actor,
        &parent_task_id,
        &child_task_id,
    )?;
    Ok(Json(Envelope {
        data: dependencies_dto(&state, &child_task_id)?,
        meta: None,
    }))
}

pub(crate) async fn list_dependencies(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<Envelope<DependenciesDto>>, ApiError> {
    Ok(Json(Envelope {
        data: dependencies_dto(&state, &task_id)?,
        meta: None,
    }))
}

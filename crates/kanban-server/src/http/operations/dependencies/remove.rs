use super::super::support::request_actor;
use super::support::*;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::delete,
};
use kanban_service::RemoveDependencyCommand;
use kanban_protocol::{RemoveDependencyPath, RemoveDependencyResponse};

pub(crate) async fn remove_dependency(
    State(state): State<AppState>,
    Path(RemoveDependencyPath {
        child_task_id,
        parent_task_id,
    }): Path<RemoveDependencyPath>,
    headers: HeaderMap,
) -> Result<Json<RemoveDependencyResponse>, ApiError> {
    let actor = request_actor(None, &headers, state.default_actor())?;
    let result = state
        .application()
        .remove_dependency(RemoveDependencyCommand {
            child_task_id,
            parent_task_id,
            actor,
        })
        .await?;
    Ok(Json(RemoveDependencyResponse {
        data: api_dependencies(result.dependencies)?,
    }))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v1/tasks/:child_task_id/dependencies/:parent_task_id",
        delete(remove_dependency),
    )
}
#[cfg(test)]
mod tests {}

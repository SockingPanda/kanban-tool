use super::support::*;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use kanban_protocol::{ListDependenciesPath, ListDependenciesResponse};

pub(crate) async fn list_dependencies(
    State(state): State<AppState>,
    Path(ListDependenciesPath { task_id }): Path<ListDependenciesPath>,
) -> Result<Json<ListDependenciesResponse>, ApiError> {
    let dependencies = state.application().list_dependencies(&task_id).await?;
    Ok(Json(ListDependenciesResponse {
        data: api_dependencies(dependencies)?,
    }))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        crate::http::operations::registered_path(
            kanban_protocol::HttpMethod::Get,
            "/api/v1/tasks/:task_id/dependencies",
        ),
        get(list_dependencies),
    )
}
#[cfg(test)]
mod tests {}

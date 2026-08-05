use super::support::*;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, Query, State, rejection::QueryRejection},
    routing::get,
};
use kanban_contract::{GetTaskPath, GetTaskQuery, GetTaskResponse};
use kanban_core::KanbanError;

pub(crate) async fn get_task(
    State(state): State<AppState>,
    Path(GetTaskPath { task_id }): Path<GetTaskPath>,
    query: Result<Query<GetTaskQuery>, QueryRejection>,
) -> Result<Json<GetTaskResponse>, ApiError> {
    let Query(query) =
        query.map_err(|error| KanbanError::InvalidInput(format!("invalid query: {error}")))?;
    if let Some(include) = query.include.as_deref() {
        if include == "ontology" {
            return Err(KanbanError::FeatureNotAvailable(
                "task ontology details are not available on the single-host path".to_owned(),
            )
            .into());
        }
        return Err(
            KanbanError::InvalidInput(format!("unsupported task include: {include}")).into(),
        );
    }
    let task = state.application().get_task(&task_id).await?;
    Ok(Json(GetTaskResponse::new(api_task(task)?, None)))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v1/tasks/:task_id", get(get_task))
}
#[cfg(test)]
mod tests {}

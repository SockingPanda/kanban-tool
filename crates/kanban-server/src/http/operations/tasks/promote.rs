use super::super::support::request_actor;
use super::support::*;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::HeaderMap,
    routing::post,
};
use kanban_service::PromoteTaskCommand;
use kanban_core::KanbanError;
use kanban_protocol::{PromoteTaskPath, PromoteTaskRequest, PromoteTaskResponse};

pub(crate) async fn promote_task(
    State(state): State<AppState>,
    Path(PromoteTaskPath { task_id }): Path<PromoteTaskPath>,
    headers: HeaderMap,
    body: Result<Json<PromoteTaskRequest>, JsonRejection>,
) -> Result<Json<PromoteTaskResponse>, ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("invalid JSON body: {error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let task = state
        .application()
        .promote_task(PromoteTaskCommand { task_id, actor })
        .await?;
    Ok(Json(PromoteTaskResponse::new(api_task(task)?)))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v1/tasks/:task_id/transitions/promote",
        post(promote_task),
    )
}

#[cfg(test)]
mod tests {}

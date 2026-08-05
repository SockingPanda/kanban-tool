use super::super::support::request_actor;
use super::support::*;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::HeaderMap,
    routing::post,
};
use kanban_application::SpecifyTaskCommand;
use kanban_contract::{SpecifyTaskPath, SpecifyTaskRequest, SpecifyTaskResponse};
use kanban_core::KanbanError;

pub(crate) async fn specify_task(
    State(state): State<AppState>,
    Path(SpecifyTaskPath { task_id }): Path<SpecifyTaskPath>,
    headers: HeaderMap,
    body: Result<Json<SpecifyTaskRequest>, JsonRejection>,
) -> Result<Json<SpecifyTaskResponse>, ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("无效 JSON 请求体: {error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let task = state
        .application()
        .specify_task(SpecifyTaskCommand {
            task_id,
            actor,
            description: body.description,
            scheduled_at: body.scheduled_at,
        })
        .await?;
    Ok(Json(SpecifyTaskResponse::new(api_task(task)?)))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v1/tasks/:task_id/transitions/specify",
        post(specify_task),
    )
}

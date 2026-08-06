use super::super::support::request_actor;
use super::support::*;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::HeaderMap,
    routing::patch,
};
use kanban_protocol::{UpdateTaskPath, UpdateTaskRequest, UpdateTaskResponse};
use kanban_service::KanbanError;
use kanban_service::UpdateTaskCommand;

pub(crate) async fn update_task(
    State(state): State<AppState>,
    Path(UpdateTaskPath { task_id }): Path<UpdateTaskPath>,
    headers: HeaderMap,
    body: Result<Json<UpdateTaskRequest>, JsonRejection>,
) -> Result<Json<UpdateTaskResponse>, ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("无效 JSON 请求体: {error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let task = state
        .application()
        .update_task(UpdateTaskCommand {
            task_id,
            actor,
            expected_lock_version: body.expected_lock_version,
            title: body.title,
            description: body.description,
            assignee: body.assignee,
            priority: body.priority,
            scheduled_at: body.scheduled_at,
            due_at: body.due_at,
            max_retries: body.max_retries,
            metadata: body.metadata,
        })
        .await?;
    Ok(Json(UpdateTaskResponse::new(api_task(task)?)))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        crate::http::operations::registered_path(
            kanban_protocol::HttpMethod::Patch,
            "/api/v1/tasks/:task_id",
        ),
        patch(update_task),
    )
}

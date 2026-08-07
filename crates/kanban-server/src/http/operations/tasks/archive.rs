use super::super::support::request_actor;
use super::support::*;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::HeaderMap,
    routing::post,
};
use kanban_protocol::{ArchiveTaskPath, ArchiveTaskRequest, ArchiveTaskResponse};
use kanban_service::ArchiveTaskCommand;
use kanban_service::KanbanError;

pub(crate) async fn archive_task(
    State(state): State<AppState>,
    Path(ArchiveTaskPath { task_id }): Path<ArchiveTaskPath>,
    headers: HeaderMap,
    body: Result<Json<ArchiveTaskRequest>, JsonRejection>,
) -> Result<Json<ArchiveTaskResponse>, ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("无效 JSON 请求体: {error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let task = state
        .application()
        .archive_task(ArchiveTaskCommand {
            task_id,
            actor,
            force: body.force,
        })
        .await?;
    Ok(Json(ArchiveTaskResponse::new(api_task(task)?)))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        crate::http::operations::registered_path(
            kanban_protocol::HttpMethod::Post,
            "/api/v1/tasks/:task_id/transitions/archive",
        ),
        post(archive_task),
    )
}

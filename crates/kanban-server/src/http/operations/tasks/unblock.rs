use super::super::support::request_actor;
use super::support::*;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::HeaderMap,
    routing::post,
};
use kanban_service::UnblockTaskCommand;
use kanban_core::KanbanError;
use kanban_protocol::{UnblockTaskPath, UnblockTaskRequest, UnblockTaskResponse};

pub(crate) async fn unblock_task(
    State(state): State<AppState>,
    Path(UnblockTaskPath { task_id }): Path<UnblockTaskPath>,
    headers: HeaderMap,
    body: Result<Json<UnblockTaskRequest>, JsonRejection>,
) -> Result<Json<UnblockTaskResponse>, ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("无效 JSON 请求体: {error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let task = state
        .application()
        .unblock_task(UnblockTaskCommand { task_id, actor })
        .await?;
    Ok(Json(UnblockTaskResponse::new(api_task(task)?)))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v1/tasks/:task_id/transitions/unblock",
        post(unblock_task),
    )
}

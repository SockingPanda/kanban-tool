use super::super::support::request_actor;
use super::support::*;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::HeaderMap,
    routing::post,
};
use kanban_service::ReclaimTaskCommand;
use kanban_core::{KanbanError, TaskStatus};
use kanban_protocol::{
    ReclaimTargetStatus, ReclaimTaskPath, ReclaimTaskRequest, ReclaimTaskResponse,
};

pub(crate) async fn reclaim_task(
    State(state): State<AppState>,
    Path(ReclaimTaskPath { task_id }): Path<ReclaimTaskPath>,
    headers: HeaderMap,
    body: Result<Json<ReclaimTaskRequest>, JsonRejection>,
) -> Result<Json<ReclaimTaskResponse>, ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("无效 JSON 请求体: {error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let target_status = body.to_status.map(|target| match target {
        ReclaimTargetStatus::Ready => TaskStatus::Ready,
        ReclaimTargetStatus::Blocked => TaskStatus::Blocked,
    });
    let task = state
        .application()
        .reclaim_task(ReclaimTaskCommand {
            task_id,
            actor,
            force: body.force,
            target_status,
            reason: body.reason,
        })
        .await?;
    Ok(Json(ReclaimTaskResponse::new(api_task(task)?)))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v1/tasks/:task_id/transitions/reclaim",
        post(reclaim_task),
    )
}

use super::super::support::request_actor;
use super::support::*;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::HeaderMap,
    routing::post,
};
use kanban_application::ReopenTaskCommand;
use kanban_contract::{ReopenTaskPath, ReopenTaskRequest, ReopenTaskResponse};
use kanban_core::KanbanError;

pub(crate) async fn reopen_task(
    State(state): State<AppState>,
    Path(ReopenTaskPath { task_id }): Path<ReopenTaskPath>,
    headers: HeaderMap,
    body: Result<Json<ReopenTaskRequest>, JsonRejection>,
) -> Result<Json<ReopenTaskResponse>, ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("无效 JSON 请求体: {error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let task = state
        .application()
        .reopen_task(ReopenTaskCommand {
            task_id,
            actor,
            reason: body.reason,
        })
        .await?;
    Ok(Json(ReopenTaskResponse::new(api_task(task)?)))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v1/tasks/:task_id/transitions/reopen",
        post(reopen_task),
    )
}

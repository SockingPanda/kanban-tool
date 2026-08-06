use super::super::support::request_actor;
use super::support::*;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::HeaderMap,
    routing::post,
};
use kanban_protocol::{PromoteTaskPath, PromoteTaskRequest, PromoteTaskResponse};
use kanban_service::KanbanError;
use kanban_service::PromoteTaskCommand;

pub(crate) async fn promote_task(
    State(state): State<AppState>,
    Path(PromoteTaskPath { task_id }): Path<PromoteTaskPath>,
    headers: HeaderMap,
    body: Result<Json<PromoteTaskRequest>, JsonRejection>,
) -> Result<Json<PromoteTaskResponse>, ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("JSON 请求体无效：{error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let task = state
        .application()
        .promote_task(PromoteTaskCommand { task_id, actor })
        .await?;
    Ok(Json(PromoteTaskResponse::new(api_task(task)?)))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        crate::http::operations::registered_path(
            kanban_protocol::HttpMethod::Post,
            "/api/v1/tasks/:task_id/transitions/promote",
        ),
        post(promote_task),
    )
}

#[cfg(test)]
mod tests {}

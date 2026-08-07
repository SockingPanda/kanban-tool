use super::super::support::request_actor;
use super::support::*;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::HeaderMap,
    routing::post,
};
use kanban_protocol::{HeartbeatTaskPath, HeartbeatTaskRequest, HeartbeatTaskResponse};
use kanban_service::HeartbeatTaskCommand;
use kanban_service::KanbanError;

pub(crate) async fn heartbeat_task(
    State(state): State<AppState>,
    Path(HeartbeatTaskPath { task_id }): Path<HeartbeatTaskPath>,
    headers: HeaderMap,
    body: Result<Json<HeartbeatTaskRequest>, JsonRejection>,
) -> Result<Json<HeartbeatTaskResponse>, ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("JSON 请求体无效：{error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let task = state
        .application()
        .heartbeat_task(HeartbeatTaskCommand {
            task_id,
            actor,
            claim_token: body.claim_token,
            ttl_ms: body.ttl_ms,
            note: body.note,
        })
        .await?;
    Ok(Json(HeartbeatTaskResponse::new(api_task(task)?)))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        crate::http::operations::registered_path(
            kanban_protocol::HttpMethod::Post,
            "/api/v1/tasks/:task_id/transitions/heartbeat",
        ),
        post(heartbeat_task),
    )
}

#[cfg(test)]
mod tests {}

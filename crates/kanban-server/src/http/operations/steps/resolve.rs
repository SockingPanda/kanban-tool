use super::super::support::request_actor;
use super::support::*;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::HeaderMap,
    routing::post,
};
use kanban_application::{CompleteStepCommand, ReopenStepCommand, SkipStepCommand};
use kanban_contract::{
    CompleteStepPath, CompleteStepRequest, CompleteStepResponse, ReopenStepPath, ReopenStepRequest,
    ReopenStepResponse, SkipStepPath, SkipStepRequest, SkipStepResponse,
};
use kanban_core::KanbanError;

pub(crate) async fn complete_step(
    State(state): State<AppState>,
    Path(CompleteStepPath { task_id, step_id }): Path<CompleteStepPath>,
    headers: HeaderMap,
    body: Result<Json<CompleteStepRequest>, JsonRejection>,
) -> Result<Json<CompleteStepResponse>, ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("JSON 请求体无效：{error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let steps = state
        .application()
        .complete_step(CompleteStepCommand {
            task_id,
            step_id,
            note: body.note,
            actor,
        })
        .await?;
    Ok(Json(CompleteStepResponse {
        data: api_task_steps(steps)?,
    }))
}

pub(crate) async fn skip_step(
    State(state): State<AppState>,
    Path(SkipStepPath { task_id, step_id }): Path<SkipStepPath>,
    headers: HeaderMap,
    body: Result<Json<SkipStepRequest>, JsonRejection>,
) -> Result<Json<SkipStepResponse>, ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("JSON 请求体无效：{error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let steps = state
        .application()
        .skip_step(SkipStepCommand {
            task_id,
            step_id,
            reason: body.reason,
            actor,
        })
        .await?;
    Ok(Json(SkipStepResponse {
        data: api_task_steps(steps)?,
    }))
}

pub(crate) async fn reopen_step(
    State(state): State<AppState>,
    Path(ReopenStepPath { task_id, step_id }): Path<ReopenStepPath>,
    headers: HeaderMap,
    body: Result<Json<ReopenStepRequest>, JsonRejection>,
) -> Result<Json<ReopenStepResponse>, ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("JSON 请求体无效：{error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let steps = state
        .application()
        .reopen_step(ReopenStepCommand {
            task_id,
            step_id,
            reason: body.reason,
            actor,
        })
        .await?;
    Ok(Json(ReopenStepResponse {
        data: api_task_steps(steps)?,
    }))
}

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/tasks/:task_id/steps/:step_id/done",
            post(complete_step),
        )
        .route(
            "/api/v1/tasks/:task_id/steps/:step_id/skip",
            post(skip_step),
        )
        .route(
            "/api/v1/tasks/:task_id/steps/:step_id/reopen",
            post(reopen_step),
        )
}

use super::super::support::request_actor;
use super::support::*;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::HeaderMap,
    routing::post,
};
use kanban_protocol::{
    MarkExecutionPlanNotRequiredPath, MarkExecutionPlanNotRequiredRequest,
    MarkExecutionPlanNotRequiredResponse,
};
use kanban_service::KanbanError;
use kanban_service::MarkExecutionPlanNotRequiredCommand;

pub(crate) async fn mark_execution_plan_not_required(
    State(state): State<AppState>,
    Path(MarkExecutionPlanNotRequiredPath { task_id }): Path<MarkExecutionPlanNotRequiredPath>,
    headers: HeaderMap,
    body: Result<Json<MarkExecutionPlanNotRequiredRequest>, JsonRejection>,
) -> Result<Json<MarkExecutionPlanNotRequiredResponse>, ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("invalid JSON body: {error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let plan = state
        .application()
        .mark_execution_plan_not_required(MarkExecutionPlanNotRequiredCommand {
            task_id,
            reason: body.reason,
            actor,
        })
        .await?;
    Ok(Json(MarkExecutionPlanNotRequiredResponse {
        data: api_execution_plan(plan),
    }))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v1/tasks/:task_id/execution-plan/not-required",
        post(mark_execution_plan_not_required),
    )
}

#[cfg(test)]
mod tests {}

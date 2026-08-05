use super::super::support::request_actor;
use super::support::*;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::HeaderMap,
    routing::patch,
};
use kanban_service::UpdateStepCommand;
use kanban_service::KanbanError;
use kanban_protocol::{UpdateStepPath, UpdateStepRequest, UpdateStepResponse};

pub(crate) async fn update_step(
    State(state): State<AppState>,
    Path(UpdateStepPath { task_id, step_id }): Path<UpdateStepPath>,
    headers: HeaderMap,
    body: Result<Json<UpdateStepRequest>, JsonRejection>,
) -> Result<Json<UpdateStepResponse>, ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("invalid JSON body: {error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let steps = state
        .application()
        .update_step(UpdateStepCommand {
            task_id,
            step_id,
            title: body.title,
            body: body.body,
            linked_task_id: body.linked_task_ref,
            unlink_task: body.unlink_task,
            position: body.position,
            required: body.required,
            actor,
        })
        .await?;
    Ok(Json(UpdateStepResponse {
        data: api_task_steps(steps)?,
    }))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v1/tasks/:task_id/steps/:step_id", patch(update_step))
}
#[cfg(test)]
mod tests {}

use super::super::support::request_actor;
use super::support::*;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::delete,
};
use kanban_protocol::{RemoveStepPath, RemoveStepResponse};
use kanban_service::operations::RemoveStepCommand;

pub(crate) async fn remove_step(
    State(state): State<AppState>,
    Path(RemoveStepPath { task_id, step_id }): Path<RemoveStepPath>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<RemoveStepResponse>), ApiError> {
    let actor = request_actor(None, &headers, state.default_actor())?;
    let steps = state
        .application()
        .remove_step(RemoveStepCommand {
            task_id,
            step_id,
            actor,
        })
        .await?;
    Ok((
        StatusCode::OK,
        Json(RemoveStepResponse {
            data: api_task_steps(steps)?,
        }),
    ))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        crate::http::operations::registered_path(
            kanban_protocol::HttpMethod::Delete,
            "/api/v1/tasks/:task_id/steps/:step_id",
        ),
        delete(remove_step),
    )
}

use super::super::support::request_actor;
use super::support::*;
use crate::application::support::CallContext;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{RemoveStepPath, RemoveStepResponse};
use kanban_service::operations::RemoveStepCommand;
pub(crate) async fn remove_step(
    state: AppState,
    RemoveStepPath { task_id, step_id }: RemoveStepPath,
    headers: CallContext,
) -> Result<RemoveStepResponse, ApiError> {
    let actor = request_actor(None, &headers, state.default_actor())?;
    let steps = state
        .application()
        .remove_step(RemoveStepCommand {
            task_id,
            step_id,
            actor,
        })
        .await?;
    Ok(RemoveStepResponse {
        data: api_task_steps(steps)?,
    })
}

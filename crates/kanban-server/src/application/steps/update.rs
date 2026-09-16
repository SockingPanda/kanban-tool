use super::super::support::request_actor;
use super::support::*;
use crate::application::support::CallContext;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{UpdateStepPath, UpdateStepRequest, UpdateStepResponse};
use kanban_service::UpdateStepCommand;
pub(crate) async fn update_step(
    state: AppState,
    UpdateStepPath { task_id, step_id }: UpdateStepPath,
    headers: CallContext,
    body: UpdateStepRequest,
) -> Result<UpdateStepResponse, ApiError> {
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
    Ok(UpdateStepResponse {
        data: api_task_steps(steps)?,
    })
}

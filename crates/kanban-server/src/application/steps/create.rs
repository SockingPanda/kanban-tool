use super::super::support::request_actor;
use super::support::*;
use crate::application::support::CallContext;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{CreateStepPath, CreateStepRequest, CreateStepResponse};
use kanban_service::CreateStepCommand;
pub(crate) async fn create_step(
    state: AppState,
    CreateStepPath { task_id }: CreateStepPath,
    headers: CallContext,
    body: CreateStepRequest,
) -> Result<CreateStepResponse, ApiError> {
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let steps = state
        .application()
        .create_step(CreateStepCommand {
            task_id,
            idempotency_key: body.idempotency_key,
            title: body.title,
            body: body.body,
            linked_task_id: body.linked_task_ref,
            position: body.position,
            required: body.required,
            actor,
        })
        .await?;
    Ok(CreateStepResponse {
        data: api_task_steps(steps)?,
    })
}

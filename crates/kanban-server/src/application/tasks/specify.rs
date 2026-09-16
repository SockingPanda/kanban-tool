use super::super::support::request_actor;
use super::support::*;
use crate::application::support::CallContext;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{SpecifyTaskPath, SpecifyTaskRequest, SpecifyTaskResponse};
use kanban_service::SpecifyTaskCommand;
pub(crate) async fn specify_task(
    state: AppState,
    SpecifyTaskPath { task_id }: SpecifyTaskPath,
    headers: CallContext,
    body: SpecifyTaskRequest,
) -> Result<SpecifyTaskResponse, ApiError> {
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let task = state
        .application()
        .specify_task(SpecifyTaskCommand {
            task_id,
            actor,
            description: body.description,
            scheduled_at: body.scheduled_at,
        })
        .await?;
    Ok(SpecifyTaskResponse::new(api_task(task)?))
}

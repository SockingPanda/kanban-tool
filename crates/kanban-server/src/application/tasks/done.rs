use super::super::support::request_actor;
use super::support::*;
use crate::application::support::CallContext;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{CompleteTaskPath, CompleteTaskRequest, CompleteTaskResponse};
use kanban_service::CompleteTaskCommand;
pub(crate) async fn complete_task(
    state: AppState,
    CompleteTaskPath { task_id }: CompleteTaskPath,
    headers: CallContext,
    body: CompleteTaskRequest,
) -> Result<CompleteTaskResponse, ApiError> {
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let task = state
        .application()
        .complete_task(CompleteTaskCommand {
            task_id,
            actor,
            claim_token: body.claim_token,
            force: body.force,
            summary: body.summary,
            result: body.result,
        })
        .await?;
    Ok(CompleteTaskResponse::new(api_task(task)?))
}

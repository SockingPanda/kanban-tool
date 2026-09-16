use super::super::support::request_actor;
use super::support::*;
use crate::application::support::CallContext;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{
    CompleteStepPath, CompleteStepRequest, CompleteStepResponse, ReopenStepPath, ReopenStepRequest,
    ReopenStepResponse, SkipStepPath, SkipStepRequest, SkipStepResponse,
};
use kanban_service::operations::{CompleteStepCommand, ReopenStepCommand, SkipStepCommand};
pub(crate) async fn complete_step(
    state: AppState,
    CompleteStepPath { task_id, step_id }: CompleteStepPath,
    headers: CallContext,
    body: CompleteStepRequest,
) -> Result<CompleteStepResponse, ApiError> {
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
    Ok(CompleteStepResponse {
        data: api_task_steps(steps)?,
    })
}
pub(crate) async fn skip_step(
    state: AppState,
    SkipStepPath { task_id, step_id }: SkipStepPath,
    headers: CallContext,
    body: SkipStepRequest,
) -> Result<SkipStepResponse, ApiError> {
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
    Ok(SkipStepResponse {
        data: api_task_steps(steps)?,
    })
}
pub(crate) async fn reopen_step(
    state: AppState,
    ReopenStepPath { task_id, step_id }: ReopenStepPath,
    headers: CallContext,
    body: ReopenStepRequest,
) -> Result<ReopenStepResponse, ApiError> {
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
    Ok(ReopenStepResponse {
        data: api_task_steps(steps)?,
    })
}

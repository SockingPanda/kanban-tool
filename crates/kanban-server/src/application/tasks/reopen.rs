use super::super::support::request_actor;
use super::support::*;
use crate::application::support::CallContext;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{ReopenTaskPath, ReopenTaskRequest, ReopenTaskResponse};
use kanban_service::ReopenTaskCommand;
pub(crate) async fn reopen_task(
    state: AppState,
    ReopenTaskPath { task_id }: ReopenTaskPath,
    headers: CallContext,
    body: ReopenTaskRequest,
) -> Result<ReopenTaskResponse, ApiError> {
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let task = state
        .application()
        .reopen_task(ReopenTaskCommand {
            task_id,
            actor,
            reason: body.reason,
        })
        .await?;
    Ok(ReopenTaskResponse::new(api_task(task)?))
}

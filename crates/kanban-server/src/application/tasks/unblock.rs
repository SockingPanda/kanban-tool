use super::super::support::request_actor;
use super::support::*;
use crate::application::support::CallContext;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{UnblockTaskPath, UnblockTaskRequest, UnblockTaskResponse};
use kanban_service::UnblockTaskCommand;
pub(crate) async fn unblock_task(
    state: AppState,
    UnblockTaskPath { task_id }: UnblockTaskPath,
    headers: CallContext,
    body: UnblockTaskRequest,
) -> Result<UnblockTaskResponse, ApiError> {
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let task = state
        .application()
        .unblock_task(UnblockTaskCommand { task_id, actor })
        .await?;
    Ok(UnblockTaskResponse::new(api_task(task)?))
}

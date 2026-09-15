use super::super::support::request_actor;
use super::support::*;
use crate::application::support::CallContext;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{BlockTaskPath, BlockTaskRequest, BlockTaskResponse};
use kanban_service::BlockTaskCommand;
pub(crate) async fn block_task(
    state: AppState,
    BlockTaskPath { task_id }: BlockTaskPath,
    headers: CallContext,
    body: BlockTaskRequest,
) -> Result<BlockTaskResponse, ApiError> {
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let task = state
        .application()
        .block_task(BlockTaskCommand {
            task_id,
            actor,
            reason: body.reason,
            claim_token: body.claim_token,
            force: body.force,
        })
        .await?;
    Ok(BlockTaskResponse::new(api_task(task)?))
}

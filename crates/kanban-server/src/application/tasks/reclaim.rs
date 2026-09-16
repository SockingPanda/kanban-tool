use super::super::support::request_actor;
use super::support::*;
use crate::application::support::CallContext;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{
    ReclaimTargetStatus, ReclaimTaskPath, ReclaimTaskRequest, ReclaimTaskResponse,
};
use kanban_service::ReclaimTaskCommand;
use kanban_service::TaskStatus;
pub(crate) async fn reclaim_task(
    state: AppState,
    ReclaimTaskPath { task_id }: ReclaimTaskPath,
    headers: CallContext,
    body: ReclaimTaskRequest,
) -> Result<ReclaimTaskResponse, ApiError> {
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let target_status = body.to_status.map(|target| match target {
        ReclaimTargetStatus::Ready => TaskStatus::Ready,
        ReclaimTargetStatus::Blocked => TaskStatus::Blocked,
    });
    let task = state
        .application()
        .reclaim_task(ReclaimTaskCommand {
            task_id,
            actor,
            force: body.force,
            target_status,
            reason: body.reason,
        })
        .await?;
    Ok(ReclaimTaskResponse::new(api_task(task)?))
}

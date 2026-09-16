use super::super::support::request_actor;
use super::support::*;
use crate::application::support::CallContext;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{ArchiveTaskPath, ArchiveTaskRequest, ArchiveTaskResponse};
use kanban_service::ArchiveTaskCommand;
pub(crate) async fn archive_task(
    state: AppState,
    ArchiveTaskPath { task_id }: ArchiveTaskPath,
    headers: CallContext,
    body: ArchiveTaskRequest,
) -> Result<ArchiveTaskResponse, ApiError> {
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let task = state
        .application()
        .archive_task(ArchiveTaskCommand {
            task_id,
            actor,
            force: body.force,
        })
        .await?;
    Ok(ArchiveTaskResponse::new(api_task(task)?))
}

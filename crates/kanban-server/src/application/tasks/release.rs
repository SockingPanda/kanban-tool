use super::super::support::request_actor;
use super::support::*;
use crate::application::support::CallContext;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{ReleaseTaskPath, ReleaseTaskRequest, ReleaseTaskResponse};
use kanban_service::ReleaseTaskCommand;
pub(crate) async fn release_task(
    state: AppState,
    ReleaseTaskPath { task_id }: ReleaseTaskPath,
    headers: CallContext,
    body: ReleaseTaskRequest,
) -> Result<ReleaseTaskResponse, ApiError> {
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let task = state
        .application()
        .release_task(ReleaseTaskCommand {
            task_id,
            actor,
            claim_token: body.claim_token,
        })
        .await?;
    Ok(ReleaseTaskResponse::new(api_task(task)?))
}

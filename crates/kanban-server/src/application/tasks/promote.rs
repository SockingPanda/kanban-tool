use super::super::support::request_actor;
use super::support::*;
use crate::application::support::CallContext;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{PromoteTaskPath, PromoteTaskRequest, PromoteTaskResponse};
use kanban_service::PromoteTaskCommand;
pub(crate) async fn promote_task(
    state: AppState,
    PromoteTaskPath { task_id }: PromoteTaskPath,
    headers: CallContext,
    body: PromoteTaskRequest,
) -> Result<PromoteTaskResponse, ApiError> {
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let task = state
        .application()
        .promote_task(PromoteTaskCommand { task_id, actor })
        .await?;
    Ok(PromoteTaskResponse::new(api_task(task)?))
}

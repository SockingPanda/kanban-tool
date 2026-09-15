use super::super::support::request_actor;
use super::support::*;
use crate::application::support::CallContext;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{RemoveDependencyPath, RemoveDependencyResponse};
use kanban_service::RemoveDependencyCommand;
pub(crate) async fn remove_dependency(
    state: AppState,
    RemoveDependencyPath {
        child_task_id,
        parent_task_id,
    }: RemoveDependencyPath,
    headers: CallContext,
) -> Result<RemoveDependencyResponse, ApiError> {
    let actor = request_actor(None, &headers, state.default_actor())?;
    let result = state
        .application()
        .remove_dependency(RemoveDependencyCommand {
            child_task_id,
            parent_task_id,
            actor,
        })
        .await?;
    Ok(RemoveDependencyResponse {
        data: api_dependencies(result.dependencies)?,
    })
}

use super::super::support::request_actor;
use super::support::*;
use crate::application::support::CallContext;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{AddDependencyPath, AddDependencyRequest, AddDependencyResponse};
use kanban_service::AddDependencyCommand;
pub(crate) async fn add_dependency(
    state: AppState,
    AddDependencyPath { task_id }: AddDependencyPath,
    headers: CallContext,
    body: AddDependencyRequest,
) -> Result<AddDependencyResponse, ApiError> {
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let result = state
        .application()
        .add_dependency(AddDependencyCommand {
            child_task_id: task_id,
            parent_task_id: body.parent_task_id,
            actor,
        })
        .await?;
    Ok(AddDependencyResponse {
        data: api_dependencies(result.dependencies)?,
    })
}

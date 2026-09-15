use super::super::support::request_actor;
use super::support::*;
use crate::application::support::CallContext;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{UpdateTaskPath, UpdateTaskRequest, UpdateTaskResponse};
use kanban_service::UpdateTaskCommand;
pub(crate) async fn update_task(
    state: AppState,
    UpdateTaskPath { task_id }: UpdateTaskPath,
    headers: CallContext,
    body: UpdateTaskRequest,
) -> Result<UpdateTaskResponse, ApiError> {
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let task = state
        .application()
        .update_task(UpdateTaskCommand {
            task_id,
            actor,
            expected_lock_version: body.expected_lock_version,
            title: body.title,
            description: body.description,
            assignee: body.assignee,
            priority: body.priority,
            scheduled_at: body.scheduled_at,
            due_at: body.due_at,
            max_retries: body.max_retries,
            metadata: body.metadata,
        })
        .await?;
    Ok(UpdateTaskResponse::new(api_task(task)?))
}

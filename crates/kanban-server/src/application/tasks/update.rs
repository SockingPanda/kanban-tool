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
    let request_fingerprint = Some(request_fingerprint(&body, &actor)?);
    let task = state
        .application()
        .update_task(UpdateTaskCommand {
            planning: kanban_service::TaskPlanningInput {
                module_ids: body.module_ids,
                cycle_id: body.cycle_id,
                expected_object_version: body.expected_object_version,
                expected_versions: crate::application::planning::versions(body.expected_versions),
                expected_catalog_version: body.expected_catalog_version,
            },
            request_fingerprint,
            request_id: body.request_id,
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

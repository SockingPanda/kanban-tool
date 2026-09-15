use super::super::support::request_actor;
use super::support::*;
use crate::application::support::CallContext;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{ApiCreateTaskStatus, CreateTaskPath, CreateTaskRequest, CreateTaskResponse};
use kanban_service::CreateTaskCommand;
use kanban_service::{TaskStatus, new_task_id};
fn create_status(status: ApiCreateTaskStatus) -> TaskStatus {
    match status {
        ApiCreateTaskStatus::Triage => TaskStatus::Triage,
        ApiCreateTaskStatus::Todo => TaskStatus::Todo,
        ApiCreateTaskStatus::Scheduled => TaskStatus::Scheduled,
        ApiCreateTaskStatus::Ready => TaskStatus::Ready,
    }
}
pub(crate) async fn create_task(
    state: AppState,
    CreateTaskPath { board }: CreateTaskPath,
    headers: CallContext,
    body: CreateTaskRequest,
) -> Result<CreateTaskResponse, ApiError> {
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let task = state
        .application()
        .create_task(CreateTaskCommand {
            task_id: body.task_id.unwrap_or_else(new_task_id),
            board,
            idempotency_key: body.idempotency_key,
            title: body.title,
            description: body.description,
            requested_status: body.status.map(create_status),
            assignee: body.assignee,
            priority: body.priority,
            scheduled_at: body.scheduled_at,
            due_at: body.due_at,
            max_retries: body.max_retries,
            metadata: body.metadata.unwrap_or_default(),
            labels: body.labels,
            depends_on: body.depends_on,
            actor,
        })
        .await?;
    Ok(CreateTaskResponse {
        data: api_task(task)?,
    })
}

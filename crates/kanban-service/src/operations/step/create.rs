use kanban_core::{
    Clock, KanbanError, ReadinessFacts, Result, TaskStatus, new_event_id, new_typed_id,
    recompute_ready_status,
};

use crate::{ExecutionPlanState, KanbanService, TaskStepsRecord};

#[derive(Debug, Clone, PartialEq)]
pub struct CreateStepCommand {
    pub task_id: String,
    pub idempotency_key: Option<String>,
    pub title: String,
    pub body: Option<String>,
    pub linked_task_id: Option<String>,
    pub position: Option<i64>,
    pub required: bool,
    pub actor: String,
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn create_step(&self, command: CreateStepCommand) -> Result<TaskStepsRecord> {
        let task_id = command.task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "task_id must be a global t_... id".to_owned(),
            ));
        }
        let actor = command.actor.trim();
        if actor.is_empty() {
            return Err(KanbanError::InvalidInput("actor is required".to_owned()));
        }
        let title = command.title.trim();
        if title.is_empty() {
            return Err(KanbanError::InvalidInput(
                "step title is required".to_owned(),
            ));
        }
        if command.position.is_some_and(|position| position < 0) {
            return Err(KanbanError::InvalidInput(
                "step position must be non-negative".to_owned(),
            ));
        }
        if command
            .idempotency_key
            .as_deref()
            .is_some_and(|key| key.trim().is_empty())
        {
            return Err(KanbanError::InvalidInput(
                "idempotency_key must not be empty".to_owned(),
            ));
        }
        let _mutation = self.mutation_gate.lock().await;
        let parent = self.get_task(task_id).await?;
        if parent.archived_at.is_some() || parent.status == TaskStatus::Archived {
            return Err(KanbanError::InvalidTransition(
                "archived parent task cannot receive steps".to_owned(),
            ));
        }
        let linked_task_id = command
            .linked_task_id
            .map(|linked_task_id| {
                let linked_task_id = linked_task_id.trim().to_owned();
                if !linked_task_id.starts_with("t_") || linked_task_id.len() <= 2 {
                    return Err(KanbanError::InvalidInput(
                        "linked_task_id must be a global t_... id".to_owned(),
                    ));
                }
                Ok(linked_task_id)
            })
            .transpose()?;
        if let Some(linked_task_id) = linked_task_id.as_deref() {
            let linked = self.get_task(linked_task_id).await?;
            if linked.board_id != parent.board_id {
                return Err(KanbanError::InvalidInput(
                    "linked task must belong to the parent board".to_owned(),
                ));
            }
            if linked.id == parent.id {
                return Err(KanbanError::InvalidInput(
                    "step cannot link to its parent task".to_owned(),
                ));
            }
            if linked.archived_at.is_some() || linked.status == TaskStatus::Archived {
                return Err(KanbanError::InvalidInput(
                    "archived linked task is not allowed".to_owned(),
                ));
            }
        }
        let now = self.clock.now_ms();
        let target_status = if matches!(
            parent.status,
            TaskStatus::Triage | TaskStatus::Todo | TaskStatus::Scheduled | TaskStatus::Ready
        ) {
            recompute_ready_status(
                ReadinessFacts {
                    title: &parent.title,
                    description: parent.description.as_deref(),
                    scheduled_at: parent.scheduled_at,
                    dependencies_done: !parent.dependency_blocked,
                },
                now,
            )
        } else {
            parent.status
        };
        self.application
            .store
            .store
            .create_step(
                task_id,
                crate::store_operations::CreateStepInput {
                    id: new_typed_id("step"),
                    idempotency_key: command.idempotency_key.map(|key| key.trim().to_owned()),
                    title: title.to_owned(),
                    body: command.body.map(|body| body.trim().to_owned()),
                    linked_task_id,
                    position: command.position,
                    required: command.required,
                    created_by: actor.to_owned(),
                    event_id: new_event_id(),
                    plan_event_id: new_event_id(),
                    recompute_event_id: new_event_id(),
                    created_at: now,
                    expected_lock_version: parent.lock_version,
                    expected_plan_state: execution_plan_state_as_str(parent.execution_plan_state)
                        .to_owned(),
                    target_status: target_status.as_str().to_owned(),
                },
            )
            .await
            .map_err(crate::adapter::store_error)?;
        self.list_steps(task_id).await
    }
}

fn execution_plan_state_as_str(state: ExecutionPlanState) -> &'static str {
    match state {
        ExecutionPlanState::Unplanned => "unplanned",
        ExecutionPlanState::Planned => "planned",
        ExecutionPlanState::NotRequired => "not_required",
    }
}

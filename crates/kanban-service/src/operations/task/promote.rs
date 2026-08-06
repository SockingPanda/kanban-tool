use kanban_core::{
    Clock, KanbanError, ReadinessFacts, Result, TaskStatus, can_promote_from, new_event_id,
    recompute_ready_status,
};

use crate::{KanbanService, TaskRecord};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromoteTaskCommand {
    pub task_id: String,
    pub actor: String,
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn promote_task(&self, command: PromoteTaskCommand) -> Result<TaskRecord> {
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
        let _mutation = self.mutation_gate.lock().await;
        let task = self.get_task(task_id).await?;
        if !can_promote_from(task.status) {
            return Err(KanbanError::InvalidTransition(format!(
                "cannot promote from {}",
                task.status.as_str()
            )));
        }
        let now = self.clock.now_ms();
        let target = recompute_ready_status(
            ReadinessFacts {
                title: &task.title,
                description: task.description.as_deref(),
                scheduled_at: task.scheduled_at,
                dependencies_done: !task.dependency_blocked,
            },
            now,
        );
        if target != TaskStatus::Ready {
            return Err(KanbanError::InvalidTransition(match target {
                TaskStatus::Todo => "dependency blocked".to_owned(),
                TaskStatus::Scheduled => "scheduled_at is in the future".to_owned(),
                TaskStatus::Triage => "task spec is incomplete".to_owned(),
                _ => format!("cannot promote to {}", target.as_str()),
            }));
        }
        if task.execution_plan_state == crate::ExecutionPlanState::Unplanned {
            return Err(KanbanError::ExecutionPlanRequired(
                "add steps or mark execution plan not_required before promoting task".to_owned(),
            ));
        }
        self.application
            .store
            .store
            .promote_task(
                task_id,
                crate::store_operations::PromoteTaskInput {
                    expected_lock_version: task.lock_version,
                    actor: actor.to_owned(),
                    event_id: new_event_id(),
                    updated_at: now,
                },
            )
            .await
            .map_err(crate::adapter::store_error)
            .and_then(super::application_task)
    }
}

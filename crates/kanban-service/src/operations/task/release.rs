use kanban_core::{
    Clock, KanbanError, ReadinessFacts, Result, TaskStatus, new_event_id, recompute_ready_status,
    running_claim_is_present,
};

use crate::{KanbanService, TaskRecord};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseTaskCommand {
    pub task_id: String,
    pub actor: String,
    pub claim_token: String,
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn release_task(&self, command: ReleaseTaskCommand) -> Result<TaskRecord> {
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
        if command.claim_token.trim().is_empty() {
            return Err(KanbanError::InvalidInput(
                "claim_token is required".to_owned(),
            ));
        }
        let _mutation = self.mutation_gate.lock().await;
        let task = self.get_task(task_id).await?;
        if !running_claim_is_present(
            task.status,
            task.has_claim_token,
            task.current_run_id.is_some(),
        ) {
            return Err(KanbanError::InvalidTransition(
                "release requires an active running claim".to_owned(),
            ));
        }
        if task.claim_owner.as_deref() != Some(actor) {
            return Err(KanbanError::InvalidTransition(
                "claim owner mismatch".to_owned(),
            ));
        }
        if task.execution_plan_state == crate::ExecutionPlanState::Unplanned {
            return Err(KanbanError::ExecutionPlanRequired(
                "add steps or mark execution plan not_required before releasing task".to_owned(),
            ));
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
                _ => format!("cannot release to {}", target.as_str()),
            }));
        }
        self.application
            .store
            .store
            .release_task(
                task_id,
                crate::store_operations::ReleaseTaskInput {
                    expected_lock_version: task.lock_version,
                    actor: actor.to_owned(),
                    claim_token: command.claim_token,
                    event_id: new_event_id(),
                    now,
                },
            )
            .await
            .map_err(crate::adapter::store_error)
            .and_then(super::application_task)
    }
}

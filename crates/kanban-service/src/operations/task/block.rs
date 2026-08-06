use kanban_core::{Clock, KanbanError, Result, TaskStatus, new_event_id, running_claim_is_present};

use crate::{KanbanService, TaskRecord};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockTaskCommand {
    pub task_id: String,
    pub actor: String,
    pub reason: String,
    pub claim_token: Option<String>,
    pub force: bool,
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn block_task(&self, command: BlockTaskCommand) -> Result<TaskRecord> {
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
        if command.reason.trim().is_empty() {
            return Err(KanbanError::InvalidInput(
                "block reason is required".to_owned(),
            ));
        }

        let _mutation = self.mutation_gate.lock().await;
        let task = self.get_task(task_id).await?;
        if !matches!(
            task.status,
            TaskStatus::Triage
                | TaskStatus::Todo
                | TaskStatus::Scheduled
                | TaskStatus::Ready
                | TaskStatus::Running
                | TaskStatus::Review
        ) {
            return Err(KanbanError::InvalidTransition(
                "cannot block task".to_owned(),
            ));
        }
        if task.status == TaskStatus::Running {
            if !running_claim_is_present(
                task.status,
                task.has_claim_token,
                task.current_run_id.is_some(),
            ) {
                return Err(KanbanError::InvalidTransition(
                    "block requires an active running claim".to_owned(),
                ));
            }
            if !command.force && task.claim_owner.as_deref() != Some(actor) {
                return Err(KanbanError::InvalidTransition(
                    "claim owner mismatch".to_owned(),
                ));
            }
        }

        self.application
            .store
            .store
            .block_task(
                task_id,
                crate::store_operations::BlockTaskInput {
                    expected_lock_version: task.lock_version,
                    actor: actor.to_owned(),
                    reason: command.reason,
                    claim_token: command.claim_token,
                    force: command.force,
                    event_id: new_event_id(),
                    now: self.clock.now_ms(),
                },
            )
            .await
            .map_err(crate::adapter::store_error)
            .and_then(super::application_task)
    }
}

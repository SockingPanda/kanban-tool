use kanban_core::{Clock, KanbanError, Result, TaskStatus, new_event_id, running_claim_is_present};

use crate::{KanbanService, TaskRecord};

#[derive(Debug, Clone, PartialEq)]
pub struct CompleteTaskCommand {
    pub task_id: String,
    pub actor: String,
    pub claim_token: Option<String>,
    pub force: bool,
    pub summary: Option<String>,
    pub result: Option<serde_json::Value>,
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn complete_task(&self, command: CompleteTaskCommand) -> Result<TaskRecord> {
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
        let result_json = command
            .result
            .map(|result| serde_json::to_string(&result))
            .transpose()
            .map_err(|error| KanbanError::InvalidInput(format!("invalid result: {error}")))?;

        let _mutation = self.mutation_gate.lock().await;
        let task = self.get_task(task_id).await?;
        if !matches!(task.status, TaskStatus::Running | TaskStatus::Review) {
            return Err(KanbanError::InvalidTransition(
                "complete requires running or review".to_owned(),
            ));
        }
        if task.status == TaskStatus::Running {
            if !running_claim_is_present(
                task.status,
                task.has_claim_token,
                task.current_run_id.is_some(),
            ) {
                return Err(KanbanError::InvalidTransition(
                    "complete requires an active running claim".to_owned(),
                ));
            }
            if !command.force && task.claim_owner.as_deref() != Some(actor) {
                return Err(KanbanError::InvalidTransition(
                    "claim owner mismatch".to_owned(),
                ));
            }
        }
        if task.completed_required_step_count != task.required_step_count {
            return Err(KanbanError::StepsIncomplete(format!(
                "{} required step(s) remain incomplete",
                task.required_step_count
                    .saturating_sub(task.completed_required_step_count)
            )));
        }

        self.application
            .store
            .store
            .complete_task(
                task_id,
                crate::store_operations::CompleteTaskInput {
                    expected_lock_version: task.lock_version,
                    actor: actor.to_owned(),
                    claim_token: command.claim_token,
                    force: command.force,
                    summary: command.summary,
                    result_json,
                    event_id: new_event_id(),
                    now: self.clock.now_ms(),
                },
            )
            .await
            .map_err(crate::adapter::store_error)
            .and_then(super::application_task)
    }
}

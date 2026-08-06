use kanban_core::{Clock, KanbanError, Result, new_event_id};

use crate::{ExecutionPlanRecord, KanbanService};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkExecutionPlanNotRequiredCommand {
    pub task_id: String,
    pub reason: String,
    pub actor: String,
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn mark_execution_plan_not_required(
        &self,
        command: MarkExecutionPlanNotRequiredCommand,
    ) -> Result<ExecutionPlanRecord> {
        let task_id = command.task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "task_id must be a global t_... id".to_owned(),
            ));
        }
        let reason = command.reason.trim();
        if reason.is_empty() {
            return Err(KanbanError::InvalidInput(
                "execution plan not_required reason is required".to_owned(),
            ));
        }
        let actor = command.actor.trim();
        if actor.is_empty() {
            return Err(KanbanError::InvalidInput("actor is required".to_owned()));
        }
        let _mutation = self.mutation_gate.lock().await;
        self.application
            .store
            .store
            .mark_execution_plan_not_required(
                task_id,
                crate::store_operations::MarkExecutionPlanNotRequiredInput {
                    reason: reason.to_owned(),
                    actor: actor.to_owned(),
                    event_id: new_event_id(),
                    updated_at: self.clock.now_ms(),
                },
            )
            .await
            .map_err(crate::adapter::store_error)
            .and_then(crate::adapter::application_execution_plan)
    }
}

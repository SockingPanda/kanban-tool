use kanban_core::{Clock, KanbanError, Result, new_event_id, running_claim_is_present};

use crate::{KanbanService, TaskRecord};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatTaskCommand {
    pub task_id: String,
    pub actor: String,
    pub claim_token: String,
    pub ttl_ms: i64,
    pub note: Option<String>,
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn heartbeat_task(&self, command: HeartbeatTaskCommand) -> Result<TaskRecord> {
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
        if command.ttl_ms <= 0 {
            return Err(KanbanError::InvalidInput(
                "ttl_ms must be positive".to_owned(),
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
                "heartbeat requires an active running claim".to_owned(),
            ));
        }
        if task.claim_owner.as_deref() != Some(actor) {
            return Err(KanbanError::InvalidTransition(
                "claim owner mismatch".to_owned(),
            ));
        }
        let now = self.clock.now_ms();
        let claim_expires_at = now.checked_add(command.ttl_ms).ok_or_else(|| {
            KanbanError::InvalidInput("ttl_ms produces an invalid claim expiry".to_owned())
        })?;
        self.store
            .heartbeat_task(
                task_id,
                crate::store_operations::HeartbeatTaskInput {
                    expected_lock_version: task.lock_version,
                    actor: actor.to_owned(),
                    claim_token: command.claim_token,
                    event_id: new_event_id(),
                    note: command.note,
                    now,
                    claim_expires_at,
                },
            )
            .await
            .map_err(crate::error::store_error)
            .and_then(super::application_task)
    }
}

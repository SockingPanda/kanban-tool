use kanban_core::{Clock, KanbanError, Result, TaskStatus, new_event_id};

use crate::{KanbanService, TaskRecord};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnblockTaskCommand {
    pub task_id: String,
    pub actor: String,
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn unblock_task(&self, command: UnblockTaskCommand) -> Result<TaskRecord> {
        let task_id = command.task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "task_id 必须是全局 t_... ID".to_owned(),
            ));
        }
        let actor = command.actor.trim();
        if actor.is_empty() {
            return Err(KanbanError::InvalidInput("actor 不能为空".to_owned()));
        }
        let _mutation = self.mutation_gate.lock().await;
        let task = self.get_task(task_id).await?;
        if task.status != TaskStatus::Blocked {
            return Err(KanbanError::InvalidTransition(
                "只能 unblock blocked 任务".to_owned(),
            ));
        }
        self.store
            .unblock_task(
                task_id,
                crate::store_operations::UnblockTaskInput {
                    expected_lock_version: task.lock_version,
                    actor: actor.to_owned(),
                    event_id: new_event_id(),
                    now: self.clock.now_ms(),
                },
            )
            .await
            .map_err(crate::error::store_error)
            .and_then(super::application_task)
    }
}

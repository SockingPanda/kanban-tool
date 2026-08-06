use kanban_core::{Clock, KanbanError, Result, TaskStatus, new_event_id};

use crate::{KanbanService, TaskRecord};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecifyTaskCommand {
    pub task_id: String,
    pub actor: String,
    pub description: Option<String>,
    pub scheduled_at: Option<i64>,
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn specify_task(&self, command: SpecifyTaskCommand) -> Result<TaskRecord> {
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
        if command
            .description
            .as_deref()
            .is_some_and(|description| description.trim().is_empty())
        {
            return Err(KanbanError::InvalidInput("description 不能为空".to_owned()));
        }
        let _mutation = self.mutation_gate.lock().await;
        let task = self.get_task(task_id).await?;
        if task.status != TaskStatus::Triage {
            return Err(KanbanError::InvalidTransition(
                "只能 specify triage 任务".to_owned(),
            ));
        }
        self.store
            .specify_task(
                task_id,
                crate::store_operations::SpecifyTaskInput {
                    expected_lock_version: task.lock_version,
                    actor: actor.to_owned(),
                    description: command
                        .description
                        .map(|description| description.trim().to_owned()),
                    scheduled_at: command.scheduled_at,
                    event_id: new_event_id(),
                    now: self.clock.now_ms(),
                },
            )
            .await
            .map_err(crate::error::store_error)
            .and_then(super::application_task)
    }
}

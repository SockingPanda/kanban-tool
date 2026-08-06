use kanban_core::{Clock, KanbanError, Result, TaskStatus, new_event_id};

use crate::{KanbanService, TaskStepsRecord};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveStepCommand {
    pub task_id: String,
    pub step_id: String,
    pub actor: String,
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn remove_step(&self, command: RemoveStepCommand) -> Result<TaskStepsRecord> {
        validate_remove_step(&command)?;
        let task_id = command.task_id.trim().to_owned();
        let step_id = command.step_id.trim().to_owned();
        let actor = command.actor.trim().to_owned();
        let _mutation = self.mutation_gate.lock().await;
        let parent = self.get_task(&task_id).await?;
        if parent.archived_at.is_some() || parent.status == TaskStatus::Archived {
            return Err(KanbanError::InvalidTransition(
                "已归档的父任务不能修改 step".to_owned(),
            ));
        }
        let now = self.clock.now_ms();
        self.application
            .store
            .store
            .remove_step(
                &task_id,
                &step_id,
                crate::store_operations::RemoveStepInput {
                    actor,
                    event_id: new_event_id(),
                    recompute_event_id: new_event_id(),
                    updated_at: now,
                    expected_lock_version: parent.lock_version,
                },
            )
            .await
            .map_err(crate::adapter::store_error)?;
        self.list_steps(&task_id).await
    }
}

fn validate_remove_step(command: &RemoveStepCommand) -> Result<()> {
    let task_id = command.task_id.trim();
    if !task_id.starts_with("t_") || task_id.len() <= 2 {
        return Err(KanbanError::InvalidInput(
            "task_id 必须是全局 t_... ID".to_owned(),
        ));
    }
    let step_id = command.step_id.trim().to_owned();
    if !step_id.starts_with("step_") || step_id.len() <= 5 {
        return Err(KanbanError::InvalidInput(
            "step_id 必须是全局 step_... ID".to_owned(),
        ));
    }
    let actor = command.actor.trim().to_owned();
    if actor.is_empty() {
        return Err(KanbanError::InvalidInput("actor 不能为空".to_owned()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use kanban_core::KanbanError;

    use super::RemoveStepCommand;

    #[test]
    fn remove_step_rejects_board_local_task_selector() {
        let error = super::validate_remove_step(&RemoveStepCommand {
            task_id: "default#1".to_owned(),
            step_id: "step_1".to_owned(),
            actor: "operator".to_owned(),
        })
        .expect_err("board-local selector");
        assert!(matches!(error, KanbanError::InvalidInput(_)));
    }
}

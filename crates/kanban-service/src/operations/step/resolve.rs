//! Step 完成、跳过和重新打开的 application service 命令。

use kanban_core::{Clock, KanbanError, Result, TaskStatus, new_event_id};

use crate::{KanbanService, TaskRecord, TaskStepsRecord};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteStepCommand {
    pub task_id: String,
    pub step_id: String,
    pub note: String,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkipStepCommand {
    pub task_id: String,
    pub step_id: String,
    pub reason: String,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReopenStepCommand {
    pub task_id: String,
    pub step_id: String,
    pub reason: String,
    pub actor: String,
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn complete_step(&self, command: CompleteStepCommand) -> Result<TaskStepsRecord> {
        let (task_id, step_id, actor, note, now) = self.validate_resolution(
            command.task_id,
            command.step_id,
            command.actor,
            command.note,
        )?;
        let _mutation = self.mutation_gate.lock().await;
        let parent = self.get_task(&task_id).await?;
        ensure_parent_can_change_steps(&parent)?;
        self.application
            .store
            .store
            .complete_step(
                &task_id,
                &step_id,
                crate::store_operations::CompleteStepInput {
                    note,
                    actor,
                    event_id: new_event_id(),
                    updated_at: now,
                    expected_lock_version: parent.lock_version,
                },
            )
            .await
            .map_err(crate::adapter::store_error)?;
        self.list_steps(&task_id).await
    }
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn skip_step(&self, command: SkipStepCommand) -> Result<TaskStepsRecord> {
        let (task_id, step_id, actor, reason, now) = self.validate_resolution(
            command.task_id,
            command.step_id,
            command.actor,
            command.reason,
        )?;
        let _mutation = self.mutation_gate.lock().await;
        let parent = self.get_task(&task_id).await?;
        ensure_parent_can_change_steps(&parent)?;
        self.application
            .store
            .store
            .skip_step(
                &task_id,
                &step_id,
                crate::store_operations::SkipStepInput {
                    reason,
                    actor,
                    event_id: new_event_id(),
                    updated_at: now,
                    expected_lock_version: parent.lock_version,
                },
            )
            .await
            .map_err(crate::adapter::store_error)?;
        self.list_steps(&task_id).await
    }
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn reopen_step(&self, command: ReopenStepCommand) -> Result<TaskStepsRecord> {
        let (task_id, step_id, actor, reason, now) = self.validate_resolution(
            command.task_id,
            command.step_id,
            command.actor,
            command.reason,
        )?;
        let _mutation = self.mutation_gate.lock().await;
        let parent = self.get_task(&task_id).await?;
        ensure_parent_can_change_steps(&parent)?;
        self.application
            .store
            .store
            .reopen_step(
                &task_id,
                &step_id,
                crate::store_operations::ReopenStepInput {
                    reason,
                    actor,
                    event_id: new_event_id(),
                    updated_at: now,
                    expected_lock_version: parent.lock_version,
                },
            )
            .await
            .map_err(crate::adapter::store_error)?;
        self.list_steps(&task_id).await
    }
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    fn validate_resolution(
        &self,
        task_id: String,
        step_id: String,
        actor: String,
        note: String,
    ) -> Result<(String, String, String, String, i64)> {
        let (task_id, step_id, actor, note) =
            validate_resolution_values(task_id, step_id, actor, note)?;
        let now = self.clock.now_ms();
        Ok((task_id, step_id, actor, note, now))
    }
}

fn validate_resolution_values(
    task_id: String,
    step_id: String,
    actor: String,
    note: String,
) -> Result<(String, String, String, String)> {
    let task_id = task_id.trim().to_owned();
    if !task_id.starts_with("t_") || task_id.len() <= 2 {
        return Err(KanbanError::InvalidInput(
            "task_id 必须是全局 t_... ID".to_owned(),
        ));
    }
    let step_id = step_id.trim().to_owned();
    if !step_id.starts_with("step_") || step_id.len() <= 5 {
        return Err(KanbanError::InvalidInput(
            "step_id 必须是全局 step_... ID".to_owned(),
        ));
    }
    let actor = actor.trim().to_owned();
    if actor.is_empty() {
        return Err(KanbanError::InvalidInput("actor 不能为空".to_owned()));
    }
    let note = note.trim().to_owned();
    if note.is_empty() {
        return Err(KanbanError::InvalidInput(
            "step resolution note/reason 不能为空".to_owned(),
        ));
    }
    Ok((task_id, step_id, actor, note))
}

fn ensure_parent_can_change_steps(parent: &TaskRecord) -> Result<()> {
    if parent.archived_at.is_some() || parent.status == TaskStatus::Archived {
        return Err(KanbanError::InvalidTransition(
            "已归档的父任务不能修改 step".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use kanban_core::KanbanError;

    use super::CompleteStepCommand;

    #[test]
    fn resolution_rejects_invalid_ids_and_empty_reason() {
        let invalid_id = CompleteStepCommand {
            task_id: "default#1".to_owned(),
            step_id: "step_1".to_owned(),
            note: "finished".to_owned(),
            actor: "operator".to_owned(),
        };
        let error = super::validate_resolution_values(
            invalid_id.task_id,
            invalid_id.step_id,
            invalid_id.actor,
            invalid_id.note,
        )
        .expect_err("board-local selector");
        assert!(matches!(error, KanbanError::InvalidInput(_)));

        let error = super::validate_resolution_values(
            "t_step".to_owned(),
            "step_1".to_owned(),
            "operator".to_owned(),
            "  ".to_owned(),
        )
        .expect_err("empty reason");
        assert!(matches!(error, KanbanError::InvalidInput(_)));
    }
}

use std::future::Future;

use kanban_core::{Clock, KanbanError, Result, TaskStatus, new_event_id};

use crate::{ApplicationService, ApplicationStore, TaskRecord};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveTaskCommand {
    pub task_id: String,
    pub actor: String,
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveTaskRecord {
    pub expected_lock_version: i64,
    pub actor: String,
    pub force: bool,
    pub event_id: String,
    pub now: i64,
}

pub trait TaskArchive: ApplicationStore {
    fn get_task(&self, task_id: &str) -> impl Future<Output = Result<TaskRecord>> + Send;

    fn archive_task(
        &self,
        task_id: &str,
        input: ArchiveTaskRecord,
    ) -> impl Future<Output = Result<TaskRecord>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: TaskArchive,
    C: Clock,
{
    pub async fn archive_task(&self, command: ArchiveTaskCommand) -> Result<TaskRecord> {
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
        let task = self.store.get_task(task_id).await?;
        if task.status == TaskStatus::Running && !command.force {
            return Err(KanbanError::InvalidTransition(
                "归档 running 任务必须设置 force".to_owned(),
            ));
        }
        if task.status == TaskStatus::Archived {
            return Err(KanbanError::InvalidTransition("任务已归档".to_owned()));
        }
        if !command.force && task.completed_required_step_count != task.required_step_count {
            return Err(KanbanError::StepsIncomplete(format!(
                "仍有 {} 个必需步骤未完成",
                task.required_step_count
                    .saturating_sub(task.completed_required_step_count)
            )));
        }
        self.store
            .archive_task(
                task_id,
                ArchiveTaskRecord {
                    expected_lock_version: task.lock_version,
                    actor: actor.to_owned(),
                    force: command.force,
                    event_id: new_event_id(),
                    now: self.clock.now_ms(),
                },
            )
            .await
    }
}

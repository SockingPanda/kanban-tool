use std::future::Future;

use kanban_core::{Clock, KanbanError, Result, TaskStatus, new_event_id};

use crate::{ApplicationService, ApplicationStore, TaskRecord};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReopenTaskCommand {
    pub task_id: String,
    pub actor: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReopenTaskRecord {
    pub expected_lock_version: i64,
    pub actor: String,
    pub reason: String,
    pub event_id: String,
    pub now: i64,
}

pub trait TaskReopen: ApplicationStore {
    fn get_task(&self, task_id: &str) -> impl Future<Output = Result<TaskRecord>> + Send;

    fn reopen_task(
        &self,
        task_id: &str,
        input: ReopenTaskRecord,
    ) -> impl Future<Output = Result<TaskRecord>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: TaskReopen,
    C: Clock,
{
    pub async fn reopen_task(&self, command: ReopenTaskCommand) -> Result<TaskRecord> {
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
        let reason = command.reason.trim();
        if reason.is_empty() {
            return Err(KanbanError::InvalidInput(
                "reopen reason 不能为空".to_owned(),
            ));
        }
        let _mutation = self.mutation_gate.lock().await;
        let task = self.store.get_task(task_id).await?;
        if task.status != TaskStatus::Done {
            return Err(KanbanError::InvalidTransition(
                "只能 reopen done 任务".to_owned(),
            ));
        }
        self.store
            .reopen_task(
                task_id,
                ReopenTaskRecord {
                    expected_lock_version: task.lock_version,
                    actor: actor.to_owned(),
                    reason: reason.to_owned(),
                    event_id: new_event_id(),
                    now: self.clock.now_ms(),
                },
            )
            .await
    }
}

use std::future::Future;

use kanban_core::{Clock, KanbanError, Result, TaskStatus, new_event_id};

use crate::{ApplicationService, ApplicationStore, StepRecord, TaskRecord, TaskStepsRecord};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteStepCommand {
    pub task_id: String,
    pub step_id: String,
    pub note: String,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteStepRecord {
    pub note: String,
    pub actor: String,
    pub event_id: String,
    pub updated_at: i64,
    pub expected_lock_version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkipStepCommand {
    pub task_id: String,
    pub step_id: String,
    pub reason: String,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkipStepRecord {
    pub reason: String,
    pub actor: String,
    pub event_id: String,
    pub updated_at: i64,
    pub expected_lock_version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReopenStepCommand {
    pub task_id: String,
    pub step_id: String,
    pub reason: String,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReopenStepRecord {
    pub reason: String,
    pub actor: String,
    pub event_id: String,
    pub updated_at: i64,
    pub expected_lock_version: i64,
}

pub trait StepComplete: ApplicationStore {
    fn get_task(&self, task_id: &str) -> impl Future<Output = Result<TaskRecord>> + Send;

    fn complete_step(
        &self,
        task_id: &str,
        step_id: &str,
        input: CompleteStepRecord,
    ) -> impl Future<Output = Result<StepRecord>> + Send;

    fn list_steps(&self, task_id: &str) -> impl Future<Output = Result<TaskStepsRecord>> + Send;
}

pub trait StepSkip: ApplicationStore {
    fn get_task(&self, task_id: &str) -> impl Future<Output = Result<TaskRecord>> + Send;

    fn skip_step(
        &self,
        task_id: &str,
        step_id: &str,
        input: SkipStepRecord,
    ) -> impl Future<Output = Result<StepRecord>> + Send;

    fn list_steps(&self, task_id: &str) -> impl Future<Output = Result<TaskStepsRecord>> + Send;
}

pub trait StepReopen: ApplicationStore {
    fn get_task(&self, task_id: &str) -> impl Future<Output = Result<TaskRecord>> + Send;

    fn reopen_step(
        &self,
        task_id: &str,
        step_id: &str,
        input: ReopenStepRecord,
    ) -> impl Future<Output = Result<StepRecord>> + Send;

    fn list_steps(&self, task_id: &str) -> impl Future<Output = Result<TaskStepsRecord>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: StepComplete,
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
        let parent = self.store.get_task(&task_id).await?;
        ensure_parent_can_change_steps(&parent)?;
        self.store
            .complete_step(
                &task_id,
                &step_id,
                CompleteStepRecord {
                    note,
                    actor,
                    event_id: new_event_id(),
                    updated_at: now,
                    expected_lock_version: parent.lock_version,
                },
            )
            .await?;
        self.store.list_steps(&task_id).await
    }
}

impl<S, C> ApplicationService<S, C>
where
    S: StepSkip,
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
        let parent = self.store.get_task(&task_id).await?;
        ensure_parent_can_change_steps(&parent)?;
        self.store
            .skip_step(
                &task_id,
                &step_id,
                SkipStepRecord {
                    reason,
                    actor,
                    event_id: new_event_id(),
                    updated_at: now,
                    expected_lock_version: parent.lock_version,
                },
            )
            .await?;
        self.store.list_steps(&task_id).await
    }
}

impl<S, C> ApplicationService<S, C>
where
    S: StepReopen,
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
        let parent = self.store.get_task(&task_id).await?;
        ensure_parent_can_change_steps(&parent)?;
        self.store
            .reopen_step(
                &task_id,
                &step_id,
                ReopenStepRecord {
                    reason,
                    actor,
                    event_id: new_event_id(),
                    updated_at: now,
                    expected_lock_version: parent.lock_version,
                },
            )
            .await?;
        self.store.list_steps(&task_id).await
    }
}

impl<S, C> ApplicationService<S, C>
where
    S: ApplicationStore,
    C: Clock,
{
    fn validate_resolution(
        &self,
        task_id: String,
        step_id: String,
        actor: String,
        note: String,
    ) -> Result<(String, String, String, String, i64)> {
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
        // 在输入校验完成后读取时钟，避免无效请求消耗 mutation 时间戳。
        let now = self.clock.now_ms();
        Ok((task_id, step_id, actor, note, now))
    }
}

fn ensure_parent_can_change_steps(parent: &TaskRecord) -> Result<()> {
    if parent.archived_at.is_some() || parent.status == TaskStatus::Archived {
        return Err(KanbanError::InvalidTransition(
            "已归档的父任务不能修改 step".to_owned(),
        ));
    }
    Ok(())
}

//! Step 完成、跳过和重新打开的 application service 命令与端口。

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

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use super::*;
    use crate::operations::test_support::{FixedClock, StubStore, task_for_id};
    use crate::{ExecutionPlanRecord, ExecutionPlanState};

    fn steps(task_id: &str) -> TaskStepsRecord {
        TaskStepsRecord {
            task_id: task_id.to_owned(),
            steps: vec![],
            execution_plan: ExecutionPlanRecord {
                board_id: "b_default".to_owned(),
                task_id: task_id.to_owned(),
                state: ExecutionPlanState::Planned,
                reason: None,
                updated_by: "tester".to_owned(),
                updated_at: 100,
            },
        }
    }

    fn step_record(task_id: &str, step_id: &str, status: &str, actor: &str) -> StepRecord {
        StepRecord {
            id: step_id.to_owned(),
            parent_task_id: task_id.to_owned(),
            title: "step".to_owned(),
            body: None,
            linked_task: None,
            position: 1024,
            required: true,
            status: status.to_owned(),
            resolution_note: None,
            resolved_by: (status != "todo").then(|| actor.to_owned()),
            resolved_at: (status != "todo").then_some(100),
            created_by: "tester".to_owned(),
            created_at: 100,
            updated_by: actor.to_owned(),
            updated_at: 100,
        }
    }

    impl StepComplete for StubStore {
        async fn get_task(&self, task_id: &str) -> Result<TaskRecord> {
            Ok(task_for_id(task_id))
        }

        async fn complete_step(
            &self,
            task_id: &str,
            step_id: &str,
            input: CompleteStepRecord,
        ) -> Result<StepRecord> {
            assert_eq!((task_id, step_id), ("t_step_done", "step_done"));
            assert_eq!(input.note, "finished");
            assert_eq!(input.actor, "operator");
            assert!(input.event_id.starts_with("e_"));
            assert_eq!(input.updated_at, 100);
            assert_eq!(input.expected_lock_version, 0);
            Ok(step_record(task_id, step_id, "done", &input.actor))
        }

        async fn list_steps(&self, task_id: &str) -> Result<TaskStepsRecord> {
            Ok(steps(task_id))
        }
    }

    impl StepSkip for StubStore {
        async fn get_task(&self, task_id: &str) -> Result<TaskRecord> {
            Ok(task_for_id(task_id))
        }

        async fn skip_step(
            &self,
            task_id: &str,
            step_id: &str,
            input: SkipStepRecord,
        ) -> Result<StepRecord> {
            assert_eq!((task_id, step_id), ("t_step_skip", "step_skip"));
            assert_eq!(input.reason, "not needed");
            assert_eq!(input.actor, "operator");
            assert_eq!(input.expected_lock_version, 0);
            Ok(step_record(task_id, step_id, "skipped", &input.actor))
        }

        async fn list_steps(&self, task_id: &str) -> Result<TaskStepsRecord> {
            Ok(steps(task_id))
        }
    }

    impl StepReopen for StubStore {
        async fn get_task(&self, task_id: &str) -> Result<TaskRecord> {
            Ok(task_for_id(task_id))
        }

        async fn reopen_step(
            &self,
            task_id: &str,
            step_id: &str,
            input: ReopenStepRecord,
        ) -> Result<StepRecord> {
            assert_eq!((task_id, step_id), ("t_step_reopen", "step_reopen"));
            assert_eq!(input.reason, "needs revision");
            assert_eq!(input.actor, "operator");
            assert_eq!(input.expected_lock_version, 0);
            Ok(step_record(task_id, step_id, "todo", &input.actor))
        }

        async fn list_steps(&self, task_id: &str) -> Result<TaskStepsRecord> {
            Ok(steps(task_id))
        }
    }

    #[tokio::test]
    async fn resolution_commands_trim_input_and_return_the_post_mutation_snapshot() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        let done = service
            .complete_step(CompleteStepCommand {
                task_id: " t_step_done ".to_owned(),
                step_id: " step_done ".to_owned(),
                note: " finished ".to_owned(),
                actor: " operator ".to_owned(),
            })
            .await
            .expect("complete step");
        assert_eq!(done.task_id, "t_step_done");
        assert!(done.steps.is_empty());

        let skipped = service
            .skip_step(SkipStepCommand {
                task_id: " t_step_skip ".to_owned(),
                step_id: " step_skip ".to_owned(),
                reason: " not needed ".to_owned(),
                actor: " operator ".to_owned(),
            })
            .await
            .expect("skip step");
        assert_eq!(skipped.task_id, "t_step_skip");

        let reopened = service
            .reopen_step(ReopenStepCommand {
                task_id: " t_step_reopen ".to_owned(),
                step_id: " step_reopen ".to_owned(),
                reason: " needs revision ".to_owned(),
                actor: " operator ".to_owned(),
            })
            .await
            .expect("reopen step");
        assert_eq!(reopened.task_id, "t_step_reopen");
    }

    #[tokio::test]
    async fn resolution_commands_reject_invalid_ids_and_empty_reasons_before_store_access() {
        let calls = Arc::new(AtomicUsize::new(0));
        let service = ApplicationService::with_clock(
            StubStore {
                calls: calls.clone(),
            },
            FixedClock(100),
        );
        let invalid_id = service
            .complete_step(CompleteStepCommand {
                task_id: "default#1".to_owned(),
                step_id: "step_1".to_owned(),
                note: "finished".to_owned(),
                actor: "operator".to_owned(),
            })
            .await
            .expect_err("必须拒绝 board-local task selector");
        assert!(matches!(invalid_id, KanbanError::InvalidInput(_)));

        let empty_reason = service
            .reopen_step(ReopenStepCommand {
                task_id: "t_step".to_owned(),
                step_id: "step_1".to_owned(),
                reason: "  ".to_owned(),
                actor: "operator".to_owned(),
            })
            .await
            .expect_err("必须拒绝空 reason");
        assert!(matches!(empty_reason, KanbanError::InvalidInput(_)));
        assert_eq!(calls.load(Ordering::Relaxed), 0);
    }
}

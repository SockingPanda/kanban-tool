use std::future::Future;

use kanban_core::{Clock, KanbanError, Result, TaskStatus, new_event_id};

use crate::{ApplicationService, ApplicationStore, StepRecord, TaskRecord, TaskStepsRecord};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveStepCommand {
    pub task_id: String,
    pub step_id: String,
    pub actor: String,
}

/// 由 application service 规范化后传给 canonical store 的 step 删除输入。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveStepRecord {
    pub actor: String,
    pub event_id: String,
    pub recompute_event_id: String,
    pub updated_at: i64,
    pub expected_lock_version: i64,
}

pub trait StepRemove: ApplicationStore {
    fn get_task(&self, task_id: &str) -> impl Future<Output = Result<TaskRecord>> + Send;

    fn remove_step(
        &self,
        task_id: &str,
        step_id: &str,
        input: RemoveStepRecord,
    ) -> impl Future<Output = Result<StepRecord>> + Send;

    fn list_steps(&self, task_id: &str) -> impl Future<Output = Result<TaskStepsRecord>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: StepRemove,
    C: Clock,
{
    pub async fn remove_step(&self, command: RemoveStepCommand) -> Result<TaskStepsRecord> {
        let task_id = command.task_id.trim().to_owned();
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
        let _mutation = self.mutation_gate.lock().await;
        let parent = self.store.get_task(&task_id).await?;
        if parent.archived_at.is_some() || parent.status == TaskStatus::Archived {
            return Err(KanbanError::InvalidTransition(
                "已归档的父任务不能修改 step".to_owned(),
            ));
        }
        let now = self.clock.now_ms();
        self.store
            .remove_step(
                &task_id,
                &step_id,
                RemoveStepRecord {
                    actor,
                    event_id: new_event_id(),
                    recompute_event_id: new_event_id(),
                    updated_at: now,
                    expected_lock_version: parent.lock_version,
                },
            )
            .await?;
        self.store.list_steps(&task_id).await
    }
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

    impl StepRemove for StubStore {
        async fn get_task(&self, task_id: &str) -> Result<TaskRecord> {
            Ok(task_for_id(task_id))
        }

        async fn remove_step(
            &self,
            task_id: &str,
            step_id: &str,
            input: RemoveStepRecord,
        ) -> Result<StepRecord> {
            assert_eq!((task_id, step_id), ("t_step_remove", "step_remove"));
            assert_eq!(input.actor, "operator");
            assert!(input.event_id.starts_with("e_"));
            assert!(input.recompute_event_id.starts_with("e_"));
            assert_eq!(input.updated_at, 100);
            assert_eq!(input.expected_lock_version, 0);
            Ok(StepRecord {
                id: step_id.to_owned(),
                parent_task_id: task_id.to_owned(),
                title: "step".to_owned(),
                body: None,
                linked_task: None,
                position: 1024,
                required: true,
                status: "todo".to_owned(),
                resolution_note: None,
                resolved_by: None,
                resolved_at: None,
                created_by: "operator".to_owned(),
                created_at: 100,
                updated_by: "operator".to_owned(),
                updated_at: 100,
            })
        }

        async fn list_steps(&self, task_id: &str) -> Result<TaskStepsRecord> {
            Ok(TaskStepsRecord {
                task_id: task_id.to_owned(),
                steps: vec![],
                execution_plan: ExecutionPlanRecord {
                    board_id: "b_default".to_owned(),
                    task_id: task_id.to_owned(),
                    state: ExecutionPlanState::Unplanned,
                    reason: None,
                    updated_by: "operator".to_owned(),
                    updated_at: 100,
                },
            })
        }
    }

    #[tokio::test]
    async fn remove_step_trims_input_and_returns_the_post_mutation_snapshot() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        let result = service
            .remove_step(RemoveStepCommand {
                task_id: " t_step_remove ".to_owned(),
                step_id: " step_remove ".to_owned(),
                actor: " operator ".to_owned(),
            })
            .await
            .expect("remove step");
        assert_eq!(result.task_id, "t_step_remove");
        assert!(result.steps.is_empty());
        assert_eq!(result.execution_plan.state, ExecutionPlanState::Unplanned);
    }

    #[tokio::test]
    async fn remove_step_rejects_invalid_ids_before_store_access() {
        let calls = Arc::new(AtomicUsize::new(0));
        let service = ApplicationService::with_clock(
            StubStore {
                calls: calls.clone(),
            },
            FixedClock(100),
        );
        let error = service
            .remove_step(RemoveStepCommand {
                task_id: "default#1".to_owned(),
                step_id: "step_1".to_owned(),
                actor: "operator".to_owned(),
            })
            .await
            .expect_err("必须拒绝 board-local task selector");
        assert!(matches!(error, KanbanError::InvalidInput(_)));
        assert_eq!(calls.load(Ordering::Relaxed), 0);
    }
}

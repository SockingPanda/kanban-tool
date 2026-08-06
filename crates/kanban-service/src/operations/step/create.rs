use std::future::Future;

use kanban_core::{
    Clock, KanbanError, ReadinessFacts, Result, TaskStatus, new_event_id, new_typed_id,
    recompute_ready_status,
};

use crate::{
    ApplicationService, ApplicationStore, ExecutionPlanState, StepRecord, TaskRecord,
    TaskStepsRecord,
};

#[derive(Debug, Clone, PartialEq)]
pub struct CreateStepCommand {
    pub task_id: String,
    pub idempotency_key: Option<String>,
    pub title: String,
    pub body: Option<String>,
    pub linked_task_id: Option<String>,
    pub position: Option<i64>,
    pub required: bool,
    pub actor: String,
}

/// application service 传给 Turso store 的规范化 step mutation。预期任务事实让事务
/// 始终受 CAS 保护，即使另一个调用方在 application read 与 store mutation 之间修改了
/// 父任务也不会失守。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateStepRecord {
    pub id: String,
    pub idempotency_key: Option<String>,
    pub title: String,
    pub body: Option<String>,
    pub linked_task_id: Option<String>,
    pub position: Option<i64>,
    pub required: bool,
    pub created_by: String,
    pub event_id: String,
    pub plan_event_id: String,
    pub recompute_event_id: String,
    pub created_at: i64,
    pub expected_lock_version: i64,
    pub expected_plan_state: ExecutionPlanState,
    pub target_status: TaskStatus,
}

pub trait StepCreate: ApplicationStore {
    fn get_task(&self, task_id: &str) -> impl Future<Output = Result<TaskRecord>> + Send;

    fn create_step(
        &self,
        task_id: &str,
        input: CreateStepRecord,
    ) -> impl Future<Output = Result<StepRecord>> + Send;

    fn list_steps(&self, task_id: &str) -> impl Future<Output = Result<TaskStepsRecord>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: StepCreate,
    C: Clock,
{
    pub async fn create_step(&self, command: CreateStepCommand) -> Result<TaskStepsRecord> {
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
        let title = command.title.trim();
        if title.is_empty() {
            return Err(KanbanError::InvalidInput(
                "step title is required".to_owned(),
            ));
        }
        if command.position.is_some_and(|position| position < 0) {
            return Err(KanbanError::InvalidInput(
                "step position must be non-negative".to_owned(),
            ));
        }
        if command
            .idempotency_key
            .as_deref()
            .is_some_and(|key| key.trim().is_empty())
        {
            return Err(KanbanError::InvalidInput(
                "idempotency_key must not be empty".to_owned(),
            ));
        }
        let _mutation = self.mutation_gate.lock().await;
        let parent = self.store.get_task(task_id).await?;
        if parent.archived_at.is_some() || parent.status == TaskStatus::Archived {
            return Err(KanbanError::InvalidTransition(
                "archived parent task cannot receive steps".to_owned(),
            ));
        }
        let linked_task_id = command
            .linked_task_id
            .map(|linked_task_id| {
                let linked_task_id = linked_task_id.trim().to_owned();
                if !linked_task_id.starts_with("t_") || linked_task_id.len() <= 2 {
                    return Err(KanbanError::InvalidInput(
                        "linked_task_id must be a global t_... id".to_owned(),
                    ));
                }
                Ok(linked_task_id)
            })
            .transpose()?;
        if let Some(linked_task_id) = linked_task_id.as_deref() {
            let linked = self.store.get_task(linked_task_id).await?;
            if linked.board_id != parent.board_id {
                return Err(KanbanError::InvalidInput(
                    "linked task must belong to the parent board".to_owned(),
                ));
            }
            if linked.id == parent.id {
                return Err(KanbanError::InvalidInput(
                    "step cannot link to its parent task".to_owned(),
                ));
            }
            if linked.archived_at.is_some() || linked.status == TaskStatus::Archived {
                return Err(KanbanError::InvalidInput(
                    "archived linked task is not allowed".to_owned(),
                ));
            }
        }
        let now = self.clock.now_ms();
        let target_status = if matches!(
            parent.status,
            TaskStatus::Triage | TaskStatus::Todo | TaskStatus::Scheduled | TaskStatus::Ready
        ) {
            recompute_ready_status(
                ReadinessFacts {
                    title: &parent.title,
                    description: parent.description.as_deref(),
                    scheduled_at: parent.scheduled_at,
                    dependencies_done: !parent.dependency_blocked,
                },
                now,
            )
        } else {
            parent.status
        };
        self.store
            .create_step(
                task_id,
                CreateStepRecord {
                    id: new_typed_id("step"),
                    idempotency_key: command.idempotency_key.map(|key| key.trim().to_owned()),
                    title: title.to_owned(),
                    body: command.body.map(|body| body.trim().to_owned()),
                    linked_task_id,
                    position: command.position,
                    required: command.required,
                    created_by: actor.to_owned(),
                    event_id: new_event_id(),
                    plan_event_id: new_event_id(),
                    recompute_event_id: new_event_id(),
                    created_at: now,
                    expected_lock_version: parent.lock_version,
                    expected_plan_state: parent.execution_plan_state,
                    target_status,
                },
            )
            .await?;
        self.store.list_steps(task_id).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, atomic::AtomicUsize};

    use kanban_core::{KanbanError, Result, TaskStatus};

    use crate::operations::test_support::{FixedClock, StubStore, task_for_id};
    use crate::*;

    impl StepCreate for StubStore {
        async fn get_task(&self, task_id: &str) -> Result<TaskRecord> {
            Ok(task_for_id(task_id))
        }

        async fn create_step(&self, task_id: &str, input: CreateStepRecord) -> Result<StepRecord> {
            assert_eq!(task_id, "t_step");
            assert!(input.id.starts_with("step_"));
            assert_eq!(input.idempotency_key.as_deref(), Some("step-retry"));
            assert_eq!(input.title, "step title");
            assert_eq!(input.body.as_deref(), Some("step body"));
            assert_eq!(input.target_status, TaskStatus::Ready);
            assert!(input.event_id.starts_with("e_"));
            assert!(input.plan_event_id.starts_with("e_"));
            assert!(input.recompute_event_id.starts_with("e_"));
            Ok(StepRecord {
                id: input.id,
                parent_task_id: task_id.to_owned(),
                title: input.title,
                body: input.body,
                linked_task: None,
                position: input.position.unwrap_or(1024),
                required: input.required,
                status: "todo".to_owned(),
                resolution_note: None,
                resolved_by: None,
                resolved_at: None,
                created_by: input.created_by.clone(),
                created_at: input.created_at,
                updated_by: input.created_by,
                updated_at: input.created_at,
            })
        }

        async fn list_steps(&self, task_id: &str) -> Result<TaskStepsRecord> {
            assert_eq!(task_id, "t_step");
            Ok(TaskStepsRecord {
                task_id: task_id.to_owned(),
                steps: vec![],
                execution_plan: ExecutionPlanRecord {
                    board_id: "b_default".into(),
                    task_id: task_id.to_owned(),
                    state: ExecutionPlanState::Planned,
                    reason: None,
                    updated_by: "tester".into(),
                    updated_at: 100,
                },
            })
        }
    }
    #[tokio::test]
    async fn create_and_list_steps_share_global_id_and_canonicalize_input() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        let steps = service
            .create_step(CreateStepCommand {
                task_id: " t_step ".into(),
                idempotency_key: Some(" step-retry ".into()),
                title: " step title ".into(),
                body: Some(" step body ".into()),
                linked_task_id: None,
                position: None,
                required: true,
                actor: " tester ".into(),
            })
            .await
            .unwrap();
        assert_eq!(steps.task_id, "t_step");
        assert!(steps.steps.is_empty());
        assert_eq!(steps.execution_plan.state, ExecutionPlanState::Planned);

        let invalid = service
            .list_steps("default#1")
            .await
            .expect_err("board-local selectors must be resolved by the client");
        assert!(
            matches!(invalid, KanbanError::InvalidInput(message) if message.contains("global"))
        );
    }
}

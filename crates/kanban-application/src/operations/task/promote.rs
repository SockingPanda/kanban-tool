use std::future::Future;

use kanban_core::{
    Clock, KanbanError, ReadinessFacts, Result, TaskStatus, can_promote_from, new_event_id,
    recompute_ready_status,
};

use crate::{ApplicationService, ApplicationStore, TaskRecord};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromoteTaskCommand {
    pub task_id: String,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromoteTaskRecord {
    pub expected_lock_version: i64,
    pub actor: String,
    pub event_id: String,
    pub updated_at: i64,
}

pub trait TaskPromote: ApplicationStore {
    fn get_task(&self, task_id: &str) -> impl Future<Output = Result<TaskRecord>> + Send;

    fn promote_task(
        &self,
        task_id: &str,
        input: PromoteTaskRecord,
    ) -> impl Future<Output = Result<TaskRecord>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: TaskPromote,
    C: Clock,
{
    pub async fn promote_task(&self, command: PromoteTaskCommand) -> Result<TaskRecord> {
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
        let _mutation = self.mutation_gate.lock().await;
        let task = self.store.get_task(task_id).await?;
        if !can_promote_from(task.status) {
            return Err(KanbanError::InvalidTransition(format!(
                "cannot promote from {}",
                task.status.as_str()
            )));
        }
        let now = self.clock.now_ms();
        let target = recompute_ready_status(
            ReadinessFacts {
                title: &task.title,
                description: task.description.as_deref(),
                scheduled_at: task.scheduled_at,
                dependencies_done: !task.dependency_blocked,
            },
            now,
        );
        if target != TaskStatus::Ready {
            return Err(KanbanError::InvalidTransition(match target {
                TaskStatus::Todo => "dependency blocked".to_owned(),
                TaskStatus::Scheduled => "scheduled_at is in the future".to_owned(),
                TaskStatus::Triage => "task spec is incomplete".to_owned(),
                _ => format!("cannot promote to {}", target.as_str()),
            }));
        }
        if task.execution_plan_state == crate::ExecutionPlanState::Unplanned {
            return Err(KanbanError::ExecutionPlanRequired(
                "add steps or mark execution plan not_required before promoting task".to_owned(),
            ));
        }
        self.store
            .promote_task(
                task_id,
                PromoteTaskRecord {
                    expected_lock_version: task.lock_version,
                    actor: actor.to_owned(),
                    event_id: new_event_id(),
                    updated_at: now,
                },
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, atomic::AtomicUsize};

    use kanban_core::{KanbanError, Result, TaskStatus};

    use crate::operations::test_support::{FixedClock, StubStore, task_for_id};
    use crate::*;

    impl TaskPromote for StubStore {
        async fn get_task(&self, task_id: &str) -> Result<TaskRecord> {
            Ok(task_for_id(task_id))
        }

        async fn promote_task(
            &self,
            task_id: &str,
            input: PromoteTaskRecord,
        ) -> Result<TaskRecord> {
            assert_eq!(task_id, "t_promote");
            assert_eq!(input.expected_lock_version, 0);
            assert_eq!(input.actor, "promoter");
            assert!(input.event_id.starts_with("e_"));
            assert_eq!(input.updated_at, 100);
            let mut task = task_for_id(task_id);
            task.status = TaskStatus::Ready;
            task.lock_version += 1;
            task.updated_at = input.updated_at;
            Ok(task)
        }
    }
    #[tokio::test]
    async fn promote_task_uses_core_readiness_and_plan_guards() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        let promoted = service
            .promote_task(PromoteTaskCommand {
                task_id: "t_promote".into(),
                actor: " promoter ".into(),
            })
            .await
            .unwrap();
        assert_eq!(promoted.status, TaskStatus::Ready);
        assert_eq!(promoted.lock_version, 1);

        let unplanned = service
            .promote_task(PromoteTaskCommand {
                task_id: "t_unplanned".into(),
                actor: "promoter".into(),
            })
            .await
            .unwrap_err();
        assert!(matches!(unplanned, KanbanError::ExecutionPlanRequired(_)));

        let future = service
            .promote_task(PromoteTaskCommand {
                task_id: "t_future".into(),
                actor: "promoter".into(),
            })
            .await
            .unwrap_err();
        assert!(matches!(future, KanbanError::InvalidTransition(_)));

        let running = service
            .promote_task(PromoteTaskCommand {
                task_id: "t_running".into(),
                actor: "promoter".into(),
            })
            .await
            .unwrap_err();
        assert!(matches!(running, KanbanError::InvalidTransition(_)));
    }
}
